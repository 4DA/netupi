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
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, List, Scroll};
use druid::{
    AppLauncher, Color, Data, Lens, LocalizedString, UnitPoint, Widget, WidgetExt, WindowDesc,
};

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


fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}

type PropertyMap = HashMap<String, Rc<Property>>;

#[derive(Debug, Clone, Data)]
struct TrackerTodo {
    properties: PropertyMap,
    alarms: Vector<Rc<IcalAlarm>>
}

type TodoMap = HashMap<String, TrackerTodo>;

#[derive(Debug, Clone, Data)]
struct TrackingState {
    active: bool,
    task_id: u32,
    timestamp: Instant
}

#[derive(Clone, Data)]
struct ViewState {
    filterByTag: String,
    filterByRelevance: String
}




#[derive(Clone, Data, Lens)]
struct AppModel {
    tasks: Vector<Task>,
    tags: Vector<String>,
    focus: Vector<String>,
    todos: TodoMap,
    cal: Rc<IcalCalendar>,
    tracking: TrackingState,
    view: ViewState
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
    timestamps: Vector<String>
}

impl Task {
    fn new(name: String, description: Option<String>,
           uid: String, categories: Vector<String>,
           priority: u32, status: Option<String>, seq: u32,
           timestamps: Vector<String>) -> Task {
        return Task{name, description, uid, categories, priority, status, seq, timestamps};
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

fn todos_by_uid(todo_vec: &Vec<IcalTodo>) -> TodoMap {
    let mut result = TodoMap::new();

    for task in todo_vec {
        let properties = props_by_name(&task.properties);

        result.insert(properties.get("UID").unwrap().value.clone().unwrap(),
                      TrackerTodo{properties, alarms: Vector::new()});
    }

    return result;
}

fn parse_ical() -> (TodoMap, IcalCalendar, Vector<Task>, OrdSet<String>) {
    let buf = BufReader::new(File::open("/home/dc/Tasks.ics")
        .unwrap());

    let mut reader = ical::IcalParser::new(buf);

    let mut tasks = Vector::new();
    let mut tags = OrdSet::new();

    let ical = reader.next().unwrap().unwrap();

    let tracker_todos = todos_by_uid(&ical.todos);
    println!("todos: {:?}", tracker_todos);

    for (uid, todo) in &tracker_todos {
        // println!("{}", type_of(&todo.properties));
        let mut summary = String::new();
        let mut description = None;
        let mut uid = String::new();
        let mut categories = Vector::new();
        let mut priority = 0;
        let mut status = None;
        let mut seq = 0;
        let mut timestamps = Vector::new();

        for (name, property) in &todo.properties {
            // println!("{}", property);
            // println!("{}", type_of(&property));

            match name.as_ref() {
                "UID" => {uid = property.value.as_ref().unwrap().clone();}
                "SUMMARY" => {summary = property.value.as_ref().unwrap().clone();}
                "DESCRIPTION" => {description = property.value.clone();}
                "CATEGORIES" => {
                    if (property.value.is_some()) {
                        categories.insert(0,  property.value.as_ref().unwrap().clone());
                        tags.insert(property.value.as_ref().unwrap().clone());
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
                "TIMESTAMPS" => {
                    if (property.value.is_some()) {
                        timestamps =
                            convert_ts(property.value.clone());
                    }
                }
                _ => {}
            }
        }

        let task = Task::new(summary, description, uid, categories, priority, status, seq, timestamps);
        // println!("{:?}", task);
        tasks.insert(0, task);
    }


    // let tags = vector![String::from("computer"), String::from("outside")];

    return (tracker_todos, ical, tasks, tags);
}

fn update_ical(ical: &mut IcalCalendar, todo_map: &TodoMap) {

}

fn re_emit() {
    let filename = "/home/dc/Tasks.ics";

    let input = BufReader::new(File::open(filename).unwrap());
    let mut reader = ical::IcalParser::new(input);
    let generated = reader.next().unwrap().ok().unwrap().generate();
    fs::write("/home/dc/Tasks-generated.ics", generated).expect("Unable to write Tasks-generated.ics");
}

pub fn main() {
    let focus = vector![String::from("todo"), String::from("active"), String::from("done"), String::from("all") ];


    let (todos, ical, tasks, tags) = parse_ical();

    let data = AppModel{
        tasks,
        tags: tags.iter().map(|x : &String| {x.clone()}).collect(),
        focus,
        todos: todos,
        cal: Rc::new(ical),
        tracking: TrackingState{active: false, task_id: 0, timestamp: Instant::now()},
        view: ViewState{filterByTag: String::from(""), filterByRelevance: String::from("")}
    };

    
    re_emit();
    let main_window = WindowDesc::new(ui_builder())
        .title(LocalizedString::new("list-demo-window-title").with_placeholder("List Demo"));
    
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .expect("launch failed");
}

fn start_tracking(data: &mut AppModel, uid: &String) {
    data.tracking.active = true;
    data.tracking.timestamp = Instant::now();
    println!("started tracking");
}

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);


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
                .background(Color::rgb(0.5, 0.5, 0.5))
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
                .background(Color::rgb(0.5, 0.5, 0.5))
        }))
        .vertical()
        .lens(AppModel::tags),
        1.0,
    );

    main_row.add_flex_child(focus_column, 0.5);


    // Build a list with shared data
    main_row.add_flex_child(
        Scroll::new(
            List::new(|| {
                Flex::row()
                    .with_child(
                        Label::new(|(d, uid): &(AppModel, String), _env: &_| {
                            format!("{}", uid)
                            // let id = *item as usize;
                            // format!("{} | dsc: {:?} | cats: {:?} | pri: {} | sta: {:?} | seq: {}",
                            //         d.tasks[id].name, d.tasks[id].description, d.tasks[id].categories,
                            //         d.tasks[id].priority, d.tasks[id].status, d.tasks[id].seq)
                        })
                        .align_vertical(UnitPoint::LEFT),
                    )
                    .with_flex_spacer(1.0)
                    .with_child(
                        Button::new("Start tracking")
                            .on_click(|_ctx, (shared, item): &mut (AppModel, String), _env| {
                                // We have access to both child's data and shared data.
                                // Remove element from right list.
                                // shared.retain(|v| v != item);
                                start_tracking(shared, &item);
                            })
                            .fix_size(120.0, 20.0)
                            .align_vertical(UnitPoint::CENTER),
                    )
                    .padding(10.0)
                    .background(Color::rgb(0.5, 0.0, 0.5))
                    .fix_height(50.0)
            })
            .with_spacing(10.),
        )
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.todos.keys().cloned().collect()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        )),
        1.0,
    );

    root.add_flex_child(main_row, 1.0);

    root.with_child(Label::new(|d: &AppModel, _env: &_| {
        if (d.tracking.active) {
            format!("Elapsed: {:?}", Instant::now().duration_since(d.tracking.timestamp))
        }
        else {
            String::from("Inactive")
        }
    }).align_horizontal(UnitPoint::RIGHT))
}

