use druid::im::{OrdSet, Vector};

use druid::{Data, TimerToken, Lens };

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

impl AppModel {
    pub fn get_uids_filtered(&self) -> impl Iterator<Item = String> + '_ {
        self.tasks.keys().cloned().filter(move |uid| {
            let task = self.tasks.get(uid).expect("unknown uid");

            let focus_ok = match self.focus_filter {
                FocusFilter::Current => {task.task_status == TaskStatus::NeedsAction ||
                              task.task_status == TaskStatus::InProcess},
                FocusFilter::Completed => task.task_status == TaskStatus::Completed,
                FocusFilter::All => task.task_status != TaskStatus::Archived,
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

    pub fn check_update_selected(&mut self) {
        let filtered: Vector<String> = self.get_uids_filtered().collect();

        // select any task if currently selected is filtered out
        if !filtered.contains(&self.selected_task) {
            self.selected_task = filtered.front().unwrap_or(&"".to_string()).clone();
        }
    }

    pub fn get_tags(&self) -> OrdSet<String> {
        let mut result = OrdSet::new();

        for (_, task) in self.tasks.iter() {
            for tag in &task.tags {
                if task.task_status != TaskStatus::Archived {
                    result.insert(tag.clone());
                }
            }
        }

        return result;
    }

    pub fn update_tags(&mut self) {
        self.tags.clear();
        self.tags = self.get_tags();
    }
}

