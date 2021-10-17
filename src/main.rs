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

use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};
use druid::lens::{self, LensExt};
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, List, Scroll, Container};
use druid::{
    AppLauncher, Color, Data,
    FontWeight, FontDescriptor, FontFamily,
    Lens, LocalizedString, theme, UnitPoint, Widget, WidgetExt, WindowDesc};

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

use std::env;

// uid stuff
use uuid::v1::{Timestamp, Context};
use uuid::Uuid;

// chrono
use chrono::prelude::*;

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
    timestamp: Rc<DateTime<Utc>>
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
}


#[derive(Debug, Clone, Data)]
struct Task {
    name: String,
    description: Option<String>,
    uid: String,
    categories: Vector<String>,
    priority: u32,
    status: Option<String>,
    seq: u32,
    time_records: Vector<TimeRecord>,
}

#[derive(Debug, Clone, Data)]
struct TimeRecord {
    from: Rc<DateTime<Utc>>,
    to: Rc<DateTime<Utc>>,
}

impl Task {
    fn new(name: String, description: Option<String>,
           uid: String, categories: Vector<String>,
           priority: u32, status: Option<String>, seq: u32,
           time_records: Vector<TimeRecord>) -> Task {
        return Task{name, description, uid, categories, priority, status, seq, time_records};
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
    let mut description = None;
    let mut uid = String::new();
    let mut categories = Vector::new();
    let mut priority = 0;
    let mut status = None;
    let mut seq = 0;
    let mut time_records = Vector::new();

    for property in &ical_todo.properties {
        // println!("{}", property);
        // println!("{}", type_of(&property));

        match property.name.as_ref() {

            "UID" => {uid = property.value.as_ref().unwrap().clone();}
            "SUMMARY" => {summary = property.value.as_ref().unwrap().clone();}
            "DESCRIPTION" => {description = property.value.clone();}
            "CATEGORIES" => {
                if (property.value.is_some()) {
                    categories.insert(0,  property.value.as_ref().unwrap().clone());
                }
            }
            "STATUS" => {status = property.value.clone();}
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

fn emit(cal: &IcalCalendar) {
    let generated = cal.generate();
    fs::write("/home/dc/Tasks-generated.ics", generated).expect("Unable to write Tasks-generated.ics");
}

pub fn main() {

    let args: Vec<String> = env::args().collect();

    let file_path = match args.len() {
        // no arguments passed
        1 => String::from("/home/dc/Tasks.ics"),
        2 => args[1].clone(),
        _ => args[1].clone(),
    };

    let focus = vector![String::from("todo"), String::from("active"), String::from("done"), String::from("all") ];


    let (tasks, tags) = parse_ical(file_path);

    let null_uid = "".to_string();
    let selected_task = tasks.keys().nth(0).unwrap_or(&null_uid).clone();

    let data = AppModel{
        tasks,
        tags: tags.iter().map(|x : &String| {x.clone()}).collect(),
        focus,
        tracking: TrackingState{active: false, task_uid: "".to_string(), timestamp:
                                Rc::new(Utc::now())},
        view: ViewState{filterByTag: String::from(""), filterByRelevance: String::from("")},
        selected_task,
    };

    let main_window = WindowDesc::new(ui_builder())
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

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    let mut tasks_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    static TASK_COLOR_BG: Color = Color::rgb8(127, 0, 127);

    static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI)
    .with_weight(FontWeight::BOLD)
    .with_size(18.0);

    focus_column.add_default_spacer();
    focus_column.add_flex_child(Label::new("Focus"), 1.0);
    focus_column.add_default_spacer();

    focus_column.add_child(
        Scroll::new(List::new(|| {
            Label::new(|item: &String, _env: &_| format!("{}", item))
                .align_vertical(UnitPoint::LEFT)
                .padding(10.0)
                .expand()
                .height(30.0)
                .background(TASK_COLOR_BG.clone())
        }))
        .vertical()
        .lens(AppModel::focus)
    );

    focus_column.add_default_spacer();
    focus_column.add_child(Label::new("Tags"));
    focus_column.add_default_spacer();

    focus_column.add_flex_child(
        Scroll::new(List::new(|| {
            Label::new(|item: &String, _env: &_| format!("{}", item))
                .align_vertical(UnitPoint::LEFT)
                .padding(10.0)
                .expand()
                .height(30.0)
                .background(TASK_COLOR_BG.clone())
        }))
        .vertical()
        .lens(AppModel::tags),
        1.0,
    );

    main_row.add_flex_child(focus_column, 0.5);

    let tasks_scroll = Scroll::new(
            List::new(|| {
                Container::new(
                    Label::new(|(d, uid): &(AppModel, String), _env: &_| {
                            let task = d.tasks.get(uid).expect("unknown uid");
                            format!("{}", task.name)
                    })
                        .expand()
                        .on_click(|_ctx, (shared, uid): &mut (AppModel, String), _env| {
                            shared.selected_task = uid.clone();
                        })
                        .align_vertical(UnitPoint::LEFT),
                    )
                    .background(TASK_COLOR_BG.clone())
                    .fix_height(50.0)
            })
            .with_spacing(10.),
        )
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.tasks.keys().cloned().collect()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        ));

    // Build a list with shared data
    tasks_column.add_flex_child(tasks_scroll, 2.0);

    tasks_column.add_spacer(30.0);
    tasks_column.add_child(Flex::row()
                           .with_child(
                               Button::new("Start tracking")
                                   .on_click(|_ctx, shared: &mut AppModel, _env| {
                                       start_tracking(shared, shared.selected_task.clone());
                                   })
                                   .fix_size(120.0, 20.0)
                                   .align_vertical(UnitPoint::CENTER),
                           )
                           .with_default_spacer()
                           .with_child(
                               Button::new("Stop tracking")
                                   .on_click(|_ctx, shared: &mut AppModel, _env| {
                                       stop_tracking(shared);
                                   })
                                   .fix_size(120.0, 20.0)
                                   .align_vertical(UnitPoint::CENTER),
                           ));

    tasks_column.add_spacer(10.0);

    tasks_column.add_flex_child(
        Label::new(|(d): &(AppModel), _env: &_| {
            if d.selected_task.eq("") {
                return "".to_string();
            }
            d.tasks.get(&d.selected_task).expect("unknown uid").name.clone()
        })
        .with_font(FONT_CAPTION_DESCR.clone())
        .padding(10.0)
        .background(TASK_COLOR_BG.clone())
        .fix_height(50.0),
        1.0
    );

    tasks_column.add_spacer(10.0);

    tasks_column.add_flex_child(
        Label::new(|(d): &(AppModel), _env: &_| {
            if d.selected_task.eq("") {
                return "".to_string();
            }

            let task = &d.tasks.get(&d.selected_task).expect("unknown uid");

            if let Some(text) = &task.description {
                return format!("{}", text);
            }
            else {
                return "".to_string();
            }

        })
        .padding(10.0)
        .background(TASK_COLOR_BG.clone())
        .fix_height(50.0),
        1.0
    );

    tasks_column.add_spacer(10.0);

    tasks_column.add_flex_child(
        Label::new(|d: &AppModel, _env: &_| {
            if d.selected_task.eq("") {
                return "".to_string();
            }

            let task = &d.tasks.get(&d.selected_task).expect("unknown uid");
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


    main_row.add_flex_child(tasks_column, 1.0);

    root.add_flex_child(main_row, 1.0);

    root.add_child(
        Button::new("Save")
            .on_click(|_ctx, (model): &mut (AppModel), _env| {
                // todo dont clone IcalCalendar
                // let newcal = update_ical(&mut IcalCalendar::clone(&model.cal), &model.tasks);
                // emit(&newcal);
                // model.cal = Rc::new(newcal)
            })
            .fix_size(120.0, 20.0)
            .align_vertical(UnitPoint::CENTER),
    );

    root.add_child(
        Button::new("Add")
            .on_click(|_ctx, (model): &mut (AppModel), _env| {
                let uid = generate_uid();
                let task = Task::new("new task".to_string(), None, uid.clone(), Vector::new(),
                                     0, None, 0, Vector::new());

                model.tasks.insert(uid, task);
                // let mut newtask = IcalTodo::new();
                // let uid = generate_uid();
                // assert_eq!(model.tasks.contains_key(&uid), false, "VTODOs can't have duplicate UIDs");

                // let mut props = PropertyMap::new();
                // props.insert("UID".to_string(), Rc::new(ical_property!("UID", uid.clone())));
                // props.insert("SUMMARY".to_string(),
                //              Rc::new(ical_property!("SUMMARY", "this is new task")));                

                // model.tasks.insert(uid.clone(),
                //                    TrackerTodo{properties: props,
                //                                alarms: Vector::new()});

                // println!("type = {:?}", type_of(newtask));
            })
            .fix_size(120.0, 20.0)
            .align_vertical(UnitPoint::CENTER),
    );

    root.with_child(Label::new(|d: &AppModel, _env: &_| {
        if d.tracking.active {
            let active_task = &d.tasks.get(&d.tracking.task_uid).expect("unknown uid");
            format!("Started tracking '{}' at {:?}", active_task.name, d.tracking.timestamp)
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
    }).align_horizontal(UnitPoint::RIGHT))
        .debug_paint_layout()
}

