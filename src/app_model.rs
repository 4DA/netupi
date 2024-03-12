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
    Status(TaskStatus),
    All
}

impl FocusFilter {
    pub fn to_string(&self) -> &str {
        match &self {
            FocusFilter::Status(x) => x.to_string(),
            FocusFilter::All => "All",
        }
    }

    pub fn to_int(&self) -> u8 {
        use FocusFilter::*;
        match &self {
            Status(TaskStatus::NeedsAction) => 0,
            Status(TaskStatus::Completed) => 1,
            Status(TaskStatus::InProcess) => 2,
            Status(TaskStatus::Archived) => 3,
            All => 4,
        }
    }

    pub fn cycle_next(&self) -> Self {
        use FocusFilter::*;
        match self {
            Status(TaskStatus::NeedsAction) => Status(TaskStatus::Completed),
            Status(TaskStatus::Completed) => Status(TaskStatus::InProcess),
            Status(TaskStatus::InProcess) => Status(TaskStatus::Archived),
            Status(TaskStatus::Archived) => All,
            All => Status(TaskStatus::NeedsAction)
        }
    }

    pub fn cycle_prev(&self) -> Self {
        use FocusFilter::*;
        match self {
            Status(TaskStatus::NeedsAction) => All,
            Status(TaskStatus::Completed) => Status(TaskStatus::NeedsAction),
            Status(TaskStatus::InProcess) => Status(TaskStatus::Completed),
            Status(TaskStatus::Archived) => Status(TaskStatus::InProcess),
            All => Status(TaskStatus::Archived)
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
    pub records_killed: Rc<TimeRecordSet>,
    pub task_sums: TaskSums,
    pub tags: OrdSet<String>,
    pub tracking: TrackingCtx,
    pub selected_task: Option<String>,
    pub focus_filter: FocusFilter,
    pub tag_filter: Option<String>,
    pub hot_log_entry: Option<Rc<DateTime<Utc>>>,

    pub show_task_edit: bool,
    pub show_task_summary: bool
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
    pub fn get_task(&self, uid_opt: &Option<String>) -> Option<&Task> {
        if let Some(uid) = uid_opt {
            self.tasks.get(uid)
        } else {None}
    }

    pub fn get_task_sum(&self, uid_opt: &Option<String>) -> Option<&TimePrefixSum> {
        if let Some(uid) = uid_opt {
            self.task_sums.get(uid)
        } else {None}
    }

    fn passes_filter(&self, task: &Task) -> bool {
        let focus_ok = match self.focus_filter {
            FocusFilter::Status(ref x) => x.eq(&task.task_status),
            FocusFilter::All => task.task_status != TaskStatus::Archived,
        };

        let tag_ok =
            if let Some(ref tag_filter) = self.tag_filter {
                task.tags.contains(tag_filter)
            } else {
                true
            };

        return focus_ok && tag_ok;
    }

    pub fn get_tasks_filtered(&self) -> Vector<Task> {
        let mut elems = self.tasks.values()
            .filter(|t| self.passes_filter(&t))
            .collect::<Vector<&Task>>();

        elems.sort_by(|v1: &&Task, v2: &&Task| v1.cmp(v2));

        return elems.iter().map(|v| v.clone()).cloned().collect();
    }

    pub fn get_uids_filtered(&self) -> Vector<String> {
        return self.get_tasks_filtered().into_iter().map(|t| t.uid).collect();
    }

    pub fn check_update_selected(&mut self) {
        if let Some(ref selected) = self.selected_task {
            let mut filtered: Vector<String> = self.get_uids_filtered();

            // select any task if currently selected is filtered out
            if !filtered.contains(selected) {
                self.selected_task = filtered.pop_front();
            }
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

