use druid::im::{Vector, OrdSet};
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
    pub focus: Vector<String>,
    pub tracking: TrackingCtx,
    pub selected_task: String,
    pub focus_filter: String,
    pub tag_filter: Option<String>,
}
