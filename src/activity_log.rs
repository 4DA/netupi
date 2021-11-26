use std::rc::Rc;
use std::time::SystemTime;

use druid::widget::prelude::*;
use druid::im::{Vector};
use druid::lens::{self, LensExt};

use druid::widget::{CrossAxisAlignment, Controller, Flex, Label, List, Container, Painter};

use druid::{
    Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    LifeCycle, FontDescriptor, FontFamily, Point, Widget, WidgetPod, WidgetExt};

use druid::{Selector};

use chrono::prelude::*;

use crate::task::*;
use crate::app_model::*;
use crate::common::*;

type TimeRecordCtx = (AppModel, TimeRecord);

struct LogEntryController;

impl LogEntryController {
    const CMD_HOT: Selector<Rc<DateTime<Utc>>> = Selector::new("alog_entry_hot");
    const CMD_COLD: Selector = Selector::new("alog_entry_cold");
}

impl<W: Widget<TimeRecordCtx>> Controller<TimeRecordCtx, W> for LogEntryController {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut TimeRecordCtx,
        env: &Env,
    ) {
        match event {
            _ => child.event(ctx, event, data, env),
        }
    }

    fn lifecycle(&mut self, child: &mut W, ctx: &mut LifeCycleCtx<'_, '_>,
                     event: &LifeCycle, data: &TimeRecordCtx, env: &Env)
    {
        match event {
            _ => child.lifecycle(ctx, event, data, env),
        }
    }
}




pub struct ActivityLogWidget {
    inner: WidgetPod<AppModel, Container<AppModel>>,
    hot: Option<Rc<DateTime<Utc>>>,
}

fn paint_log_entry(ctx: &mut PaintCtx, (_shared, __record): &TimeRecordCtx) {
    let bounds = ctx.size().to_rect();

    if ctx.is_hot() {
        ctx.stroke(bounds, &TASK_ACTIVE_COLOR_BG, 2.0);
    }
}

impl ActivityLogWidget {
    pub fn new() -> ActivityLogWidget
    {
        static FONT_LOG_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::MONOSPACE);

        let flex = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start)

            .with_child(
                    List::new(||{
                        Label::new(|(model, record): &TimeRecordCtx, _env: &_| {
                            if let Some(task) = model.tasks.get(&record.uid) {
                                let mut name =  format!("{}", task.name);
                                name.truncate(15);

                                let duration = format_duration(record.to.signed_duration_since(*record.from));

                                let now: DateTime<Local> = DateTime::from(SystemTime::now());
                                let when: DateTime<Local> = DateTime::<Local>::from(*record.from);

                                let time = if now.year() > when.year() {
                                    when.format("%-d %b %y, %H:%M").to_string()
                                } else if now.ordinal() > when.ordinal() {
                                    when.format("%-d %b, %H:%M").to_string()
                                } else {
                                    when.format("%H:%M").to_string()
                                };

                                format!("{:<15} {:<10} {:<10}", name, duration, time)
                            } else {
                                "".to_string()
                            }
                        })
                        .with_font(FONT_LOG_DESCR.clone())
                        .on_click(|_ctx, (_data, _what): &mut TimeRecordCtx, _env| {})
                        .background(
                            Painter::new(|ctx: &mut PaintCtx, data: &TimeRecordCtx, _env| {
                                paint_log_entry(ctx, data);
                            }))
                    })
                    .with_spacing(10.0))
            .padding((0.0, 0.0, 15.0, 0.0))
            .lens(lens::Identity.map(
                |m: &AppModel| (m.clone(),
                                m.records.values().map(|v| v.clone()).rev().collect()),

                |_outer: &mut AppModel, _inner: (AppModel, Vector<TimeRecord>)| {
                },
            ));

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

