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

//! Demos basic list widget and list manipulations.

// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use druid::im::{vector, Vector};
use druid::lens::{self, LensExt};
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, List, Scroll};
use druid::{
    AppLauncher, Color, Data, Lens, LocalizedString, UnitPoint, Widget, WidgetExt, WindowDesc,
};

extern crate ical;
use ical::generator;

use std::io::BufReader;
use std::fs::File;

use std::any::type_name;

use std::rc::Rc;
use crate::ical::{generator::*, *};
use std::fs;

fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}

#[derive(Clone, Data, Lens)]
struct AppData {
    tasks: Vector<Task>
}

#[derive(Debug, Clone, Data)]
struct Task {
    name: String,
    description: Option<String>,
    uid: String,
    categories: Vector<String>
}

impl Task {
    fn new(name: String, description: Option<String>,
           uid: String, categories: Vector<String>) -> Task {
        return Task{name: name, description: description, uid: uid, categories: categories};
    }
}

fn parse_ical() -> Vector<Task> {
    let buf = BufReader::new(File::open("/home/dc/Tasks.ics")
        .unwrap());

    let reader = ical::IcalParser::new(buf);

    let mut result = Vector::new();

    for line in reader {
        let ical = line.unwrap();
        for todo in ical.todos {
            // println!("{}", type_of(&todo.properties));
            let mut summary = String::new();
            let mut description = None;
            let mut uid = String::new();
            let mut categories = Vector::new();

            for property in &todo.properties {
                // println!("{}", property);
                // println!("{}", type_of(&property));

                match property.name.as_ref() {
                    "UID" => {uid = property.value.as_ref().unwrap().clone();}
                    "SUMMARY" => {summary = property.value.as_ref().unwrap().clone();}
                    "DESCRIPTION" => {description = property.value.clone();}
                    "CATEGORIES" => {
                        if (property.value.is_some()) {
                            categories.insert(0,  property.value.as_ref().unwrap().clone())}
                    }
                    _ => {}
                }
            }
            
            let task = Task::new(summary, description, uid, categories);
            // println!("{:?}", task);
            result.insert(0, task);
        }
    }

    return result;
}

fn re_emit() {
    let filename = "/home/dc/Tasks.ics";

    let input = BufReader::new(File::open(filename).unwrap());
    let mut reader = ical::IcalParser::new(input);
    let generated = reader.next().unwrap().ok().unwrap().generate();
    fs::write("/home/dc/Tasks-generated.ics", generated).expect("Unable to write Tasks-generated.ics");
}

pub fn main() {
    let tasks = parse_ical();
    re_emit();
    let main_window = WindowDesc::new(ui_builder())
        .title(LocalizedString::new("list-demo-window-title").with_placeholder("List Demo"));

    // Set our initial data
    let data = AppData {
        tasks,
    };
    
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .expect("launch failed");
}

fn ui_builder() -> impl Widget<AppData> {
    let mut root = Flex::column();

    // Build a button to add children to both lists
    root.add_child(
        Button::new("Add")
            .on_click(|_, data: &mut AppData, _| {
                // Add child to left list

            })
            .fix_height(30.0)
            .expand_width(),
    );

    let mut lists = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    // Build a list with shared data
    lists.add_flex_child(
        Scroll::new(
            List::new(|| {
                Flex::row()
                    .with_child(
                        Label::new(|(tasks, item): &(Vector<Task>, u32), _env: &_| {
                            let id = *item as usize;
                            format!("{} | {:?} | {:?}", tasks[id].name, tasks[id].description, tasks[id].categories)
                        })
                        .align_vertical(UnitPoint::LEFT),
                    )
                    .with_flex_spacer(1.0)
                    .with_child(
                        Button::new("Start tracking")
                            .on_click(|_ctx, (shared, item): &mut (Vector<Task>, u32), _env| {
                                // We have access to both child's data and shared data.
                                // Remove element from right list.
                                // shared.retain(|v| v != item);
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
            |d: &AppData| (d.tasks.clone(), (0 .. d.tasks.len() as u32).collect()),
            |d: &mut AppData, x: (Vector<Task>, Vector<u32>)| {
                // If shared data was changed reflect the changes in our AppData
                d.tasks = x.0
            },
        )),
        1.0,
    );

    root.add_flex_child(lists, 1.0);

    root.with_child(Label::new("horizontal list"))
}

