// Copyright 2019 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// druid stuff
// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use core::time::Duration;

// use druid::piet::{PietTextLayoutBuilder, TextStorage};
use druid::text::{Attribute, RichText, TextStorage};
use druid::piet::{PietTextLayoutBuilder, TextStorage as PietTextStorage};
use druid::widget::prelude::*;
use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};
use druid::lens::{self, LensExt};
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, SizedBox, RawLabel, List, Scroll, Controller, ControllerHost, Container, Painter, Radio};

use druid::{
    AppLauncher, Application, Color, Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    FontWeight, FontDescriptor, FontFamily, Point,
    Menu, MenuItem, TimerToken,
    Lens, LocalizedString, theme, UnitPoint, Widget, WidgetPod, WidgetExt, WindowDesc, WindowId,
    Command, Selector, Target};

use rodio::{Decoder, OutputStream, source::Source, Sink};

// ical stuff
extern crate ical;
use ical::generator;
use crate::ical::{generator::*, *, parser::*};
use ical::parser::ical::component::IcalTodo;
use ical::parser::ical::component::IcalAlarm;

// std stuff
use std::io::BufReader;
use std::fs::File;
use std::any::type_name;
use std::rc::Rc;
use std::fs;
use std::time::Instant;
use std::time::SystemTime;
use std::thread;

use std::env;

// uid stuff
use uuid::v1::{Timestamp, Context};
use uuid::Uuid;

// chrono
use chrono::prelude::*;

mod editable_label;
use crate::editable_label::EditableLabel;

mod maybe;
use crate::maybe::Maybe;

type ImportResult<T> = std::result::Result<T, String>;

fn generate_uid() -> String {
    let context = Context::new(42);
    let epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let ts = Timestamp::from_unix(&context, epoch.as_secs(), epoch.subsec_nanos());
    let uuid = Uuid::new_v1(ts, &[1, 2, 3, 4, 5, 6]).expect("failed to generate UUID");
    return uuid.to_string();
}

fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}

type PropertyMap = HashMap<String, Rc<Property>>;

type TaskMap = HashMap<String, Task>;

#[derive(Debug, Clone, Data)]
struct TrackingState {
    active: bool,
    task_uid: String,
    timestamp: Rc<DateTime<Utc>>,
    timer_id: Rc<TimerToken>
}

#[derive(Clone, Data)]
struct ViewState {
    filterByTag: String,
    filterByRelevance: String
}

#[derive(Clone, Data, Lens)]
struct AppModel {
    tasks: TaskMap,
    tags: Vector<String>,
    focus: Vector<String>,
    tracking: TrackingState,
    view: ViewState,
    selected_task: String,
    focus_filter: String,
    tag_filter: Option<String>,
}

#[derive(Debug, Clone, Data, PartialEq)]
enum TaskStatus {
    NEEDS_ACTION,
    COMPLETED,
    IN_PROCESS,
    CANCELLED
}

impl TaskStatus {
    fn to_string(&self) -> &str {
        match &self {
            TaskStatus::NEEDS_ACTION => "Needs action",
            TaskStatus::COMPLETED    => "Completed",
            TaskStatus::IN_PROCESS   => "In process",
            TaskStatus::CANCELLED    => "Cancelled",
            _ => {panic!("Unknown status {:?}", self);}
        }
    }
}

#[derive(Debug, Clone, Data, Lens)]
struct Task {
    name: String,
    description: String,
    uid: String,
    categories: Vector<String>,
    priority: u32,
    task_status: TaskStatus,
    seq: u32,
    time_records: Vector<TimeRecord>,
}

#[derive(Debug, Clone, Data)]
struct TimeRecord {
    from: Rc<DateTime<Utc>>,
    to: Rc<DateTime<Utc>>,
}

static TIMER_INTERVAL: Duration = Duration::from_secs(10);
static UI_TIMER_INTERVAL: Duration = Duration::from_secs(1);

const COMMAND_TASK_START:  Selector<String>    = Selector::new("tcmenu.task_start");
const COMMAND_TASK_STOP:   Selector            = Selector::new("tcmenu.task_stop");
const COMMAND_TASK_SWITCH: Selector<String>    = Selector::new("tcmenu.task_switch");
const COMMAND_TASK_NEW:    Selector            = Selector::new("tcmenu.task_new");
const COMMAND_TASK_EDIT:   Selector<String>    = Selector::new("tcmenu.task_edit");
const COMMAND_TASK_DELETE: Selector<String>    = Selector::new("tcmenu.task_delete");
const COMMAND_TASK_COMPLETED: Selector<String> = Selector::new("tcmenu.task_completed");

const SOUND_TASK_FINISH: &str = "res/bell.ogg";

const TASK_FOCUS_CURRENT: &str = "Current";
const TASK_FOCUS_COMPLETED: &str = "Completed";
const TASK_FOCUS_ALL: &str = "All";

impl Task {
    fn new(name: String, description: String,
           uid: String, categories: Vector<String>,
           priority: u32, task_status: TaskStatus, seq: u32,
           time_records: Vector<TimeRecord>) -> Task {
        return Task{name, description, uid, categories, priority, task_status, seq, time_records};
    }
}

impl AppModel {
    fn get_uids_filtered(&self) -> impl Iterator<Item = String> + '_ {
        self.tasks.keys().cloned().filter(move |uid| {
            let task = self.tasks.get(uid).expect("unknown uid");

            let focus_ok = match self.focus_filter.as_str() {
                TASK_FOCUS_CURRENT => {task.task_status == TaskStatus::NEEDS_ACTION ||
                              task.task_status == TaskStatus::IN_PROCESS},
                TASK_FOCUS_COMPLETED => task.task_status == TaskStatus::COMPLETED,
                TASK_FOCUS_ALL => true,
                _ => panic!("Unknown focus filter {}", &self.focus_filter),
            };

            let tag_ok =
                if let Some(ref tag_filter) = self.tag_filter {
                    task.categories.contains(tag_filter)
                } else {
                    true
                };

            return focus_ok && tag_ok;
        })
    }

    fn check_update_selected(&mut self) {
        let filtered: Vector<String> = self.get_uids_filtered().collect();

        // select any task if currently selected is filtered out
        if !filtered.contains(&self.selected_task) {
            self.selected_task = filtered.front().unwrap_or(&"".to_string()).clone();
        }
    }

    fn update_tags(&mut self) {
        self.tags.clear();

        for (_, task) in self.tasks.iter() {
            for tag in &task.categories {
                self.tags.push_back(tag.clone());
            }
        }

        self.tags.sort();
    }
}

fn convert_ts(optstr: Option<String>) -> Vector<String> {
    match optstr {
        Some(st) => vector![st],
        None => Vector::new(),
    }
}

fn props_by_name(prop_vec: &Vec<Property>) -> PropertyMap {
    let mut result = PropertyMap::new();

    for p in prop_vec {
        result.insert(p.name.clone(), Rc::new(p.clone()));
    }

    return result;
}

// fn todos_by_uid(todo_vec: &Vec<IcalTodo>) -> TodoMap {
//     let mut result = TodoMap::new();

//     for task in todo_vec {
//         let properties = props_by_name(&task.properties);

//         result.insert(properties.get("UID").unwrap().value.clone().unwrap(),
//                       TrackerTodo{properties, alarms: Vector::new()});
//     }

//     return result;
// }

fn parse_time_records(optsrc: &Option<String>) -> Vector<TimeRecord> {
    let mut result = Vector::new();

    let split = optsrc.as_ref().unwrap().split(";");

    for s in split {
        let res = Utc.datetime_from_str(&s, "%Y-%m-%d %H:%M:%S");
    }

    return result;
}

fn parse_todo(ical_todo: &IcalTodo) -> ImportResult<Task> {
    let mut summary = String::new();
    let mut description = String::new();
    let mut uid = String::new();
    let mut categories = Vector::new();
    let mut priority = 0;
    let mut status = TaskStatus::NEEDS_ACTION;
    let mut seq = 0;
    let mut time_records = Vector::new();

    for property in &ical_todo.properties {
        // println!("{}", property);
        // println!("{}", type_of(&property));

        match property.name.as_ref() {

            "UID" => {uid = property.value.as_ref().unwrap().clone();}
            "SUMMARY" => {summary = property.value.as_ref().unwrap().clone();}
            "DESCRIPTION" => {description = property.value.clone().unwrap_or("".to_string());}
            "CATEGORIES" => {
                if (property.value.is_some()) {
                    categories.insert(0,  property.value.as_ref().unwrap().clone());
                }
            }
            "STATUS" => {
                status = if let Some(ref sta) = property.value {
                    match sta.as_str() {
                        "NEEDS-ACTION" => TaskStatus::NEEDS_ACTION,
                        "COMPLETED" => TaskStatus::COMPLETED,
                        "IN-PROCESS" => TaskStatus::IN_PROCESS,
                        "CANCELLED" => TaskStatus::CANCELLED,
                        _ => {panic!("Unknown status {}", sta);
                              TaskStatus::NEEDS_ACTION}
                    }
                } else {
                    TaskStatus::NEEDS_ACTION
                };
            }
            "PRIORITY" => {
                if (property.value.is_some()) {
                    priority = property.value.as_ref().unwrap().parse::<u32>().unwrap();
                }
            }
            "SEQUENCE" => {
                if (property.value.is_some()) {
                    seq = property.value.as_ref().unwrap().parse::<u32>().unwrap();
                }
            },
            "TIME_RECORDS" => {
                if (property.value.is_some()) {
                    time_records =
                        parse_time_records(&property.value);
                }
            }
            _ => {}
        }
    }

    return Ok(Task::new(summary, description, uid, categories, priority, status, seq, time_records));
}

fn parse_ical(file_path: String) -> (TaskMap, OrdSet<String>) {
    let buf = BufReader::new(File::open(file_path)
        .unwrap());

    let mut reader = ical::IcalParser::new(buf);

    let mut tags = OrdSet::new();

    let ical = reader.next().unwrap().unwrap();

    // let tracker_todos = todos_by_uid(&ical.todos);
    // println!("todos: {:?}", tracker_todos);

    let mut task_map = TaskMap::new();


    for ical_todo in &ical.todos {
        let task = parse_todo(ical_todo).unwrap();

        for tag in &task.categories {
            tags.insert(tag.clone());
        }

        task_map.insert(task.uid.clone(), task);
    }


    // let tags = vector![String::from("computer"), String::from("outside")];

    return (task_map, tags);
}

// fn update_ical(src: &IcalCalendar, todo_map: &TaskMap) -> IcalCalendar {
//     let mut ical = src.clone();

//     ical.todos.clear();
//     for (uid, todo) in todo_map {
//         let mut ical_props = Vec::<Property>::new();
//         let mut ical_alarms = Vec::<IcalAlarm>::new();

//         for (name, task) in &todo.properties {
//             ical_props.insert(0, Property::clone(task));
//         }

//         for alarm in &todo.alarms {
//             ical_alarms.insert(0, IcalAlarm::clone(alarm));
//         }

//         ical.todos.insert(0, IcalTodo{properties: ical_props, alarms: ical_alarms});
//     }
//     return ical
// }

fn get_any_task_uid(tasks: &TaskMap) -> String {
    let null_uid = "".to_string();
    tasks.keys().nth(0).unwrap_or(&null_uid).clone()
}

fn emit(cal: &IcalCalendar) {
    let generated = cal.generate();
    fs::write("/home/dc/Tasks-generated.ics", generated).expect("Unable to write Tasks-generated.ics");
}

fn play_sound(file: String) {
    thread::spawn(move || {
        // Get a output stream handle to the default physical sound device
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        // Load a sound from a file, using a path relative to Cargo.toml
        let file = BufReader::new(File::open(file).unwrap());
        // Decode that sound file into a source
        let source = Decoder::new(file).unwrap();

        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.append(source);

        // The sound plays in a separate thread. This call will block the current thread until the sink
        // has finished playing all its queued sounds.
        // sink.sleep_until_end();

        // The sound plays in a separate audio thread,
        // so we need to keep the main thread alive while it's playing.
        std::thread::sleep(std::time::Duration::from_secs(sink.len() as u64));
    });
}

pub fn main() {

    let args: Vec<String> = env::args().collect();

    let file_path = match args.len() {
        // no arguments passed
        1 => String::from("/home/dc/Tasks.ics"),
        2 => args[1].clone(),
        _ => args[1].clone(),
    };

    const COMMAND_TASK_COMPLETED: Selector<String> = Selector::new("tcmenu.task_completed");

    // "NEEDS-ACTION" ;Indicates to-do needs action.
    // "COMPLETED"    ;Indicates to-do completed.
    // "IN-PROCESS"   ;Indicates to-do in process of.
    // "CANCELLED"    ;Indicates to-do was cancelled.
    // https://www.kanzaki.com/docs/ical/status.html

    let focus = vector![TASK_FOCUS_CURRENT.to_string(),
                        TASK_FOCUS_COMPLETED.to_string(),
                        TASK_FOCUS_ALL.to_string()];

    let (tasks, tags) = parse_ical(file_path);
    let selected_task = get_any_task_uid(&tasks);

    let mut data = AppModel{
        tasks,
        tags: tags.iter().map(|x : &String| {x.clone()}).collect(),
        focus,
        tracking: TrackingState{active: false, task_uid: "".to_string(),
                                timestamp: Rc::new(Utc::now()),
                                timer_id: Rc::new(TimerToken::INVALID)},
        view: ViewState{filterByTag: String::from(""), filterByRelevance: String::from("")},
        selected_task: selected_task,
        focus_filter: TASK_FOCUS_CURRENT.to_string(),
        tag_filter: None
    };

    // TODO should be done in ctor
    data.update_tags();

    let main_window = WindowDesc::new(ui_builder())
        .menu(make_menu)
        .title(LocalizedString::new("time-tracker-window-title").with_placeholder("Time tracker"));
    
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .expect("launch failed");
}

fn start_tracking(data: &mut AppModel, uid: String) {
    data.tracking.active = true;
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.task_uid = uid;
}

fn stop_tracking(data: &mut AppModel) {
    if data.tracking.task_uid.is_empty() {return};

    let mut task = data.tasks.get(&data.tracking.task_uid).expect("unknown uid").clone();
    let now = Rc::new(Utc::now());

    task.time_records.push_back(TimeRecord{from: data.tracking.timestamp.clone(), to: now.clone()});

    let duration = now.signed_duration_since(data.tracking.timestamp.as_ref().clone());

    println!("Task '{}' duration: {}:{}:{}", &task.name,
             duration.num_hours(), duration.num_minutes(), duration.num_seconds());

    data.tasks = data.tasks.update(task.uid.clone(), task);
    data.tracking.active = false;
    data.tracking.task_uid = "".to_string();
}

fn delete_task(model: &mut AppModel, uid: &String) {
    model.tasks.remove(uid);
    model.update_tags();
    model.check_update_selected();
}

#[allow(unused_assignments)]
fn make_menu(_: Option<WindowId>, model: &AppModel, _: &Env) -> Menu<AppModel> {
    let mut base = Menu::empty();

    // base.rebuild_on(|old_data, data, _env| old_data.menu_count != data.menu_count)
    let mut file = Menu::new(LocalizedString::new("File"));

    file = file.entry(
        MenuItem::new(LocalizedString::new("Import ical"))
            .on_activate(move |_ctx, data, _env| {})
    );

    file = file.entry(
        MenuItem::new(LocalizedString::new("Export ical"))
            .on_activate(move |_ctx, data, _env| {})
    );
    
    file = file.entry(
        MenuItem::new(LocalizedString::new("Exit"))
            .on_activate(move |_ctx, data, _env| {Application::global().quit();})
    );
    
    base.entry(file)
}


fn make_task_context_menu(d: &AppModel, current: &String) -> Menu<AppModel> {
    let selected_task = d.tasks.get(current).expect("unknown uid");

    let uid = current.clone();

    // TODO: understand ownership with 'static type bound here

    let start_stop_item =
        if uid.eq(&d.tracking.task_uid) {
            MenuItem::new(LocalizedString::new("Stop tracking")).on_activate(
                move |ctx, d: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_STOP.with(()));
                }
            )
        } else if d.tracking.task_uid.is_empty() {
            MenuItem::new(LocalizedString::new("Start tracking")).on_activate(
                move |ctx, d: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_START.with(uid.clone()));
                }
            )
        } else {
            MenuItem::new(LocalizedString::new("Switch to")).on_activate(
                move |ctx, d: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_SWITCH.with(uid.clone()));
                }
            )
        };

    // TODO: understand ownership with 'static type bound here
    let uid_new = current.clone();
    let uid_edit = current.clone();
    let uid_delete = current.clone();
    let uid_completed = current.clone();

    Menu::empty()
        .entry(
            start_stop_item,
        )
        .entry(
            MenuItem::new(LocalizedString::new("Edit"))
                .on_activate(
                    move |ctx, data: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_EDIT.with(uid_edit.clone()));
                }),
        )
        .entry(
            MenuItem::new(LocalizedString::new("Mark completed"))
                .on_activate(
                    move |ctx, data: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_COMPLETED.with(uid_completed.clone()));
                }),
        )
        .entry(
            MenuItem::new(LocalizedString::new("New task"))
                .on_activate(
                    move |ctx, model: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_NEW.with(()));
                }),
        )
        .entry(
            MenuItem::new(LocalizedString::new("Delete")).on_activate(
                move |ctx, model: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_DELETE.with(uid_delete.clone()));
                },
            ),
        )
}

struct TaskListWidget {
    inner: WidgetPod<(AppModel, Vector<String>), List<(AppModel, String)>>
}

impl TaskListWidget {
    fn new() -> TaskListWidget {
        static TASK_COLOR_BG: Color = Color::rgb8(127, 0, 127);
        static TASK_ACTIVE_COLOR_BG: Color = Color::rgb8(127, 0, 127);

        let inner = List::new(|| {

                let task_painter =
                    Painter::new(|ctx: &mut PaintCtx, (shared, uid): &(AppModel, String), _env| {
                        let bounds = ctx.size().to_rect();
                        if shared.selected_task.eq(uid) {
                            ctx.fill(bounds, &TASK_ACTIVE_COLOR_BG);
                        }
                        else {
                            ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                        }
                    });

                let container = Container::new(
                    Label::new(|(d, uid): &(AppModel, String), _env: &_| {
                        let task = d.tasks.get(uid).expect("unknown uid");
                        format!("{}[{:?}][{:?}]", task.name, task.task_status, task.categories)
                    })
                        .expand()
                        .on_click(|_ctx, (shared, uid): &mut (AppModel, String), _env| {
                            shared.selected_task = uid.clone();
                        })
                        .align_vertical(UnitPoint::LEFT),
                    )
                    .background(task_painter)
                    .fix_height(50.0);

                // container
                ControllerHost::new(container, ContextMenuController)
        })
            .with_spacing(10.);

        return TaskListWidget{inner: WidgetPod::new(inner)};
    }
}

impl Widget<(AppModel, Vector<String>)> for TaskListWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event,
             data: &mut (AppModel, Vector<String>), _env: &Env) {

        match event {
            //TODO rewrite when "if let" guards are stablilized
            // https://github.com/rust-lang/rust/issues/51114

            Event::Command(cmd) if cmd.is(COMMAND_TASK_START) => {
                start_tracking(&mut data.0, cmd.get(COMMAND_TASK_START).unwrap().clone());
                data.0.tracking.timer_id = Rc::new(ctx.request_timer(TIMER_INTERVAL));
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_STOP) => {
                stop_tracking(&mut data.0);
                data.0.tracking.timer_id = Rc::new(TimerToken::INVALID);
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_SWITCH) => {
                stop_tracking(&mut data.0);
                start_tracking(&mut data.0, cmd.get(COMMAND_TASK_SWITCH).unwrap().clone());
                data.0.tracking.timer_id = Rc::new(ctx.request_timer(TIMER_INTERVAL));
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_NEW) => {
                let uid = generate_uid();
                let task = Task::new("new task".to_string(), "".to_string(), uid.clone(), Vector::new(),
                                     0, TaskStatus::NEEDS_ACTION, 0, Vector::new());

                data.0.selected_task = task.uid.clone();
                data.0.tasks.insert(uid.clone(), task);
                data.0.update_tags();
                ctx.request_update();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_EDIT) => {

            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_COMPLETED) => {
                let uid = cmd.get(COMMAND_TASK_COMPLETED).unwrap().clone();
                let mut task = data.0.tasks.get(&uid).expect("unknown uid").clone();
                task.task_status = TaskStatus::COMPLETED;
                if data.0.tracking.task_uid.eq(&uid) {
                    stop_tracking(&mut data.0);
                }

                data.0.tasks = data.0.tasks.update(uid, task);
                data.0.check_update_selected();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_DELETE) => {
                delete_task(&mut data.0, cmd.get(COMMAND_TASK_DELETE).unwrap());
                ctx.request_update();
            },
            Event::Timer(id) => {
                if *id == *data.0.tracking.timer_id {
                    println!("timer for task {} finished", data.0.tracking.task_uid);
                    play_sound(SOUND_TASK_FINISH.to_string());
                    stop_tracking(&mut data.0);
                }
            }

            _ => self.inner.event(ctx, event, data, _env),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(AppModel, Vector<String>), _env: &Env) {
        self.inner.lifecycle(ctx, event, _data, _env)
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(AppModel, Vector<String>), _data: &(AppModel, Vector<String>), _env: &Env) {
        self.inner.update(_ctx, _data, _env)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &(AppModel, Vector<String>), env: &Env,
    ) -> Size {
        let ret = self.inner.layout(ctx, bc, data, env);
        self.inner.set_origin(ctx, &data, env, Point::ORIGIN);
        return ret;
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &(AppModel, Vector<String>), env: &Env) {
        self.inner.paint(ctx, data, env)
    }
}


struct ContextMenuController;

impl<W: Widget<(AppModel, String)>> Controller<(AppModel, String), W> for ContextMenuController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut (AppModel, String),
        env: &Env,
    ) {
        match event {
            Event::MouseDown(ref mouse) if mouse.button.is_right() => {
                println!("mouse down");
                ctx.show_context_menu(make_task_context_menu(&data.0, &data.1), mouse.pos);
            }
            _ => child.event(ctx, event, data, env),
        }
    }
}

struct StatusBar {
    inner: WidgetPod<String, Label<String>>,
    timer_id: TimerToken,
}

fn format_duration(dur: chrono::Duration) -> String {
    if dur.num_hours() > 0 {
        format!("{:02}h:{:02}m:{:02}s", dur.num_hours(), dur.num_minutes(), dur.num_seconds())
    } else if dur.num_minutes() > 0 {
        format!("{:02}m:{:02}s", dur.num_minutes(), dur.num_seconds())
    } else {
        format!("{:02}s", dur.num_seconds())
    }
}

fn get_status_string(d: &AppModel) -> String {
    if d.tracking.active {
        let active_task = &d.tasks.get(&d.tracking.task_uid).expect("unknown uid");
        let duration = Utc::now().signed_duration_since(d.tracking.timestamp.as_ref().clone());
        let total = chrono::Duration::from_std(Duration::from_secs(10)).unwrap();

        format!("Active task: '{}' | Elapsed: {} / {}",
                active_task.name, format_duration(duration), format_duration(total))
    }
    else {
        if d.selected_task.is_empty() {
            format!("No records")
        }
        else {
            let selected = d.tasks.get(&d.selected_task).expect("unknown uid");
            format!("Records: {}", selected.time_records.len())
        }
    }
}

impl StatusBar {
    fn new() -> StatusBar {
        StatusBar{inner: WidgetPod::new(Label::dynamic(|d: &String, _env| d.clone())),
                  timer_id: TimerToken::INVALID}
    }
}

impl Widget<AppModel> for StatusBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppModel, env: &Env) {

        if self.timer_id == TimerToken::INVALID {
            self.timer_id = ctx.request_timer(UI_TIMER_INTERVAL);
        }

        let mut status = get_status_string(&data);

        match event {
            Event::Timer(id) => {
                if *id == self.timer_id {
                    self.timer_id = ctx.request_timer(UI_TIMER_INTERVAL);
                    ctx.request_update();
                    self.inner.event(ctx, event, &mut status, env);
                }
            },
            _ => {self.inner.event(ctx, event, &mut status, env)},
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.lifecycle(ctx, event, &status, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &AppModel, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.update(ctx, &status, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &AppModel, env: &Env) -> Size {
        let status = get_status_string(&data);
        let ret = self.inner.layout(ctx, &bc.loosen(), &status, env);
        self.inner.set_origin(ctx, &status, env, Point::ORIGIN);
        return ret;
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.paint(ctx, &status, env);
    }
}

fn task_details_widget() -> impl Widget<Task> {
    static TASK_COLOR_BG: Color = Color::rgb8(127, 0, 127);
    static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI);

    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    column.add_flex_child(
        EditableLabel::parse()
        .with_font(FONT_CAPTION_DESCR.clone())
        .padding(10.0)
        .background(TASK_COLOR_BG.clone())
        .fix_height(50.0)
            .lens(lens::Identity.map(
                |d: &Task| d.name.clone(),
                |d: &mut Task, x: String| {
                    d.name = x;
                    d.seq += 1;
                },
            )),
        1.0
    );

    column.add_spacer(10.0);

    column.add_flex_child(
        EditableLabel::parse()
        .padding(10.0)
        .background(TASK_COLOR_BG.clone())
        .fix_height(50.0)
            .lens(lens::Identity.map(
                |d: &Task| d.description.clone(),
                |d: &mut Task, x: String| {
                    d.description = x;
                    d.seq += 1;
                },
            ))
        ,
        1.0
    );

    column.add_spacer(10.0);

    column.add_flex_child(
        Label::new(|task: &Task, _env: &_| {
            let mut result = String::new();

            for record in &task.time_records {
                let new_record = format!("{:?} - {:?}\n", record.from, record.to);
                result.push_str(&new_record);
            }

            return result;
        })
        .padding(10.0)
        .background(TASK_COLOR_BG.clone())
        .fix_height(50.0),
        1.0
    );

    column.add_default_spacer();

    column.add_child(Label::new("Status")
                     .with_font(FontDescriptor::new(FontFamily::SYSTEM_UI)
                                .with_weight(FontWeight::BOLD)
                                .with_size(16.0)));

    column.add_flex_child(
        Flex::row()
            .with_child(Radio::new("needs action" , TaskStatus::NEEDS_ACTION))
            .with_child(Radio::new("in process"   , TaskStatus::IN_PROCESS))
            .with_child(Radio::new("completed"    , TaskStatus::COMPLETED))
            .with_child(Radio::new("cancelled"    , TaskStatus::CANCELLED))
            .lens(Task::task_status),
        1.0,
    );

    // DropdownSelect from widget nursery creates separated window
    // column.add_flex_child(
    //     DropdownSelect::new(vec![
    //         ("needs action" , TaskStatus::NEEDS_ACTION),
    //         ("in process"   , TaskStatus::IN_PROCESS),
    //         ("completed"    , TaskStatus::COMPLETED),
    //         ("cancelled"    , TaskStatus::CANCELLED),
    //     ])
    //     .align_left()
    //     .lens(Task::task_status),
    //     1.0,
    // );

    return column;
}

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    let mut tasks_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    static TASK_COLOR_BG: Color = Color::rgb8(127, 0, 127);
    static TASK_ACTIVE_COLOR_BG: Color = Color::rgb8(127, 0, 127);

    static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI)
    .with_weight(FontWeight::BOLD)
    .with_size(18.0);

    focus_column.add_default_spacer();
    focus_column.add_flex_child(Label::new("Focus").with_font(FONT_CAPTION_DESCR.clone()), 1.0);
    focus_column.add_default_spacer();

    focus_column.add_child(
        Scroll::new(List::new(|| {
            Container::new(
                Label::new(|item: &(AppModel, String), _env: &_| format!("{}", item.1))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .background(
                        Painter::new(|ctx: &mut PaintCtx, (shared, id): &(AppModel, String), _env| {
                            let bounds = ctx.size().to_rect();
                            if shared.focus_filter.eq(id) {
                                ctx.fill(bounds, &TASK_ACTIVE_COLOR_BG);
                            }
                            else {
                                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                            }
                        })
                    )
            )
            .on_click(|_ctx, (model, what): &mut (AppModel, String), _env| {
                model.focus_filter = what.clone();
                model.check_update_selected();
            })
        }))
        .vertical()
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel| (d.clone(), d.focus.clone()),
                |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                    // If shared data was changed reflect the changes in our AppModel
                    *d = x.0
                },
            ))
    );

    focus_column.add_default_spacer();
    focus_column.add_child(Label::new("Tags").with_font(FONT_CAPTION_DESCR.clone()));
    focus_column.add_default_spacer();

    focus_column.add_flex_child(
        Scroll::new(List::new(|| {
            Container::new(
                Label::new(|item: &(AppModel, String), _env: &_| format!("{}", item.1))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .background(
                        Painter::new(|ctx: &mut PaintCtx, (shared, id): &(AppModel, String), _env| {
                            let bounds = ctx.size().to_rect();
                            if shared.tag_filter.is_some() &&
                                shared.tag_filter.as_ref().unwrap().eq(id) {
                                ctx.fill(bounds, &TASK_ACTIVE_COLOR_BG);
                            }
                            else {
                                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                            }
                        })
                    )
            )
                .on_click(|_ctx, (shared, what): &mut (AppModel, String), _env| {
                    if let Some(ref tf) = shared.tag_filter {
                        if tf.eq(what) {
                            shared.tag_filter = None;
                            return;
                        }
                    }

                    shared.tag_filter = Some(what.clone());
                })
        }))
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.tags.clone()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        )),
        1.0,
    );

    main_row.add_child(focus_column);
    main_row.add_default_spacer();

    let tasks_scroll = Scroll::new(
            TaskListWidget::new()
        )
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.get_uids_filtered().collect()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        ));

    // Build a list with shared data
    tasks_column.add_flex_child(tasks_scroll, 2.0);

    tasks_column.add_spacer(10.0);

    tasks_column.add_flex_child(
        Maybe::new(
            || task_details_widget().boxed(),
            || SizedBox::empty().expand_width().boxed(),
        )
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel| d.tasks.get(&d.selected_task).map_or(None, |r| Some(r.clone())),
                |d: &mut AppModel, x: Option<Task>| {
                    x.map(|new_task| d.tasks = d.tasks.update(d.selected_task.clone(), new_task));
                },
            )),
        1.0
    );


    main_row.add_flex_child(tasks_column, 1.0);

    root.add_flex_child(main_row, 1.0);

    // bottom row 
    // root.add_child(
    //     Button::new("Save")
    //         .on_click(|_ctx, (model): &mut (AppModel), _env| {
    //             // todo dont clone IcalCalendar
    //             // let newcal = update_ical(&mut IcalCalendar::clone(&model.cal), &model.tasks);
    //             // emit(&newcal);
    //             // model.cal = Rc::new(newcal)
    //         })
    //         .fix_size(120.0, 20.0)
    //         .align_vertical(UnitPoint::CENTER),
    // );

    root.with_child(StatusBar::new()).align_horizontal(UnitPoint::RIGHT)
        // .debug_paint_layout()
}

