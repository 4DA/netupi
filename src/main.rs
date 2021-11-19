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
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, SizedBox, RawLabel, List, Scroll, Controller, ControllerHost, Container, Painter, Radio, TextBox};

use druid::{
    AppLauncher, Application, Color, Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    FontWeight, FontDescriptor, FontFamily, Point,
    Menu, MenuItem, TimerToken, KeyOrValue,
    Lens, LocalizedString, theme, UnitPoint, Widget, WidgetId, WidgetPod, WidgetExt, WindowDesc, WindowId,
    Command, Selector, Target};

use rodio::{Decoder, OutputStream, source::Source, Sink};

// std stuff
use std::io::BufReader;
use std::fs::File;
use std::fs;
use std::rc::Rc;
use std::any::type_name;
use std::time::Instant;
use std::time::SystemTime;
use std::thread;
use std::env;

// uid stuff
use uuid::v1::{Timestamp, Context};
use uuid::Uuid;

use anyhow::{anyhow};

// chrono
use chrono::prelude::*;

mod editable_label;
use crate::editable_label::EditableLabel;

mod maybe;
use crate::maybe::Maybe;

mod task;
use task::*;

mod icalendar;
use icalendar::parse_ical;

mod db;
use db::*;

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

#[derive(Debug, Clone, Data)]
enum TrackingState {
    Inactive,
    Active(String),
    Paused(String),
    Rest(String)
}

#[derive(Debug, Clone, Data)]
struct TrackingCtx {
    state: TrackingState,
    timestamp: Rc<DateTime<Utc>>,
    timer_id: Rc<TimerToken>,
    elapsed: Rc<chrono::Duration>,
}

#[derive(Clone, Data, Lens)]
struct AppModel {
    db: Rc<rusqlite::Connection>,
    tasks: TaskMap,
    records: TimeRecordMap,
    task_sums: TaskSums,
    tags: OrdSet<String>,
    focus: Vector<String>,
    tracking: TrackingCtx,
    selected_task: String,
    focus_filter: String,
    tag_filter: Option<String>,
}

static TASK_COLOR_BG: Color                 = Color::rgb8(80, 73, 69);
static APP_BORDER: Color                    = Color::rgb8(60, 56, 54);
static TASK_ACTIVE_COLOR_BG: Color          = Color::rgb8(250, 189, 47);

static UI_TIMER_INTERVAL: Duration = Duration::from_secs(1);

fn get_work_interval(_uid: &String) -> chrono::Duration {
    chrono::Duration::minutes(50)
    // chrono::Duration::seconds(10)
}

fn get_rest_interval(_uid: &String) -> chrono::Duration {
    chrono::Duration::minutes(10)
}

const COMMAND_TASK_NEW:    Selector            = Selector::new("tcmenu.task_new");
const COMMAND_TASK_START:  Selector<String>    = Selector::new("tcmenu.task_start");
const COMMAND_TASK_STOP:   Selector            = Selector::new("tcmenu.task_stop");
const COMMAND_TASK_PAUSE:   Selector           = Selector::new("tcmenu.task_pause");
const COMMAND_TASK_RESUME:   Selector<String>  = Selector::new("tcmenu.task_resume");
const COMMAND_TASK_ARCHIVE: Selector<String>   = Selector::new("tcmenu.task_archive");
const COMMAND_TASK_COMPLETED: Selector<String> = Selector::new("tcmenu.task_completed");

const COMMAND_DETAILS_REQUEST_FOCUS: Selector  = Selector::new("details_request_focus");

const SOUND_TASK_FINISH: &str = "res/bell.ogg";

const TASK_FOCUS_CURRENT: &str = "Current";
const TASK_FOCUS_COMPLETED: &str = "Completed";
const TASK_FOCUS_ALL: &str = "All";

const TASK_NAME_EDIT_WIDGET: WidgetId = WidgetId::reserved(9247);

impl AppModel {
    fn get_uids_filtered(&self) -> impl Iterator<Item = String> + '_ {
        self.tasks.keys().cloned().filter(move |uid| {
            let task = self.tasks.get(uid).expect("unknown uid");

            let focus_ok = match self.focus_filter.as_str() {
                TASK_FOCUS_CURRENT => {task.task_status == TaskStatus::NEEDS_ACTION ||
                              task.task_status == TaskStatus::IN_PROCESS},
                TASK_FOCUS_COMPLETED => task.task_status == TaskStatus::COMPLETED,
                TASK_FOCUS_ALL => task.task_status != TaskStatus::ARCHIVED,
                _ => panic!("Unknown focus filter {}", &self.focus_filter),
            };

            let tag_ok =
                if let Some(ref tag_filter) = self.tag_filter {
                    task.tags.contains(tag_filter)
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
            for tag in &task.tags {
                if task.task_status != TaskStatus::ARCHIVED {
                    self.tags.insert(tag.clone());
                }
            }
        }
    }
}

fn convert_ts(optstr: Option<String>) -> Vector<String> {
    match optstr {
        Some(st) => vector![st],
        None => Vector::new(),
    }
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

pub fn main() -> anyhow::Result<()> {

    let args: Vec<String> = env::args().collect();

    let conn = db::init()?;
    let db = Rc::new(conn);

    let file_path = match args.len() {
        // no arguments passed
        1 => String::from("/home/dc/Tasks.ics"),
        2 => args[1].clone(),
        _ => args[1].clone(),
    };

    let focus = vector![TASK_FOCUS_CURRENT.to_string(),
                        TASK_FOCUS_COMPLETED.to_string(),
                        TASK_FOCUS_ALL.to_string()];

    // let (tasks, tags) = parse_ical(file_path);

    let (tasks, tags) = db::get_tasks(db.clone())?;
    let records = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone());
        task_sums.insert(uid.clone(), sum);
    }

    let selected_task = "".to_string();

    let mut data = AppModel{
        db,
        tasks,
        records,
        task_sums,
        tags,
        focus,
        tracking: TrackingCtx{state: TrackingState::Inactive,
                              timestamp: Rc::new(Utc::now()),
                              timer_id: Rc::new(TimerToken::INVALID),
                              elapsed: Rc::new(chrono::Duration::zero())},
        selected_task: selected_task,
        focus_filter: TASK_FOCUS_CURRENT.to_string(),
        tag_filter: None
    };

    let selected = data.get_uids_filtered().nth(0).unwrap_or("".to_string()).clone();
    data.selected_task = selected;

    // TODO should be done in ctor
    data.update_tags();

    let main_window = WindowDesc::new(ui_builder())
        .window_size((1280.0, 800.0))
        .menu(make_menu)
        .title(LocalizedString::new("time-tracker-window-title").with_placeholder("Time tracker"));
    
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .expect("launch failed");

    Ok(())
}

fn resume_tracking(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer_id =
        Rc::new(ctx.request_timer(get_work_interval(&uid).checked_sub(&data.tracking.elapsed)
                                  .unwrap_or(chrono::Duration::zero()).to_std().unwrap()));
    data.tracking.state = TrackingState::Active(uid);
}

fn start_tracking(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.elapsed = Rc::new(chrono::Duration::zero());
    data.tracking.timer_id = Rc::new(ctx.request_timer(get_work_interval(&uid).to_std().unwrap()));
    data.tracking.state = TrackingState::Active(uid);
}

fn stop_tracking(data: &mut AppModel, new_state: TrackingState) {
    data.tracking.timer_id = Rc::new(TimerToken::INVALID);

    if let TrackingState::Inactive = &data.tracking.state {return};
    if let TrackingState::Rest(_) = &data.tracking.state {return};

    let task = match &data.tracking.state {
        TrackingState::Active(uid) => data.tasks.get(uid).unwrap(),
        TrackingState::Paused(uid) => data.tasks.get(uid).unwrap(),
        _ => panic!("bug: current task insn't active/paused"),
    };

    let now = Rc::new(Utc::now());
    let record = TimeRecord{from: data.tracking.timestamp.clone(), to: now.clone(),
                            uid: task.uid.clone()};

    if let Err(what) = db::add_time_record(data.db.clone(), &record) {
        println!("db error: {}", what);
    }

    let duration = now.signed_duration_since(data.tracking.timestamp.as_ref().clone());

    data.tracking.elapsed = Rc::new(duration);

    println!("Task '{}' duration: {}:{}:{}", &task.name,
             duration.num_hours(), duration.num_minutes(), duration.num_seconds());

    data.records.insert(*record.from, record.clone());
    add_record_to_sum(data.task_sums.get_mut(&task.uid).expect("unknown uid"), &record);

    data.tracking.state = new_state;
}

fn archive_task(model: &mut AppModel, uid: &String) {
    let task = model.tasks.get_mut(uid).expect(&format!("unknown task: {}", uid));
    task.task_status = TaskStatus::ARCHIVED;
    if let Err(what) = db::update_task(model.db.clone(), &task) {
        println!("db error: {}", what);
    }
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
    
    let mut task = make_task_menu(model, &model.selected_task);
    task = task.rebuild_on(|prev: &AppModel, now: &AppModel, _env: &Env| {
        !prev.tasks.same(&now.tasks) |
        !prev.selected_task.same(&now.selected_task) |
        !prev.tracking.same(&now.tracking)
    });

    base.entry(file).entry(task)
}


fn make_task_menu(d: &AppModel, current: &String) -> Menu<AppModel> {
    let mut result = Menu::new(LocalizedString::new("Task"));

    let new_task_entry = MenuItem::new(LocalizedString::new("New task"))
                .on_activate(
                    move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_NEW.with(()));
                    });

    if current.is_empty() {
        return result.entry(new_task_entry);
    }

    let start_entry = {
        let uid_for_closure = current.clone();
        MenuItem::new(LocalizedString::new("Start tracking")).on_activate(
            move |ctx, _d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_START.with(uid_for_closure.clone()));
            })
    };

    let pause_entry =
        MenuItem::new(LocalizedString::new("Pause")).on_activate(
            move |ctx, _d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_PAUSE.with(()));
            });

    let stop_entry = MenuItem::new(LocalizedString::new("Stop tracking")).on_activate(
        move |ctx, _d: &mut AppModel, _env| {
            ctx.submit_command(COMMAND_TASK_STOP.with(()));
        });

    let resume_entry = {
        let uid_for_closure = current.clone();
        MenuItem::new(LocalizedString::new("Resume")).on_activate(
            move |ctx, d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_RESUME.with(uid_for_closure.clone()));
            })
    };

    match &d.tracking.state {
        TrackingState::Active(uid) if current.eq(uid) =>
            result = result.entry(pause_entry).entry(stop_entry),

        TrackingState::Paused(uid) if current.eq(uid) =>
            result = result.entry(resume_entry).entry(stop_entry),

        TrackingState::Rest(uid) if current.eq(uid) =>
            result = result.entry(start_entry),

        _ =>
            result = result.entry(start_entry),
    };

    let uid_archive = current.clone();
    let uid_completed = current.clone();

    result
        .entry(
            MenuItem::new(LocalizedString::new("Mark completed"))
                .on_activate(
                    move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_COMPLETED.with(uid_completed.clone()));
                }),
        )
        .entry(
            new_task_entry,
        )
        .entry(
            MenuItem::new(LocalizedString::new("Archive")).on_activate(
                move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_ARCHIVE.with(uid_archive.clone()));
                },
            ),
        )
}

struct TaskListWidget {
    inner: WidgetPod<(AppModel, Vector<String>), List<(AppModel, String)>>
}

impl TaskListWidget {
    fn new() -> TaskListWidget {

        let inner = List::new(|| {

            let task_painter =
                Painter::new(|ctx: &mut PaintCtx, (shared, uid): &(AppModel, String), _env| {
                    let bounds = ctx.size().to_rect();
                    if shared.selected_task.eq(uid) {
                        ctx.fill(bounds, &TASK_COLOR_BG);
                    }

                    match shared.tracking.state {
                        TrackingState::Active(ref active) if uid.eq(active) => {
                            ctx.stroke(bounds, &TASK_ACTIVE_COLOR_BG, 4.0);
                            return;
                        },
                        _ => (),
                    };

                    ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                });

            let container = Container::new(
                Label::new(|(d, uid): &(AppModel, String), _env: &_| {
                    let task = d.tasks.get(uid).expect("unknown uid");
                    format!("{}", task.name)
                })
                 .expand_width()
                 .align_vertical(UnitPoint::LEFT)
                 .padding(10.0),
                )
                .on_click(|_ctx, (shared, uid): &mut (AppModel, String), _env| {
                        shared.selected_task = uid.clone();
                    })
                .background(task_painter);

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
                stop_tracking(&mut data.0, TrackingState::Inactive);
                start_tracking(&mut data.0, cmd.get(COMMAND_TASK_START).unwrap().clone(), ctx);
            },

            Event::Command(cmd) if cmd.is(COMMAND_TASK_STOP) => {
                stop_tracking(&mut data.0, TrackingState::Inactive);
            }

            Event::Command(cmd) if cmd.is(COMMAND_TASK_PAUSE) => {
                let uid = match &data.0.tracking.state {
                    TrackingState::Active(uid) => uid.clone(),
                    _ => panic!("state is not active"),
                };

                stop_tracking(&mut data.0, TrackingState::Paused(uid));
                data.0.tracking.timer_id = Rc::new(TimerToken::INVALID);
            }

           Event::Command(cmd) if cmd.is(COMMAND_TASK_RESUME) => {
               resume_tracking(&mut data.0, cmd.get(COMMAND_TASK_RESUME).unwrap().clone(), ctx);
            }

            Event::Command(cmd) if cmd.is(COMMAND_TASK_NEW) => {
                let uid = generate_uid();
                let task = Task::new("new task".to_string(), "".to_string(), uid.clone(), OrdSet::new(),
                                     0, TaskStatus::NEEDS_ACTION, 0);

                if let Err(what) = db::add_task(data.0.db.clone(), &task) {
                    println!("db error: {}", what);
                }

                data.0.selected_task = task.uid.clone();
                data.0.tasks.insert(uid.clone(), task);
                data.0.task_sums.insert(uid.clone(), TimePrefixSum::new());
                data.0.update_tags();
                ctx.submit_command(COMMAND_DETAILS_REQUEST_FOCUS.with(()));
                ctx.request_update();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_COMPLETED) => {
                let uid = cmd.get(COMMAND_TASK_COMPLETED).unwrap().clone();
                let mut task = data.0.tasks.get(&uid).expect("unknown uid").clone();
                task.task_status = TaskStatus::COMPLETED;

                match &data.0.tracking.state {
                    TrackingState::Active(cur) if cur.eq(&uid)
                        => stop_tracking(&mut data.0, TrackingState::Inactive),
                    TrackingState::Paused(cur) if cur.eq(&uid)
                        => stop_tracking(&mut data.0, TrackingState::Inactive),
                    TrackingState::Rest(cur) if cur.eq(&uid)
                        => stop_tracking(&mut data.0, TrackingState::Inactive),
                    _ => (),
                };

                data.0.tasks = data.0.tasks.update(uid, task);
                data.0.check_update_selected();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_ARCHIVE) => {
                archive_task(&mut data.0, cmd.get(COMMAND_TASK_ARCHIVE).unwrap());
                ctx.request_update();
            },
            Event::Timer(id) => {
                if *id == *data.0.tracking.timer_id {
                    play_sound(SOUND_TASK_FINISH.to_string());
                    stop_tracking(&mut data.0, TrackingState::Inactive);
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
                ctx.show_context_menu(make_task_menu(&data.0, &data.1), mouse.pos);
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
    if dur.num_days() > 0 {
        format!("{:02}d:{:02}h:{:02}m:{:02}s",
                dur.num_days(), dur.num_hours() % 24, dur.num_minutes() % 60, dur.num_seconds() % 60)
    } else if dur.num_hours() > 0 {
        format!("{:02}h:{:02}m:{:02}s", dur.num_hours(), dur.num_minutes() % 60, dur.num_seconds() % 60)
    } else if dur.num_minutes() > 0 {
        format!("{:02}m:{:02}s", dur.num_minutes() % 60, dur.num_seconds() % 60)
    } else {
        format!("{:02}s", dur.num_seconds())
    }
}

fn get_status_string(d: &AppModel) -> String {
    match d.tracking.state {
        TrackingState::Active(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            let duration = d.tracking.elapsed.checked_add(&Utc::now()
                .signed_duration_since(d.tracking.timestamp.as_ref().clone()))
                .unwrap_or(chrono::Duration::zero());

            let total = get_work_interval(uid);

            format!("Active task: '{}' | Elapsed: {} / {}",
                    active_task.name, format_duration(duration), format_duration(total))
        },
        TrackingState::Paused(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            let total = get_work_interval(uid);
            let elapsed = &d.tracking.elapsed;

            format!("Paused task: '{}' | Elapsed: {} / {}",
                    active_task.name,
                    format_duration(*(&d.tracking.elapsed).clone()), format_duration(total))
        },

        _ => format!("")
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

fn task_edit_widget() -> impl Widget<Task> {
    static FONT_CAPTION_DESCR: FontDescriptor =
        FontDescriptor::new(FontFamily::SYSTEM_UI)
        .with_weight(FontWeight::BOLD)
        .with_size(16.0);

    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Name").with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(
                EditableLabel::parse()
                    .with_id(TASK_NAME_EDIT_WIDGET)
                    .padding(10.0)
                    .fix_height(50.0)
                    .lens(lens::Identity.map(
                        |d: &Task| d.name.clone(),
                        |d: &mut Task, x: String| {
                            d.name = x;
                        },
                    ))),
    );

    column.add_spacer(10.0);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Status") .with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(Radio::new("needs action" , TaskStatus::NEEDS_ACTION))
            .with_child(Radio::new("in process"   , TaskStatus::IN_PROCESS))
            .with_child(Radio::new("completed"    , TaskStatus::COMPLETED))
            .with_child(Radio::new("cancelled"    , TaskStatus::CANCELLED))
            .lens(lens::Map::new(
                |task: &Task| task.task_status.clone(),
                |task: &mut Task, status| task.task_status = status))
    );

    column.add_spacer(15.0);

    let new_tag_edit = EditableLabel::parse()
            .lens(lens::Map::new(
                |_task: &Task| "".to_string(),
                |task: &mut Task, new_tag| {
                    if !new_tag.is_empty() {
                        task.tags = task.tags.update(new_tag);
                    }
                }));

    let tags_list =
        List::new(|| {
            Flex::row()
                .with_child(
                    Label::new(|(_, item) : &(OrdSet<String>, String), _env: &_| format!("{} ⌫", item))
                        .on_click(|_ctx, (lst, item): &mut (OrdSet<String>, String), _env| *lst = lst.without(item))
                        .align_horizontal(UnitPoint::LEFT)
                        .padding(10.0))
                .background(
                    Painter::new(|ctx: &mut PaintCtx, item: &_, _env| {
                        let bounds = ctx.size().to_rect();
                        ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                    }))

        })
        .with_spacing(10.0)
        .horizontal()
        .lens(lens::Identity.map(
            |data: &Task| {
                (data.tags.clone(),
                 data.tags.iter().map(|x: &String| {x.clone()}).collect())
            },
            |data: &mut Task, tags: (OrdSet<String>, Vector<String>)| {
                if !data.tags.same(&tags.0) {
                    data.tags = tags.0;
                }
            }));

    column.add_child(
        Flex::row()
            .with_child(Label::new("Tags").with_font(FONT_CAPTION_DESCR.clone()))
            .with_spacer(20.0)
            .with_child(new_tag_edit.padding(10.0))
            .with_default_spacer()
            .with_child(
                Scroll::new(tags_list)

            ));

    column.add_spacer(15.0);

    column.add_child(Label::new("Description").with_font(FONT_CAPTION_DESCR.clone()));
    column.add_default_spacer();
    column.add_child(
        EditableLabel::parse()
            .lens(lens::Identity.map(
                |d: &Task| d.description.clone(),
                |d: &mut Task, x: String| {
                    d.description = x;
                },
            )));

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

struct TaskDetailsController;

impl<T, W: Widget<T>> Controller<T, W> for TaskDetailsController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {

        match event {
            Event::Command(cmd) if cmd.is(COMMAND_DETAILS_REQUEST_FOCUS) => {
                let command = Command::new(editable_label::BEGIN_EDITING, (),
                                           Target::Widget(TASK_NAME_EDIT_WIDGET));
                ctx.submit_command(command);

            },

            _ => child.event(ctx, event, data, env),
        }
    }
}

fn task_details_widget() -> impl Widget<(Task, TimePrefixSum)> {
    static FONT_CAPTION_DESCR: FontDescriptor =
        FontDescriptor::new(FontFamily::SYSTEM_UI)
        .with_weight(FontWeight::BOLD)
        .with_size(16.0);

    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let edit_widget = task_edit_widget().lens(druid::lens!((Task, TimePrefixSum), 0));
    column.add_child(edit_widget);

    column.add_spacer(15.0);

    column.add_child(Label::new("Task time log").with_font(FONT_CAPTION_DESCR.clone()));
    column.add_default_spacer();
    column.add_child(
        Label::new(|(_, sum): &(Task, TimePrefixSum), _env: &_| {
            let mut result = String::new();

            let now = Local::now();
            let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));

            let epoch = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
            let total_day = get_total_time(sum, &day_start);

            let total = get_total_time(sum, &epoch);

            result.push_str(&format!("Today: {}\n", format_duration(total_day.clone())));

            Utc.from_local_datetime(
                &NaiveDate::from_isoywd(now.year(), now.iso_week().week(), Weekday::Mon)
                .and_time(NaiveTime::from_hms(0,0,0)))
                .single()
                .map(|utc| result.push_str(&format!("Week: {}\n",
                                                    format_duration(get_total_time(sum, &utc)))));

            Utc.from_local_datetime(
                &NaiveDate::from_ymd(now.year(), now.month(), 1)
                .and_time(NaiveTime::from_hms(0, 0, 0)))
                .single()
                .map(|utc| result.push_str(&format!("Month: {}\n",
                                                    format_duration(get_total_time(sum, &utc)))));

            result.push_str(&format!("Total: {}", format_duration(total.clone())));

            return result;
        })
        .padding(10.0)
        .background(
            Painter::new(|ctx: &mut PaintCtx, item: &_, _env| {
                let bounds = ctx.size().to_rect();
                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
            }))
    );

    return column.controller(TaskDetailsController);
}

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    let mut tasks_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI)
    .with_weight(FontWeight::BOLD)
    .with_size(18.0);

    focus_column.add_default_spacer();
    focus_column.add_flex_child(Label::new("Focus")
                                .with_font(FONT_CAPTION_DESCR.clone())
                                .padding(10.0),
                                1.0);

    focus_column.add_default_spacer();

    focus_column.add_child(
        List::new(|| {
            Container::new(
                Label::new(|item: &(AppModel, String), _env: &_| format!("{}", item.1))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .background(
                        Painter::new(|ctx: &mut PaintCtx, (shared, id): &(AppModel, String), _env| {
                            let bounds = ctx.size().to_rect();
                            if shared.focus_filter.eq(id) {
                                ctx.fill(bounds, &TASK_COLOR_BG);
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
        })
        .with_spacing(10.0)
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
    focus_column.add_child(Label::new("Tags")
                           .with_font(FONT_CAPTION_DESCR.clone())
                           .padding(10.0));

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
                                ctx.fill(bounds, &TASK_COLOR_BG);
                            }
                            else {
                                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                            }
                        })
                    )
            )
                .on_click(|_ctx, (data, what): &mut (AppModel, String), _env| {
                    data.tag_filter = match data.tag_filter {
                        Some(ref filter) if filter.eq(what) => None,
                        Some(_)                             => Some(what.clone()),
                        None                                => Some(what.clone())
                    };

                    data.check_update_selected();
                })
        })
        .with_spacing(10.0))
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.tags.iter().map(|x : &String| {x.clone()}).collect()),
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

    tasks_column.add_child(
        Maybe::new(
            || task_details_widget().boxed(),
            || SizedBox::empty().expand_width().boxed(),
        )
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel|
                match (d.tasks.get(&d.selected_task).map_or(None, |r| Some(r.clone())),
                       d.task_sums.get(&d.selected_task).map_or(TimePrefixSum::new(), |r| r.clone()))
                {
                    (Some(task), time) => Some((task, time)),
                    _ => None,
                },

                |d: &mut AppModel, x: Option<(Task, TimePrefixSum)>| {
                    if let Some((mut new_task, _)) = x {
                        if let Some(prev) = d.tasks.get(&d.selected_task) {
                            if !prev.same(&new_task) {
                                if let Err(what) = db::update_task(d.db.clone(), &new_task) {
                                    println!("db error: {}", what);
                                }

                                new_task.seq += 1;

                                d.tasks = d.tasks.update(d.selected_task.clone(), new_task);
                                d.check_update_selected();
                                d.update_tags();
                            }
                        }
                    }
                },
            )),
    );


    main_row.add_flex_child(tasks_column
                            .padding(10.0)
                            .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0),
                            2.0);

    let mut time_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    time_column.add_child(Label::new("Total time log")
                          .with_font(FONT_CAPTION_DESCR.clone())
                          .padding(10.0));

    time_column.add_child(
        Label::new(|model: &AppModel, _env: &_| {
            let mut result = String::new();

            let now = Local::now();
            let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));

            let epoch = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
            let total_day = get_total_time_from_sums(&model.task_sums, &day_start);

            let total = get_total_time_from_sums(&model.task_sums, &epoch);

            result.push_str(&format!("Today: {}\n", format_duration(total_day.clone())));

            Utc.from_local_datetime(
                &NaiveDate::from_isoywd(now.year(), now.iso_week().week(), Weekday::Mon)
                .and_time(NaiveTime::from_hms(0,0,0)))
                .single()
                .map(|utc| result.push_str(
                    &format!("Week: {}\n",
                             format_duration(get_total_time_from_sums(&model.task_sums, &utc)))));

            Utc.from_local_datetime(
                &NaiveDate::from_ymd(now.year(), now.month(), 1)
                .and_time(NaiveTime::from_hms(0, 0, 0)))
                .single()
                .map(|utc| result.push_str(
                    &format!("Month: {}\n",
                             format_duration(get_total_time_from_sums(&model.task_sums, &utc)))));

            result.push_str(&format!("All time: {}", format_duration(total.clone())));

            result
        })
        .padding(10.0)
        .lens(lens::Identity.map(
                    |m: &AppModel| m.clone(),
                    |_data: &mut AppModel, _m: AppModel| {},
        )));

    time_column.add_default_spacer();

    time_column.add_flex_child(
        Scroll::new(
            List::new(||{
                Label::new(|(model, record): &(AppModel, TimeRecord), _env: &_| {

                    if let Some(task) = model.tasks.get(&record.uid) {
                        let now: DateTime<Local> = DateTime::from(SystemTime::now());
                        let when: DateTime<Local> = DateTime::<Local>::from(*record.from);
                        let duration = format_duration(record.to.signed_duration_since(*record.from));

                        let time =
                            if now.year() > when.year() || now.day() > when.day() {
                                when.format("%b %-d %H:%M").to_string()
                            } else {
                                when.format("%H:%M").to_string()
                            };

                        format!("{} | {} @ {}", duration, task.name, time)
                    } else {
                        "".to_string()
                    }
                })

                // .background(
                //     Painter::new(|ctx: &mut PaintCtx, item: &_, _env| {
                //         let bounds = ctx.size().to_rect();
                //         ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                //     }))
            })
                .with_spacing(10.0)
                .padding(10.0)
                .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0)
                .lens(lens::Identity.map(
                    |m: &AppModel| (m.clone(), m.records.values().map(|v| v.clone()).rev().collect()),
                    |_data: &mut AppModel, _m: (AppModel, Vector<TimeRecord>)| {},
                ))
        ), 1.0);

    main_row.add_child(time_column);

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

    root.with_child(Container::new(StatusBar::new()
                                   .align_horizontal(UnitPoint::CENTER))
                    .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0),
    )
        // .debug_paint_layout()
}

