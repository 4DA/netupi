use std::rc::Rc;

use std::thread::{spawn, sleep};
use std::sync::mpsc::channel;

use notify_rust::Notification;

use chrono::prelude::*;

use crossterm::event::{self, KeyCode};

use ratatui::{prelude::*, widgets::*};

use crate::task::*;
use crate::theme;
use crate::app_model::*;
use crate::db;
use crate::utils;

pub struct TaskItem {
    pub uid: TaskID,
    pub name: String
}

impl TaskItem {
    pub fn to_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => theme::ROW_BG,
            _ => theme::ROW_ALT_BG,
        };
        let line = format!(" {}", self.name);

        ListItem::new(line).bg(bg_color)
    }
}

pub struct TaskList {
    pub state: ListState,
    pub items: Vec<TaskItem>,
    pub last_selected: Option<usize>,
}

impl TaskList {
    fn next(&mut self) -> Option<TaskID> {
        if self.items.is_empty() {return None;}

        let i = match self.state.selected() {
            Some(i) => (i + 1).min(self.items.len() - 1),
            None => self.last_selected.unwrap_or(0),
        };

        self.state.select(Some(i));

        return Some(self.items[i].uid.clone());
    }

    fn previous(&mut self) -> Option<TaskID> {
        if self.items.is_empty() {return None;}

        let i = match self.state.selected() {
            Some(i) => i.saturating_sub(1),
            None => self.last_selected.unwrap_or(0),
        };

        self.state.select(Some(i));

        return Some(self.items[i].uid.clone());
    }

    pub fn keymap_task_list(&mut self, model: &mut AppModel, key: event::KeyCode) {
        use KeyCode::*;
        match key {
            Char('j') | Down => model.selected_task = self.next(),
            Char('k') | Up => model.selected_task = self.previous(),

            // space: start/pause/resume tracking
            Char(' ') => {
                if let Some(selected) = model.selected_task.clone() {
                    match model.tracking.state.clone() {
                        TrackingState::Inactive => start_tracking(model, selected),

                        TrackingState::Active(ref uid) if uid.eq(&selected) =>
                            pause_tracking(model, uid.clone()),

                        TrackingState::Active(_) => {
                            stop_tracking(model, TrackingState::Inactive);
                            start_tracking(model, selected);
                        },

                        TrackingState::Paused(ref uid) if uid.eq(&selected) =>
                            resume_tracking(model, uid.clone()),

                        TrackingState::Paused(_) => {
                            stop_tracking(model, TrackingState::Inactive);
                            start_tracking(model, selected);
                        },

                        TrackingState::Break(_uid) => start_tracking(model, selected),
                    }
                    self.update(model);
                }
            },

            // escape: stop tracking
            Esc => {
                match model.tracking.state.clone() {
                    TrackingState::Active(_) => stop_tracking(model, TrackingState::Inactive),
                    _ => model.tracking.state = TrackingState::Inactive,
                }
                self.update(model);
            },


            // c: mark completed
            Char('c') => {
                if let Some(selected) = model.selected_task.clone() {
                    let old_idx = self.state.selected().unwrap_or(0);
                    let mut task = model.tasks.get(&selected).expect("unknown uid").clone();
                    task.task_status = TaskStatus::Completed;

                    if let Err(what) = db::update_task(model.db.clone(), &task) {
                        eprintln!("db error: {}", what);
                    }

                    match &model.tracking.state {
                        TrackingState::Active(cur) if cur.eq(&selected)
                            => stop_tracking(model, TrackingState::Inactive),
                        TrackingState::Paused(cur) if cur.eq(&selected)
                            => model.tracking.state = TrackingState::Inactive,
                        TrackingState::Break(cur) if cur.eq(&selected)
                            => model.tracking.state = TrackingState::Inactive,
                        _ => (),
                    };

                    model.tasks = model.tasks.update(selected, task);
                    let filtered = model.get_uids_filtered();
                    model.selected_task = if filtered.is_empty() {
                        None
                    } else {
                        let idx = old_idx.min(filtered.len() - 1);
                        Some(filtered[idx].clone())
                    };
                    self.update(model);
                }
            },

            // d: permanently delete archived task
            Char('d') => {
                if model.focus_filter == FocusFilter::Status(TaskStatus::Archived) {
                    if let Some(ref selected) = model.selected_task.clone() {
                        let old_idx = self.state.selected().unwrap_or(0);

                        if let Err(what) = db::delete_task(model.db.clone(), selected) {
                            eprintln!("db error: {}", what);
                        }
                        if let Err(what) = db::delete_time_records_for_task(model.db.clone(), selected) {
                            eprintln!("db error: {}", what);
                        }

                        let keys_to_remove: Vec<_> = model.records.iter()
                            .filter(|(_, r)| &r.uid == selected)
                            .map(|(k, _)| k.clone())
                            .collect();
                        for k in keys_to_remove {
                            model.records.remove(&k);
                        }
                        model.task_sums.remove(selected);
                        model.tasks.remove(selected);
                        model.update_tags();

                        let filtered = model.get_uids_filtered();
                        model.selected_task = if filtered.is_empty() {
                            None
                        } else {
                            let idx = old_idx.min(filtered.len() - 1);
                            Some(filtered[idx].clone())
                        };
                        self.update(model);
                    }
                }
            },

            // a: archive task
            Char('a') => {
                if let Some(ref selected) = model.selected_task.clone() {
                    let old_idx = self.state.selected().unwrap_or(0);
                    archive_task(model, selected);
                    let filtered = model.get_uids_filtered();
                    model.selected_task = if filtered.is_empty() {
                        None
                    } else {
                        let idx = old_idx.min(filtered.len() - 1);
                        Some(filtered[idx].clone())
                    };
                    self.update(model);
                }
            },

            _ => {}
        }
    }

    pub fn update(&mut self, model: &AppModel) {
        let tasks = model.get_tasks_filtered();

        let mut selected_id = None;

        self.items = tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if let Some(sel) = &model.selected_task {
                    if sel.eq(&t.uid) {selected_id = Some(i)}
                };
                TaskItem{uid: t.uid.clone(), name: t.name.clone()}
            })
            .collect();

        self.state.select(selected_id);
    }
}

fn request_timer(duration: std::time::Duration) -> Option<TimerTok>
{
    let (tx, rx) = channel();

    let _ = spawn(move || {
        sleep(duration);
        let _ = tx.send(0);
    });

    return Some(TimerTok{channel: rx});
}

pub fn start_rest(data: &mut AppModel, uid: String) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer = request_timer(get_rest_interval(data, &uid).to_std().unwrap());
    data.tracking.state = TrackingState::Break(uid);
}

pub fn resume_tracking(data: &mut AppModel, uid: String) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer = request_timer(get_work_interval(data, &uid).checked_sub(&data.tracking.elapsed)
                                  .unwrap_or(chrono::Duration::zero()).to_std().unwrap());
    data.tracking.state = TrackingState::Active(uid);
}

pub fn start_tracking(data: &mut AppModel, uid: String) {
    use TaskStatus::*;

    let now = Rc::new(Utc::now());
    data.tracking.timestamp = now.clone();
    data.tracking.elapsed = Rc::new(chrono::Duration::zero());
    data.tracking.timer = request_timer(get_work_interval(data, &uid).to_std().unwrap());

    let task = data.tasks.get_mut(&uid).expect(&format!("unknown task {}", &uid));
    let needs_update = task.task_status != InProcess;

    task.task_status = InProcess;

    if needs_update {
        if let Err(what) = db::update_task(data.db.clone(), &task) {
            eprintln!("db error: {}", what);
        }
    }

    // write record with ts_from == ts_to so external tools can detect active tracking;
    // ts_to is updated on stop, stale records are cleaned up on next startup
    let record = TimeRecord { from: now.clone(), to: now, uid: uid.clone() };
    if let Err(what) = db::add_time_record(data.db.clone(), &record) {
        eprintln!("db error: {}", what);
    }

    data.focus_filter =
    match data.focus_filter {
        FocusFilter::Status(Completed) |
        FocusFilter::Status(NeedsAction) => FocusFilter::Status(InProcess),
        ref st => st.clone(),
    };

    data.tracking.state = TrackingState::Active(uid);
}

pub fn pause_tracking(data: &mut AppModel, uid: String)
{
    stop_tracking(data, TrackingState::Paused(uid));
    data.tracking.timer = None;
}

pub fn stop_tracking(data: &mut AppModel, new_state: TrackingState) {
    data.tracking.timer = None;

    let task = match &data.tracking.state {
        TrackingState::Active(uid) => data.tasks.get(uid).unwrap(),
        _ => {
            data.tracking.state = new_state;
            return;
        }
    };

    let now = Rc::new(Utc::now());
    let record = TimeRecord{from: data.tracking.timestamp.clone(), to: now.clone(),
                            uid: task.uid.clone()};

    // update the record that was created on start
    if let Err(what) = db::update_time_record(data.db.clone(), &record) {
        eprintln!("db error: {}", what);
    }

    let duration = now.signed_duration_since(data.tracking.timestamp.as_ref().clone());

    data.tracking.elapsed = Rc::new(duration);

    data.records.insert(*record.from, record.clone());
    add_record_to_sum(data.task_sums.get_mut(&task.uid).expect("unknown uid"), &record);

    data.tracking.state = new_state;
}

fn archive_task(model: &mut AppModel, uid: &String) {
    let task = model.tasks.get_mut(uid).expect(&format!("unknown task: {}", uid));
    task.task_status = TaskStatus::Archived;
    if let Err(what) = db::update_task(model.db.clone(), &task) {
        eprintln!("db error: {}", what);
    }
    model.update_tags();
    model.check_update_selected();
}

pub fn handle_timer_event(model: &mut AppModel) {
    if let Some(timer) = &model.tracking.timer {
        if timer.channel.try_recv().is_ok() {
            utils::play_sound(utils::SOUND_TASK_FINISH, utils::WORK_TIMER_VOLUME);

            match model.tracking.state.clone() {
                TrackingState::Active(uid) => {
                    stop_tracking(model, TrackingState::Inactive);

                    let _ = Notification::new()
                        .summary(&format!("netupi: \"{}\" session finished",
                                       model.tasks.get(&uid).unwrap().name))
                        .show();

                    start_rest(model, uid);
                },
                TrackingState::Break(uid) => {
                    let _ = Notification::new()
                        .summary(&format!("netupi: \"{}\" break finished",
                                       model.tasks.get(&uid).unwrap().name))
                        .show();

                    model.tracking.state = TrackingState::Inactive;
                },
                _ => {},
            };
        }
    }
}
