use druid::im::{OrdSet};
use druid::{Data, TimerToken, Lens,};

use chrono::prelude::*;
use std::rc::Rc;

use crate::task::*;

#[derive(Debug, Clone, Data)]
pub enum TrackingState {
    Inactive,
    Active(String),
    Paused(String),
    Break(String)
}

#[derive(Debug, Clone, PartialEq, Data)]
pub enum FocusFilter {
    Current,
    Completed,
    All
}

impl FocusFilter {
    pub fn as_str(&self) -> &str {
        match &self {
            FocusFilter::Current => "Current",
            FocusFilter::Completed => "Completed",
            FocusFilter::All => "All",
        }
    }
}

#[derive(Debug, Clone, Data)]
pub struct TrackingCtx {
    pub state: TrackingState,
    pub timestamp: Rc<DateTime<Utc>>,
    pub timer_id: Rc<TimerToken>,
    pub elapsed: Rc<chrono::Duration>,
}

#[derive(Clone, Data, Lens)]
pub struct AppModel {
    pub db: Rc<rusqlite::Connection>,
    pub tasks: TaskMap,
    pub records: TimeRecordMap,
    pub task_sums: TaskSums,
    pub tags: OrdSet<String>,
    pub tracking: TrackingCtx,
    pub selected_task: String,
    pub focus_filter: FocusFilter,
    pub tag_filter: Option<String>,
}

pub fn get_work_interval(model: &AppModel, uid: &String) -> chrono::Duration {
    *model.tasks.get(uid).unwrap().work_duration.clone()
    // chrono::Duration::seconds(10)
}

pub fn get_rest_interval(model: &AppModel, uid: &String) -> chrono::Duration {
    *model.tasks.get(uid).unwrap().break_duration.clone()
    // chrono::Duration::seconds(10)
}

