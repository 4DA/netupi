use std::io::BufReader;
use std::fs::File;
use std::fs;
use std::rc::Rc;

// ical stuff
// extern crate ical;
use ical::{property::Property, generator::Emitter};
use ical::parser::ical::component::{IcalTodo, IcalAlarm, IcalCalendar, IcalEvent, IcalTimeZone};

// use ical::generator::*;
// use ical::{generator::*, *, parser::*};

use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};

use chrono::prelude::*;

use crate::task::*;

type PropertyMap = HashMap<String, Rc<Property>>;
type ImportResult<T> = std::result::Result<T, String>;

pub fn parse_ical(file_path: String) -> (TaskMap, OrdSet<String>) {
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

        for tag in &task.tags {
            tags.insert(tag.clone());
        }

        task_map.insert(task.uid.clone(), task);
    }

    // let tags = vector![String::from("computer"), String::from("outside")];

    return (task_map, tags);
}


fn parse_todo(ical_todo: &IcalTodo) -> ImportResult<Task> {
    let mut summary = String::new();
    let mut description = String::new();
    let mut uid = String::new();
    let mut tags = OrdSet::new();
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
                    tags.insert(property.value.as_ref().unwrap().clone());
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

    return Ok(Task::new(summary, description, uid, tags, priority, status, seq, time_records));
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

fn parse_time_records(optsrc: &Option<String>) -> Vector<TimeRecord> {
    let mut result = Vector::new();

    let split = optsrc.as_ref().unwrap().split(";");

    for s in split {
        let res = Utc.datetime_from_str(&s, "%Y-%m-%d %H:%M:%S");
    }

    return result;
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
