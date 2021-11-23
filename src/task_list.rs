use std::rc::Rc;
use std::thread;
use std::io::BufReader;
use std::time::SystemTime;

use druid::im::{Vector, OrdSet};
use druid::widget::prelude::*;

use druid::widget::{Label, List, Controller, ControllerHost, Container, Painter, Scroll};

use druid::{PaintCtx, RenderContext, Env, Event, EventCtx, Point,
            Menu, MenuItem, TimerToken, LocalizedString, UnitPoint, Widget, WidgetPod, WidgetExt,};


use chrono::prelude::*;

use rodio::{Decoder, OutputStream, Sink};

// uid stuff
use uuid::v1::{Timestamp, Context};
use uuid::Uuid;

use crate::task::*;
use crate::app_model::*;
use crate::common::*;
use crate::db;

pub struct TaskListWidget {
    inner: WidgetPod<(AppModel, Vector<String>),
                  Scroll<(AppModel, Vector<String>), List<(AppModel, String)>>>
}

impl TaskListWidget {
    pub fn new() -> TaskListWidget {

        let inner = Scroll::new(List::new(|| {

            let task_painter =
                Painter::new(|ctx: &mut PaintCtx, (shared, uid): &(AppModel, String), _env| {
                    let bounds = ctx.size().to_rect();
                    if shared.selected_task.eq(uid) {
                        ctx.fill(bounds, &TASK_COLOR_BG);
                    }

                    match shared.tracking.state {
                        TrackingState::Active(ref active) if uid.eq(active) => {
                            ctx.stroke(bounds, &TASK_ACTIVE_COLOR_BG, 4.0);
                            return;
                        },
                        TrackingState::Paused(ref paused) if uid.eq(paused) => {
                            ctx.stroke(bounds, &TASK_PAUSE_COLOR_BG, 4.0);
                            return;
                        },
                        TrackingState::Break(ref rest) if uid.eq(rest) => {
                            ctx.stroke(bounds, &TASK_REST_COLOR_BG, 4.0);
                            return;
                        },
                        _ => (),
                    };

                    ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                });

            let container = Container::new(
                Label::new(|(d, uid): &(AppModel, String), _env: &_| {
                    let task = d.tasks.get(uid).expect("unknown uid");
                    format!("{}", task.name)
                })
                 .expand_width()
                 .align_vertical(UnitPoint::LEFT)
                 .padding(10.0),
                )
                .on_click(|_ctx, (shared, uid): &mut (AppModel, String), _env| {
                        shared.selected_task = uid.clone();
                    })
                .background(task_painter);

            // container
            ControllerHost::new(container, ContextMenuController)
        })
        .with_spacing(10.)).vertical();

        return TaskListWidget{inner: WidgetPod::new(inner)};
    }
}

impl Widget<(AppModel, Vector<String>)> for TaskListWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event,
             data: &mut (AppModel, Vector<String>), _env: &Env) {

        match event {
            //TODO rewrite when "if let" guards are stablilized
            // https://github.com/rust-lang/rust/issues/51114

            Event::Command(cmd) if cmd.is(COMMAND_TASK_START) => {
                stop_tracking(&mut data.0, TrackingState::Inactive);
                start_tracking(&mut data.0, cmd.get(COMMAND_TASK_START).unwrap().clone(), ctx);
            },

            Event::Command(cmd) if cmd.is(COMMAND_TASK_STOP) => {
                stop_tracking(&mut data.0, TrackingState::Inactive);
            }

            Event::Command(cmd) if cmd.is(COMMAND_TASK_PAUSE) => {
                let uid = match &data.0.tracking.state {
                    TrackingState::Active(uid) => uid.clone(),
                    _ => panic!("state is not active"),
                };
                pause_tracking(&mut data.0, uid);
            }

           Event::Command(cmd) if cmd.is(COMMAND_TASK_RESUME) => {
               resume_tracking(&mut data.0, cmd.get(COMMAND_TASK_RESUME).unwrap().clone(), ctx);
            }

            Event::Command(cmd) if cmd.is(COMMAND_TASK_NEW) => {
                let uid = generate_uid();
                let task = Task::new("new task".to_string(), "".to_string(), uid.clone(), OrdSet::new(),
                                     0, TaskStatus::NeedsAction, 0);

                if let Err(what) = db::add_task(data.0.db.clone(), &task) {
                    println!("db error: {}", what);
                }

                data.0.selected_task = task.uid.clone();
                data.0.tasks.insert(uid.clone(), task);
                data.0.task_sums.insert(uid.clone(), TimePrefixSum::new());
                data.0.update_tags();
                ctx.submit_command(COMMAND_DETAILS_REQUEST_FOCUS.with(()));
                ctx.request_update();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_COMPLETED) => {
                let uid = cmd.get(COMMAND_TASK_COMPLETED).unwrap().clone();
                let mut task = data.0.tasks.get(&uid).expect("unknown uid").clone();
                task.task_status = TaskStatus::Completed;

                match &data.0.tracking.state {
                    TrackingState::Active(cur) if cur.eq(&uid)
                        => stop_tracking(&mut data.0, TrackingState::Inactive),
                    TrackingState::Paused(cur) if cur.eq(&uid)
                        => data.0.tracking.state = TrackingState::Inactive,
                    TrackingState::Break(cur) if cur.eq(&uid)
                        => data.0.tracking.state = TrackingState::Inactive,
                    _ => (),
                };

                data.0.tasks = data.0.tasks.update(uid, task);
                data.0.check_update_selected();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TASK_ARCHIVE) => {
                archive_task(&mut data.0, cmd.get(COMMAND_TASK_ARCHIVE).unwrap());
                ctx.request_update();
            },
            Event::Command(cmd) if cmd.is(COMMAND_TLIST_REQUEST_FOCUS) => {
                ctx.request_focus();
            }
            Event::Timer(id) => {
                if *id == *data.0.tracking.timer_id {
                    play_sound(SOUND_TASK_FINISH);

                    match data.0.tracking.state.clone() {
                        TrackingState::Active(uid) => {
                            stop_tracking(&mut data.0, TrackingState::Inactive);
                            start_rest(&mut data.0, uid, ctx);
                        },
                        TrackingState::Break(_) => {data.0.tracking.state = TrackingState::Inactive},
                        _ => {},
                    };
                }
            },

            Event::MouseMove(_) => ctx.request_focus(),

            //TODO think of better implementation
            Event::KeyUp(key) if key.code == druid::Code::ArrowDown => {
                let mut next = None;
                for x in data.0.get_uids_filtered() {
                    if x > data.0.selected_task {
                        next = Some(x);
                        break;
                    }
                }

                if let Some(next) = next {
                    data.0.selected_task = next;
                }
            },

            Event::KeyUp(key) if key.code == druid::Code::ArrowUp => {
                let mut next = None;
                for x in data.0.get_uids_filtered() {
                    if x == data.0.selected_task {
                        break;
                    }
                    next = Some(x);
                }

                if let Some(next) = next {
                    data.0.selected_task = next;
                }
            },

            Event::KeyUp(key) if key.code == druid::Code::Space => {
                let selected = data.0.selected_task.clone();
                match data.0.tracking.state.clone() {
                    TrackingState::Inactive => start_tracking(&mut data.0, selected, ctx),

                    TrackingState::Active(uid) if uid.eq(&selected) =>
                        pause_tracking(&mut data.0, uid),

                    TrackingState::Active(_) => {
                        stop_tracking(&mut data.0, TrackingState::Inactive);
                        start_tracking(&mut data.0, selected, ctx);
                    },
                    TrackingState::Paused(uid) if uid.eq(&selected) =>
                        resume_tracking(&mut data.0, uid, ctx),

                    TrackingState::Paused(_uid) => {
                        stop_tracking(&mut data.0, TrackingState::Inactive);
                        start_tracking(&mut data.0, selected, ctx);
                    },
                    TrackingState::Break(uid) => start_tracking(&mut data.0, uid, ctx),
                }
            }

            Event::KeyUp(key) if key.code == druid::Code::Escape => {
                match data.0.tracking.state.clone() {
                    TrackingState::Active(_) => stop_tracking(&mut data.0, TrackingState::Inactive),
                    _ => data.0.tracking.state = TrackingState::Inactive,
                }
            },

            Event::KeyUp(key) if key.code == druid::Code::KeyN => {
                ctx.submit_command(COMMAND_TASK_NEW.with(()));
            },

            _ => self.inner.event(ctx, event, data, _env),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &(AppModel, Vector<String>), _env: &Env) {
        match event {
            LifeCycle::WidgetAdded => {
                ctx.register_for_focus();
                ctx.submit_command(COMMAND_TLIST_REQUEST_FOCUS.with(()));
                self.inner.lifecycle(ctx, event, _data, _env)
            },

            _ => self.inner.lifecycle(ctx, event, _data, _env)
        };
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &(AppModel, Vector<String>), _data: &(AppModel, Vector<String>), _env: &Env) {
        self.inner.update(_ctx, _data, _env)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &(AppModel, Vector<String>), env: &Env,
    ) -> Size {
        let ret = self.inner.layout(ctx, &bc.shrink(Size::new(20.0, 20.0)), data, env);
        self.inner.set_origin(ctx, &data, env, Point::new(10.0, 10.0));

        if ret.is_empty() {
            ret
        } else {
            ret + Size::new(20.0, 20.0)
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &(AppModel, Vector<String>), env: &Env) {
        self.inner.paint(ctx, data, env);
        let bounds = ctx.size().to_rect();
        if ctx.is_focused() {
            ctx.stroke(bounds, &TASK_FOCUS_BORDER, 2.0);
        }
    }
}

struct ContextMenuController;

impl<W: Widget<(AppModel, String)>> Controller<(AppModel, String), W> for ContextMenuController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut (AppModel, String),
        env: &Env,
    ) {
        match event {
            Event::MouseDown(ref mouse) if mouse.button.is_right() => {
                ctx.show_context_menu(make_task_menu(&data.0, &data.1), mouse.pos);
            }
            _ => child.event(ctx, event, data, env),
        }
    }
}

pub fn make_task_menu(d: &AppModel, current: &String) -> Menu<AppModel> {
    let mut result = Menu::new(LocalizedString::new("Task"));

    let new_task_entry = MenuItem::new(LocalizedString::new("New task"))
                .on_activate(
                    move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_NEW.with(()));
                    });

    if current.is_empty() {
        return result.entry(new_task_entry);
    }

    let start_entry = {
        let uid_for_closure = current.clone();
        MenuItem::new(LocalizedString::new("Start tracking")).on_activate(
            move |ctx, _d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_START.with(uid_for_closure.clone()));
            })
    };

    let pause_entry =
        MenuItem::new(LocalizedString::new("Pause")).on_activate(
            move |ctx, _d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_PAUSE.with(()));
            });

    let stop_entry = MenuItem::new(LocalizedString::new("Stop tracking")).on_activate(
        move |ctx, _d: &mut AppModel, _env| {
            ctx.submit_command(COMMAND_TASK_STOP.with(()));
        });

    let resume_entry = {
        let uid_for_closure = current.clone();
        MenuItem::new(LocalizedString::new("Resume")).on_activate(
            move |ctx, _d: &mut AppModel, _env| {
                ctx.submit_command(COMMAND_TASK_RESUME.with(uid_for_closure.clone()));
            })
    };

    match &d.tracking.state {
        TrackingState::Active(uid) if current.eq(uid) =>
            result = result.entry(pause_entry).entry(stop_entry),

        TrackingState::Paused(uid) if current.eq(uid) =>
            result = result.entry(resume_entry).entry(stop_entry),

        TrackingState::Break(uid) if current.eq(uid) =>
            result = result.entry(start_entry),

        _ =>
            result = result.entry(start_entry),
    };

    let uid_archive = current.clone();
    let uid_completed = current.clone();

    result
        .entry(
            MenuItem::new(LocalizedString::new("Mark completed"))
                .on_activate(
                    move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_COMPLETED.with(uid_completed.clone()));
                }),
        )
        .entry(
            new_task_entry,
        )
        .entry(
            MenuItem::new(LocalizedString::new("Archive")).on_activate(
                move |ctx, _: &mut AppModel, _env| {
                    ctx.submit_command(COMMAND_TASK_ARCHIVE.with(uid_archive.clone()));
                },
            ),
        )
}

fn start_rest(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer_id =
        Rc::new(ctx.request_timer(get_rest_interval(&uid).to_std().unwrap()));
    data.tracking.state = TrackingState::Break(uid);
}

fn resume_tracking(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.timer_id =
        Rc::new(ctx.request_timer(get_work_interval(&uid).checked_sub(&data.tracking.elapsed)
                                  .unwrap_or(chrono::Duration::zero()).to_std().unwrap()));
    data.tracking.state = TrackingState::Active(uid);
}

fn start_tracking(data: &mut AppModel, uid: String, ctx: &mut EventCtx) {
    data.tracking.timestamp = Rc::new(Utc::now());
    data.tracking.elapsed = Rc::new(chrono::Duration::zero());
    data.tracking.timer_id = Rc::new(ctx.request_timer(get_work_interval(&uid).to_std().unwrap()));
    data.tasks.get_mut(&uid).unwrap().task_status = TaskStatus::InProcess;

    if data.focus_filter == FocusFilter::Completed {
        data.focus_filter = FocusFilter::Current;
    }

    data.tracking.state = TrackingState::Active(uid);
}

fn pause_tracking(data: &mut AppModel, uid: String)
{
    stop_tracking(data, TrackingState::Paused(uid));
    data.tracking.timer_id = Rc::new(TimerToken::INVALID);
}

fn stop_tracking(data: &mut AppModel, new_state: TrackingState) {
    data.tracking.timer_id = Rc::new(TimerToken::INVALID);

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

fn generate_uid() -> String {
    let context = Context::new(42);
    let epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let ts = Timestamp::from_unix(&context, epoch.as_secs(), epoch.subsec_nanos());
    let uuid = Uuid::new_v1(ts, &[1, 2, 3, 4, 5, 6]).expect("failed to generate UUID");
    return uuid.to_string();
}

pub fn play_sound(bytes: &'static [u8]) {
    thread::spawn(move || {
        let bytes = std::io::Cursor::new(bytes.clone());
        // Get a output stream handle to the default physical sound device
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        // Load a sound from a file, using a path relative to Cargo.toml
        let buf = BufReader::new(bytes);
        // Decode that sound file into a source
        let source = Decoder::new(buf).unwrap();

        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.append(source);

        // The sound plays in a separate thread. This call will block the current thread until the sink
        // has finished playing all its queued sounds.
        // sink.sleep_until_end();

        // The sound plays in a separate audio thread,
        // so we need to keep the main thread alive while it's playing.
        std::thread::sleep(std::time::Duration::from_secs(sink.len() as u64));
    });
}
