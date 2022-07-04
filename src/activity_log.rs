use std::rc::Rc;
use std::time::SystemTime;

use druid::widget::prelude::*;
use druid::im::{Vector};
use druid::lens::{self, LensExt};

use druid::widget::{CrossAxisAlignment, Controller, Flex, Label, List, Container, Painter};

use druid::{
    Data, PaintCtx, RenderContext, Env, Event, EventCtx, kurbo,
    LifeCycle, Point, Widget, WidgetPod, WidgetExt};

use druid::{Selector, Cursor};

use chrono::prelude::*;

use crate::task::*;
use crate::app_model::*;
use crate::time;
use crate::db;
use crate::common::*;


#[derive(Clone, Data)]
enum LogEdit {
    None,
    Killed(Rc<DateTime<Utc>>),
    Restored(Rc<DateTime<Utc>>),
}

type TimeRecordCtx = ((AppModel, LogEdit), TimeRecord);

struct LogEntryController;

impl LogEntryController {
    const CMD_HOT: Selector<Rc<DateTime<Utc>>> = Selector::new("alog_entry_hot");
    const CMD_COLD: Selector = Selector::new("alog_entry_cold");
}

fn format_time_record(task: &Task, record: &TimeRecord) -> String {
    let name =  format!("{:wid$}", task.name, wid = 11);

    let duration = time::format_duration(
        &record.to.signed_duration_since(*record.from));

    let now: DateTime<Local> = DateTime::from(SystemTime::now());
    let when: DateTime<Local> = DateTime::<Local>::from(*record.from);

    let time = if now.year() > when.year() {
        when.format("%d %b, %y, %H:%M").to_string()
    } else if now.ordinal() > when.ordinal() {
        when.format("%d %b, %H:%M").to_string()
    } else {
        when.format("%H:%M").to_string()
    };

    format!("{} {:<10} {:<10}", name, duration, time)
}

impl<W: Widget<TimeRecordCtx>> Controller<TimeRecordCtx, W> for LogEntryController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event,
        data: &mut TimeRecordCtx, env: &Env,)
    {
        match event {
            // Event::Command(cmd) if cmd.is(LogEntryController::CMD_HOT) => {
            //     let value = cmd.get(LogEntryController::CMD_HOT).unwrap();

            //     if (*value).eq(&data.1.from) {
            //         ctx.set_handled();
            //     }
            // },
            _ => child.event(ctx, event, data, env),
        }

        ctx.set_cursor(&Cursor::Pointer);
    }

    fn lifecycle(&mut self, child: &mut W, ctx: &mut LifeCycleCtx<'_, '_>,
                     event: &LifeCycle, data: &TimeRecordCtx, env: &Env)
    {
        match event {
            // LifeCycle::HotChanged(value) => if *value {
            //     ctx.submit_command(LogEntryController::CMD_HOT.with(data.1.from.clone()));
            // },

            _ => child.lifecycle(ctx, event, data, env),
        }
    }
}

pub struct ActivityLogWidget {
    inner: WidgetPod<AppModel, Container<AppModel>>,
    hot: Option<Rc<DateTime<Utc>>>,
}

impl ActivityLogWidget {
    pub fn new() -> ActivityLogWidget
    {
        let flex = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start)

            .with_child(
                    List::new(||{
                        Label::new(|((model, _killed), record): &TimeRecordCtx, _env: &_| {
                            if let Some(task) = model.tasks.get(&record.uid) {
                                format_time_record(&task, &record)
                            } else {
                                "".to_string()
                            }
                        })
                        .with_font(FONT_LOG_DESCR.clone())

                        .padding(6.0)
                        .controller(LogEntryController)
                        .on_click(|_ctx, ((data, action), what): &mut TimeRecordCtx, _env| {
                            if data.records_killed.contains(&what.from) {
                                *action = LogEdit::Restored(what.from.clone());
                            } else {
                                *action = LogEdit::Killed(what.from.clone());
                            }
                        })
                        .background(
                            Painter::new(|ctx: &mut PaintCtx, ((model, _), record): &TimeRecordCtx, _env| {
                                let bounds = ctx.size().to_rect();

                                let line =kurbo::Line::new(Point::new(bounds.min_x(), bounds.center().y), 
                                                           Point::new(bounds.max_x(), bounds.center().y));
                                
                                match (model.records_killed.contains(&record.from), ctx.is_hot()) {
                                    (true, false) => ctx.stroke(line.clone(), &COLOR_ACTIVE, 2.0),
                                    (true, true) => ctx.stroke(line.clone(), &RESTORED_TASK_BORDER, 2.0),
                                    (false, true) => ctx.stroke(line.clone(), &DELETING_TASK_BORDER, 2.0),
                                    _ => {},
                                }
                            }))
                    })
            .padding((0.0, 0.0, 15.0, 0.0))
            .lens(lens::Identity.map(
                |m: &AppModel| ((m.clone(),
                                LogEdit::None),
                                m.records.values().map(|v| v.clone()).rev().collect()),

                |outer: &mut AppModel, ((_inner, action), _) : ((AppModel, LogEdit), Vector<TimeRecord>)|
                {
                      match action {
                        LogEdit::None => {},
                        LogEdit::Killed(ref ts) => {
                            if let Some(rec) = outer.records.get(&ts) {
                                if let Err(what) = db::remove_time_record(outer.db.clone(), &rec) {
                                    println!("db error: {}", what);                                
                                }
                            }

                            outer.records_killed = Rc::new(outer.records_killed.update(*ts.clone()));
                            let mut sums = TaskSums::new();
                            
                            for (uid, _) in &outer.tasks {
                                let sum = build_time_prefix_sum(&outer.tasks, &outer.records,
                                                                uid.clone(), &outer.records_killed);
                                sums.insert(uid.clone(), sum);
                            }
                            
                            outer.task_sums = sums;
                        },
                        LogEdit::Restored(ref ts) => {
                            if let Some(rec) = outer.records.get(&ts) {
                                if let Err(what) = db::add_time_record(outer.db.clone(), &rec) {
                                    println!("db error: {}", what);
                                }
                            }

                            outer.records_killed = Rc::new(outer.records_killed.without(ts));                            
                            let mut sums = TaskSums::new();
                            
                            for (uid, _) in &outer.tasks {
                                let sum = build_time_prefix_sum(&outer.tasks, &outer.records,
                                                                uid.clone(), &outer.records_killed);
                                sums.insert(uid.clone(), sum);
                            }
                            
                            outer.task_sums = sums;                            
                        }
                    }
                },
            )));

        ActivityLogWidget {inner: WidgetPod::new(Container::new(flex)), hot: None}
    }
}

impl Widget<AppModel> for ActivityLogWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event,
             data: &mut AppModel, _env: &Env) {

        match event {
            Event::Command(cmd) if cmd.is(LogEntryController::CMD_HOT) => {
                ctx.set_handled();

                let value = cmd.get(LogEntryController::CMD_HOT).unwrap().clone();

                if let Some(prev_hot) = &self.hot {
                    if !prev_hot.same(&value) {
                        self.hot = Some(value);
                    }
                } else {
                    self.hot = Some(value);
                }

                ctx.request_paint();
            },
            Event::Command(cmd) if cmd.is(LogEntryController::CMD_COLD) => {
                ctx.set_handled();
                self.hot = None;
            },
            _ => self.inner.event(ctx, event, data, _env),
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _data: &AppModel, _env: &Env) {
        match event {

            _ => self.inner.lifecycle(ctx, event, _data, _env)
        };
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &AppModel, _data: &AppModel, _env: &Env) {
        self.inner.update(_ctx, _data, _env)
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &AppModel, env: &Env,
    ) -> Size {
        let ret = self.inner.layout(ctx, bc, data, env);
        self.inner.set_origin(ctx, &data, env, Point::new(10.0, 10.0));
        ret
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppModel, env: &Env) {
        self.inner.paint(ctx, data, env);
    }
}

