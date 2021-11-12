use std::rc::Rc;
use druid::lens::{self, LensExt};
use druid::{Data, Lens};
use chrono::prelude::*;
use druid::im::{vector, Vector, ordset, OrdSet, OrdMap, HashMap};

pub type TaskMap = HashMap<String, Task>;

#[derive(Debug, Clone, Data, PartialEq)]
pub enum TaskStatus {
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
pub struct Task {
    pub name: String,
    pub description: String,
    pub uid: String,
    pub tags: OrdSet<String>,
    pub priority: u32,
    pub task_status: TaskStatus,
    pub seq: u32,
    pub time_records: Vector<TimeRecord>,
}

#[derive(Debug, Clone, Data)]
pub struct TimeRecord {
    pub from: Rc<DateTime<Utc>>,
    pub to: Rc<DateTime<Utc>>,
}

impl Task {
    pub fn new(name: String, description: String,
           uid: String, tags: OrdSet<String>,
           priority: u32, task_status: TaskStatus, seq: u32,
           time_records: Vector<TimeRecord>) -> Task {
        return Task{name, description, uid, tags, priority, task_status, seq, time_records};
    }
}
