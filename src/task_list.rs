use std::rc::Rc;

use std::thread::{spawn, sleep};
use std::sync::mpsc::{channel, TryRecvError};

use druid::im::{Vector};
use druid::widget::prelude::*;

use druid::widget::{Flex, Label, List, Controller, ControllerHost, Container, Painter, Scroll, SizedBox};

use druid::kurbo::Circle;

use druid::{PaintCtx, RenderContext, Env, Event, EventCtx, Point,
            Menu, MenuItem, TimerToken, LocalizedString, UnitPoint, Widget, WidgetPod, WidgetExt,};

use notify_rust::Notification;

use chrono::prelude::*;

use crossterm::{
    event::{self, KeyCode},
    ExecutableCommand,
};

use ratatui::{prelude::*, style::palette::tailwind, widgets::*};

use crate::task::*;
use crate::app_model::*;
use crate::common::*;
use crate::db;
use crate::utils;

const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const ALT_ROW_COLOR: Color = tailwind::SLATE.c900;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const COMPLETED_TEXT_COLOR: Color = tailwind::GREEN.c500;

pub struct TaskItem {
    pub uid: TaskID,
    pub name: String
}

impl TaskItem {
    fn to_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => NORMAL_ROW_COLOR,
            _ => ALT_ROW_COLOR,
        };
        let line = format!(" ✓ {}", self.name);

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
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };

        self.state.select(Some(i));

        return Some(self.items[i].uid.clone());
    }

    fn previous(&mut self) -> Option<TaskID> {
        if self.items.is_empty() {return None;}

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
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
            Space => {
                if let Some(uid) = model.selected_task.as_ref() {
                    start_tracking(model, uid.clone())
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


// pub struct TaskListWidget {
//     inner: WidgetPod<(AppModel, Vector<String>),
//                   Scroll<(AppModel, Vector<String>), List<(AppModel, String)>>>
// }

// impl TaskListWidget {
//     pub fn new() -> TaskListWidget {

//         let inner = Scroll::new(List::new(|| {

//             let task_painter =
//                 Painter::new(|ctx: &mut PaintCtx, (shared, uid): &(AppModel, String), _env| {
//                     let bounds = ctx.size().to_rect();

//                     if let Some(ref selected) = shared.selected_task {
//                         if selected.contains(uid) {
//                             ctx.fill(bounds, &TASK_COLOR_BG);
//                         }
//                     }

//                     match shared.tracking.state {
//                         TrackingState::Active(ref active) if uid.eq(active) => {
//                             ctx.stroke(bounds, &TASK_ACTIVE_COLOR_BG, 4.0);
//                             return;
//                         },
//                         TrackingState::Paused(ref paused) if uid.eq(paused) => {
//                             ctx.stroke(bounds, &TASK_PAUSE_COLOR_BG, 4.0);
//                             return;
//                         },
//                         TrackingState::Break(ref rest) if uid.eq(rest) => {
//                             ctx.stroke(bounds, &TASK_REST_COLOR_BG, 4.0);
//                             return;
//                         },
//                         _ => (),
//                     };

//                     ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
//                 });

//             let container = Container::new(
//                 Flex::row()
//                     .with_child(
//                     Label::new(|(d, uid): &(AppModel, String), _env: &_| {
//                         let task = d.tasks.get(uid).expect("unknown uid");
//                         format!("{}", match task.priority.into() {
//                             CuaPriority::Low => "↓",
//                             CuaPriority::Normal | CuaPriority::Unspecified => " ",
//                             CuaPriority::High => "❗",
//                         })
//                     }).fix_width(15.0))

//                     .with_flex_child(
//                     Label::new(|(d, uid): &(AppModel, String), _env: &_| {
//                         let task = d.tasks.get(uid).expect("unknown uid");
//                         format!("{}", task.name)
//                     })  .expand_width()
//                         .align_vertical(UnitPoint::LEFT)
//                         .padding(10.0)
//                         , 1.0)
//                     .with_child(
//                         SizedBox::new(
//                             Painter::new(|ctx: &mut PaintCtx,
//                                          (model, uid): &(AppModel, String), _env| {
//                                 let task = model.tasks.get(uid).expect("unknown uid");
//                                 ctx.fill(Circle::new(Point::new(5.0, 5.0), 5.0), &task.color);
//                             }))
//                             .width(10.0).height(10.0))
//                     .padding((0.0, 0.0, 10.0, 0.0)))
//                 .on_click(|_ctx, (shared, uid): &mut (AppModel, String), _env| {
//                         shared.selected_task = Some(uid.clone());
//                     })
//                 .background(task_painter);

//             // container
//             ControllerHost::new(container, ContextMenuController)
//         })
//         .with_spacing(10.)).vertical();

//         return TaskListWidget{inner: WidgetPod::new(inner)};
//     }
// }

// timer with os thread:
// use std::thread::{spawn, sleep};
// use std::time::{Duration, Instant};
// use std::sync::mpsc::channel;


// fn main() {
//     let (tx, rx) = channel();
//     let now = Instant::now();
//     println!("Start! - {:?}", now.elapsed());
//     let _ = spawn(move || {
//         sleep(Duration::from_secs(2));
//         tx.send(()).unwrap();
//     });
    
//     // Normally recv blocks, you could also use try_recv which doesn't
//     // Instead of blocking here, go off and do something else
//     let _ = rx.recv().unwrap();
    
//     println!("Done! - {:?}", now.elapsed());
    
// }





// impl Widget<(AppModel, Vector<String>)> for TaskListWidget {
//     fn event(&mut self, ctx: &mut EventCtx, event: &Event,
//              data: &mut (AppModel, Vector<String>), _env: &Env) {

//         match event {
//             //TODO rewrite when "if let" guards are stablilized
//             // https://github.com/rust-lang/rust/issues/51114

//             Event::Command(cmd) if cmd.is(COMMAND_TASK_START) => {
//                 stop_tracking(&mut data.0, TrackingState::Inactive);
//                 start_tracking(&mut data.0, cmd.get(COMMAND_TASK_START).unwrap().clone(), ctx);
//             },

//             Event::Command(cmd) if cmd.is(COMMAND_TASK_STOP) => {
//                 stop_tracking(&mut data.0, TrackingState::Inactive);
//             }

//             Event::Command(cmd) if cmd.is(COMMAND_TASK_PAUSE) => {
//                 let uid = match &data.0.tracking.state {
//                     TrackingState::Active(uid) => uid.clone(),
//                     _ => panic!("state is not active"),
//                 };
//                 pause_tracking(&mut data.0, uid);
//             }

//            Event::Command(cmd) if cmd.is(COMMAND_TASK_RESUME) => {
//                resume_tracking(&mut data.0, cmd.get(COMMAND_TASK_RESUME).unwrap().clone(), ctx);
//             }

//             Event::Command(cmd) if cmd.is(COMMAND_TASK_NEW) => {
//                 let task = Task::new_simple("new task".to_string());
//                 let uid = task.uid.clone();

//                 if let Err(what) = db::add_task(data.0.db.clone(), &task) {
//                     println!("db error: {}", what);
//                 }

//                 data.0.selected_task = Some(task.uid.clone());
//                 data.0.tasks.insert(uid.clone(), task);
//                 data.0.task_sums.insert(uid.clone(), TimePrefixSum::new());
//                 data.0.update_tags();
//                 data.0.show_task_edit = true;

//                 ctx.set_focus(TASK_EDIT_WIDGET);
//                 ctx.request_update();
//             },
//             Event::Command(cmd) if cmd.is(COMMAND_TASK_COMPLETED) => {
//                 let uid = cmd.get(COMMAND_TASK_COMPLETED).unwrap().clone();
//                 let mut task = data.0.tasks.get(&uid).expect("unknown uid").clone();
//                 task.task_status = TaskStatus::Completed;

//                 if let Err(what) = db::update_task(data.0.db.clone(), &task) {
//                     println!("db error: {}", what);
//                 }

//                 match &data.0.tracking.state {
//                     TrackingState::Active(cur) if cur.eq(&uid)
//                         => stop_tracking(&mut data.0, TrackingState::Inactive),
//                     TrackingState::Paused(cur) if cur.eq(&uid)
//                         => data.0.tracking.state = TrackingState::Inactive,
//                     TrackingState::Break(cur) if cur.eq(&uid)
//                         => data.0.tracking.state = TrackingState::Inactive,
//                     _ => (),
//                 };

//                 data.0.tasks = data.0.tasks.update(uid, task);
//                 data.0.check_update_selected();
//             },
//             Event::Command(cmd) if cmd.is(COMMAND_TASK_ARCHIVE) => {
//                 archive_task(&mut data.0, cmd.get(COMMAND_TASK_ARCHIVE).unwrap());
//                 ctx.request_update();
//             },
//             Event::Command(cmd) if cmd.is(COMMAND_TLIST_REQUEST_FOCUS) => {
//                 ctx.request_focus();
//             }
//             Event::Timer(id) => {
//                 if *id == *data.0.tracking.timer_id {
//                     utils::play_sound(SOUND_TASK_FINISH, WORK_TIMER_VOLUME);

//                     match data.0.tracking.state.clone() {
//                         TrackingState::Active(uid) => {
//                             stop_tracking(&mut data.0, TrackingState::Inactive);

//                             #[cfg(not(target_os = "windows"))]
//                             Notification::new()
//                                 .summary(&format!("netupi: \"{}\" session finished",
//                                                data.0.tasks.get(&uid).unwrap().name))
//                                 .show();

//                             start_rest(&mut data.0, uid, ctx);
//                         },
//                         TrackingState::Break(uid) => {

//                             #[cfg(not(target_os = "windows"))]
//                             Notification::new()
//                                 .summary(&format!("netupi: \"{}\" break finished",
//                                                data.0.tasks.get(&uid).unwrap().name))
//                                 .show();

//                             data.0.tracking.state = TrackingState::Inactive
//                         },
//                         _ => {},
//                     };
//                 }
//             },

//             Event::MouseMove(_) => ctx.request_focus(),

//             //TODO think of better implementation
//             Event::KeyUp(key) if key.code == druid::Code::ArrowDown => {
//                 if let Some(ref uid) = data.0.selected_task {
//                     let mut next = None;
//                     let selected = data.0.tasks.get(uid).unwrap();

//                     for x in data.0.get_tasks_filtered() {
//                         if x > *selected {
//                             next = Some(x);
//                             break;
//                         }
//                     }

//                     next.map(|x| data.0.selected_task = Some(x.uid));
//                 }
//             },

//             Event::KeyUp(key) if key.code == druid::Code::ArrowUp => {

//                 if let Some(ref uid) = data.0.selected_task {
//                     let mut next = None;
//                     let selected = data.0.tasks.get(uid).unwrap();

//                     for x in data.0.get_tasks_filtered() {
//                         if x == *selected {
//                             break;
//                         }
//                         next = Some(x);
//                     }

//                     next.map(|x| data.0.selected_task = Some(x.uid));
//                 }
//             },

//             Event::KeyUp(key) if key.code == druid::Code::Space => {
//                 if data.0.selected_task.is_none() {return;}

//                 let selected = data.0.selected_task.as_ref().unwrap().clone();

//                 match data.0.tracking.state.clone() {
//                     TrackingState::Inactive => start_tracking(&mut data.0, selected, ctx),

//                     TrackingState::Active(uid) if uid.eq(&selected) =>
//                         pause_tracking(&mut data.0, uid),

//                     TrackingState::Active(_) => {
//                         stop_tracking(&mut data.0, TrackingState::Inactive);
//                         start_tracking(&mut data.0, selected, ctx);
//                     },
//                     TrackingState::Paused(uid) if uid.eq(&selected) =>
//                         resume_tracking(&mut data.0, uid, ctx),

//                     TrackingState::Paused(_uid) => {
//                         stop_tracking(&mut data.0, TrackingState::Inactive);
//                         start_tracking(&mut data.0, selected, ctx);
//                     },
//                     TrackingState::Break(uid) => start_tracking(&mut data.0, uid, ctx),
//                 }
//             }

//             Event::KeyUp(key) if key.code == druid::Code::Escape => {
//                 match data.0.tracking.state.clone() {
//                     TrackingState::Active(_) => stop_tracking(&mut data.0, TrackingState::Inactive),
//                     _ => data.0.tracking.state = TrackingState::Inactive,
//                 }
//             },

//             Event::KeyUp(key) if key.code == druid::Code::KeyN => {
//                 ctx.submit_command(COMMAND_TASK_NEW.with(()));
//             },

//             Event::KeyUp(key) if key.code == druid::Code::KeyE => {
//                 if !data.0.show_task_edit {
//                     data.0.show_task_edit = true;
//                     // ctx.set_focus(TASK_NAME_EDIT_WIDGET);
//                 } else {
//                     data.0.show_task_edit = false;
//                 }
//             },

//             Event::KeyUp(key) if key.code == druid::Code::Tab => {
//                 ctx.focus_next();
//             },

//             Event::KeyUp(key) if key.code == druid::Code::KeyC => {
//                 if let Some(ref selected) = data.0.selected_task {
//                     ctx.submit_command(COMMAND_TASK_COMPLETED.with(selected.clone()));
//                 }
//             },

//             // Event::KeyUp(key) => {
//             //     println!("TaskList: unknown key: {:?}", key);
//             // },

//             _ => self.inner.event(ctx, event, data, _env),
//         }
//     }

//     fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(AppModel, Vector<String>), _env: &Env) {
//         match event {
//             LifeCycle::BuildFocusChain => {
//                 ctx.register_for_focus();
//                 ctx.submit_command(COMMAND_TLIST_REQUEST_FOCUS.with(()));
//                 self.inner.lifecycle(ctx, event, _data, _env)
//             },

//             // LifeCycle::FocusChanged(val) => println!("TaskList: focus = {}", val),

//             _ => self.inner.lifecycle(ctx, event, _data, _env)
//         };
//     }

//     fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(AppModel, Vector<String>), _data: &(AppModel, Vector<String>), _env: &Env) {
//         self.inner.update(_ctx, _data, _env)
//     }

//     fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &(AppModel, Vector<String>), env: &Env,
//     ) -> Size {
//         let ret = self.inner.layout(ctx, &bc.shrink(Size::new(20.0, 20.0)), data, env);
//         self.inner.set_origin(ctx, &data, env, Point::new(10.0, 10.0));

//         if ret.is_empty() {
//             ret
//         } else {
//             ret + Size::new(20.0, 20.0)
//         }
//     }

//     fn paint(&mut self, ctx: &mut PaintCtx, data: &(AppModel, Vector<String>), env: &Env) {
//         self.inner.paint(ctx, data, env);
//         let bounds = ctx.size().to_rect();
//         if ctx.has_focus() {
//             ctx.stroke(bounds, &TASK_FOCUS_BORDER, 2.0);
//         }
//     }
// }

// struct ContextMenuController;

// impl<W: Widget<(AppModel, String)>> Controller<(AppModel, String), W> for ContextMenuController {
//     fn event(
//         &mut self,
//         child: &mut W,
//         ctx: &mut EventCtx,
//         event: &Event,
//         data: &mut (AppModel, String),
//         env: &Env,
//     ) {
//         match event {
//             Event::MouseDown(ref mouse) if mouse.button.is_right() => {
//                 ctx.show_context_menu(make_task_menu(&data.0, &Some(data.1.clone())), mouse.pos);
//             }
//             _ => child.event(ctx, event, data, env),
//         }
//     }
// }

// pub fn make_task_menu(d: &AppModel, current_opt: &Option<String>) -> Menu<AppModel> {
//     let mut result = Menu::new(LocalizedString::new("Task"));

//     let new_task_entry = MenuItem::new(LocalizedString::new("New task"))
//                 .on_activate(
//                     move |ctx, _: &mut AppModel, _env| {
//                     ctx.submit_command(COMMAND_TASK_NEW.with(()));
//                     });

//     if current_opt.is_none() {
//         return result.entry(new_task_entry);
//     }

//     let current = current_opt.as_ref().unwrap();

//     let start_entry = {
//         let uid_for_closure = current.clone();
//         MenuItem::new(LocalizedString::new("Start tracking")).on_activate(
//             move |ctx, _d: &mut AppModel, _env| {
//                 ctx.submit_command(COMMAND_TASK_START.with(uid_for_closure.clone()));
//             })
//     };

//     let pause_entry =
//         MenuItem::new(LocalizedString::new("Pause")).on_activate(
//             move |ctx, _d: &mut AppModel, _env| {
//                 ctx.submit_command(COMMAND_TASK_PAUSE.with(()));
//             });

//     let stop_entry = MenuItem::new(LocalizedString::new("Stop tracking")).on_activate(
//         move |ctx, _d: &mut AppModel, _env| {
//             ctx.submit_command(COMMAND_TASK_STOP.with(()));
//         });

//     let resume_entry = {
//         let uid_for_closure = current.clone();
//         MenuItem::new(LocalizedString::new("Resume")).on_activate(
//             move |ctx, _d: &mut AppModel, _env| {
//                 ctx.submit_command(COMMAND_TASK_RESUME.with(uid_for_closure.clone()));
//             })
//     };

//     match &d.tracking.state {
//         TrackingState::Active(uid) if current.eq(uid) =>
//             result = result.entry(pause_entry).entry(stop_entry),

//         TrackingState::Paused(uid) if current.eq(uid) =>
//             result = result.entry(resume_entry).entry(stop_entry),

//         TrackingState::Break(uid) if current.eq(uid) =>
//             result = result.entry(start_entry),

//         _ =>
//             result = result.entry(start_entry),
//     };

//     let uid_archive = current.clone();
//     let uid_completed = current.clone();

//     let completed_entry = MenuItem::new(LocalizedString::new("Mark completed"))
//         .on_activate(
//             move |ctx, _: &mut AppModel, _env| {
//                 ctx.submit_command(COMMAND_TASK_COMPLETED.with(uid_completed.clone()));
//             });

//     result = match &d.tasks.get(current).unwrap().task_status {
//         TaskStatus::NeedsAction | TaskStatus::InProcess => result.entry(completed_entry),
//         _ => result,
//     };

//     result
//         .entry(
//             new_task_entry,
//         )
//         .entry(
//             MenuItem::new(LocalizedString::new("Archive")).on_activate(
//                 move |ctx, _: &mut AppModel, _env| {
//                     ctx.submit_command(COMMAND_TASK_ARCHIVE.with(uid_archive.clone()));
//                 },
//             ),
//         )
// }

fn request_timer(duration: std::time::Duration) -> Option<TimerTok>
{
    let (tx, rx) = channel();

    let _ = spawn(move || {
        sleep(duration);
        tx.send(0).unwrap();
    });

    return Some(TimerTok{channel: rx});
}

fn start_rest(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer = request_timer(get_rest_interval(data, &uid).to_std().unwrap());
    data.tracking.state = TrackingState::Break(uid);
}

fn resume_tracking(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer = request_timer(get_work_interval(data, &uid).checked_sub(&data.tracking.elapsed)
                                  .unwrap_or(chrono::Duration::zero()).to_std().unwrap());
    data.tracking.state = TrackingState::Active(uid);
}

fn start_tracking(data: &mut AppModel, uid: String) {
    use TaskStatus::*;

    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.elapsed = Rc::new(chrono::Duration::zero());
    data.tracking.timer = request_timer(get_work_interval(data, &uid).to_std().unwrap());

    // DEBUG: fuse timer for 5 seconds
    // data.tracking.timer = request_timer(std::time::Duration::from_secs(5));

    let mut task = data.tasks.get_mut(&uid).expect(&format!("unknown task {}", &uid));
    let needs_update = task.task_status != InProcess;

    task.task_status = InProcess;

    if needs_update {
        if let Err(what) = db::update_task(data.db.clone(), &task) {
            println!("db error: {}", what);
        }
    }

    data.focus_filter =
    match data.focus_filter {
        FocusFilter::Status(Completed) |
        FocusFilter::Status(NeedsAction) => FocusFilter::Status(InProcess),
        ref st => st.clone(),
    };

    data.tracking.state = TrackingState::Active(uid);
}

fn pause_tracking(data: &mut AppModel, uid: String)
{
    stop_tracking(data, TrackingState::Paused(uid));

    //TODO communication here
    data.tracking.timer = None;
}

fn stop_tracking(data: &mut AppModel, new_state: TrackingState) {

    //TODO communication here
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

    if let Err(what) = db::add_time_record(data.db.clone(), &record) {
        println!("db error: {}", what);
    }

    let duration = now.signed_duration_since(data.tracking.timestamp.as_ref().clone());

    data.tracking.elapsed = Rc::new(duration);

    println!("Task '{}' duration: {}:{}:{}", &task.name,
             duration.num_hours(), duration.num_minutes(), duration.num_seconds());

    data.records.insert(*record.from, record.clone());
    add_record_to_sum(data.task_sums.get_mut(&task.uid).expect("unknown uid"), &record);

    data.tracking.state = new_state;
}

fn archive_task(model: &mut AppModel, uid: &String) {
    let task = model.tasks.get_mut(uid).expect(&format!("unknown task: {}", uid));
    task.task_status = TaskStatus::Archived;
    if let Err(what) = db::update_task(model.db.clone(), &task) {
        println!("db error: {}", what);
    }
    model.update_tags();
    model.check_update_selected();
}
