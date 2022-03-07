use std::rc::Rc;
use chrono::Duration;
use chrono::prelude::*;

use druid::im::{Vector, OrdSet};
use druid::lens::{self, LensExt};
use druid::widget::{Button, Either, CrossAxisAlignment, Flex, Split, Label, List, Scroll, Controller, Painter, Radio, SizedBox};

use druid::{Color, Cursor, LinearGradient,
    Data, PaintCtx, RenderContext, Env, Event, EventCtx, LifeCycle, LifeCycleCtx,
    UnitPoint, Widget, WidgetExt, Target, TextAlignment, Command};

use crate::editable_label;
use crate::editable_label::EditableLabel;
use crate::task::*;
use crate::common::*;
use crate::time;
use crate::widgets;

const COLORS_COUNT: usize = 6;
const GRADIENT_COLORS: [Color; COLORS_COUNT] =
    [Color::RED, Color::YELLOW, Color::GREEN, Color::BLUE, Color::NAVY, Color::PURPLE];

pub fn task_summary_widget() -> impl Widget<((Task, TaskViewState, TimePrefixSum), bool)> {
    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    // merge two columns, add AppModel::Summary::restrospective_i
    // parameter to show [n*i; n*(i+1)] recent entries
    // todo: draw smth like calendar here

    let days_label =
        Label::new(|(_, view_state, sum): &(Task, TaskViewState, TimePrefixSum), _env: &_| {
            let mut result = String::new();
            let now = time::daystart(Local::now());

            for i in 0..7 {
                let date = now.checked_sub_signed(Duration::days(i)).unwrap();
                let prev_date = now.checked_sub_signed(Duration::days(i+1)).unwrap();
                let duration = get_total_time(sum, &prev_date, &date);

                // if duration.is_zero() { continue; }

                let weekday = date.naive_local().weekday().to_string();

                let day = format!("{}, {}", &weekday, &date.format("%d %b").to_string());
                result.push_str(&day);

                result.push_str("    ");
                result.push_str(&format!("{:>8}", &time::format_duration(&duration)));
                result.push_str("\n");
            }
            
            return result;
        });

    column.add_default_spacer();
    column.add_child(
        Split::columns(
                Flex::column()
                    .with_child(Label::new("Total time").with_font(FONT_CAPTION_DESCR.clone()))
                    .with_default_spacer()
                .with_child(widgets::duration_widget()
                            .lens(lens::Map::new(
                                |(_task, _vs, sum): &(Task, TaskViewState, TimePrefixSum)|
                                Rc::new(time::get_duration(sum, &Local::now())),
                                |_, _| {})))
                ,
                Flex::column()
                    .with_child(Label::new("Retrospective").with_font(FONT_CAPTION_DESCR.clone()))
                    .with_default_spacer()
                    .with_child(
                        Flex::row()
                            .with_child(Scroll::new(days_label.with_font(FONT_LOG_DESCR.clone())))
                            .padding(10.0)
                            .background(
                                Painter::new(|ctx: &mut PaintCtx, _item: &_, _env| {
                                    let bounds = ctx.size().to_rect();
                                    ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                                })))
        ).bar_size(0.0));

    Either::new(|((_, _, _), visible): &((Task, TaskViewState, TimePrefixSum), bool), _env: &Env|
                *visible == true,
                column
                .lens(druid::lens!(((Task, TaskViewState, TimePrefixSum), bool), 0)),
                SizedBox::empty().expand_width())
}

pub fn task_edit_widget() -> impl Widget<(Task, bool)> {
    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Name").with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(
                EditableLabel::parse()
                    .with_id(TASK_NAME_EDIT_WIDGET)
                    .padding(10.0)
                    .fix_height(50.0)
                    .lens(lens::Identity.map(
                        |d: &Task| d.name.clone(),
                        |d: &mut Task, x: String| {
                            d.name = x;
                        },
                    ))),
    );

    column.add_spacer(10.0);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Status") .with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(Radio::new("needs action" , TaskStatus::NeedsAction))
            .with_child(Radio::new("in process"   , TaskStatus::InProcess))
            .with_child(Radio::new("completed"    , TaskStatus::Completed))
            .lens(lens::Map::new(
                |task: &Task| task.task_status.clone(),
                |task: &mut Task, status| task.task_status = status))
    );

    column.add_spacer(25.0);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Priority") .with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(Radio::new("low"         , CuaPriority::Low))
            .with_child(Radio::new("normal"      , CuaPriority::Normal))
            .with_child(Radio::new("high"        , CuaPriority::High))
            .lens(lens::Map::new(
                |task: &Task| task.priority.into(),
                |task: &mut Task, pri: CuaPriority| task.priority = pri.into()))
    );

    column.add_spacer(15.0);

    let new_tag_edit = EditableLabel::parse()
            .lens(lens::Map::new(
                |_task: &Task| "".to_string(),
                |task: &mut Task, new_tag| {
                    if !new_tag.is_empty() {
                        task.tags = task.tags.update(new_tag);
                    }
                }));

    let tags_list =
        List::new(|| {
            Flex::row()
                .with_child(
                    Label::new(|(_, item) : &(OrdSet<String>, String), _env: &_| format!("{} ⌫", item))
                        .on_click(|_ctx, (lst, item): &mut (OrdSet<String>, String), _env| *lst = lst.without(item))
                        .align_horizontal(UnitPoint::LEFT)
                        .padding(10.0))
                .background(
                    Painter::new(|ctx: &mut PaintCtx, _item: &_, _env| {
                        let bounds = ctx.size().to_rect();
                        ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                    }))

        })
        .with_spacing(10.0)
        .horizontal()
        .lens(lens::Identity.map(
            |data: &Task| {
                (data.tags.clone(),
                 data.tags.iter().map(|x: &String| {x.clone()}).collect())
            },
            |data: &mut Task, tags: (OrdSet<String>, Vector<String>)| {
                if !data.tags.same(&tags.0) {
                    data.tags = tags.0;
                }
            }));

    column.add_child(
        Flex::row()
            .with_child(Label::new("Tags").with_font(FONT_CAPTION_DESCR.clone()))
            .with_spacer(20.0)
            .with_child(new_tag_edit.padding(10.0))
            .with_default_spacer()
            .with_child(
                Scroll::new(tags_list)

            ));

    column.add_spacer(15.0);

    column.add_child(Label::new("Description").with_font(FONT_CAPTION_DESCR.clone()));
    column.add_default_spacer();
    column.add_child(
        EditableLabel::parse()
            .expand_width()
            .lens(lens::Identity.map(
                |d: &Task| d.description.clone(),
                |d: &mut Task, x: String| {
                    d.description = x;
                },
            )));

    column.add_spacer(15.0);

    column.add_child(
        Flex::row()
            .with_child(Label::new("Work/rest duration").with_font(FONT_CAPTION_DESCR.clone()))
            .with_default_spacer()
            .with_child(
                EditableLabel::parse()
                    .with_text_alignment(TextAlignment::End)
                    .lens(lens::Identity.map(
                        |d: &Task| d.work_duration.num_minutes(),
                        |d: &mut Task, x: i64| {
                            if *d.work_duration != Duration::minutes(x){
                                d.work_duration = Rc::new(Duration::minutes(x));
                            }
                        },
                    )).fix_width(40.0))
            .with_default_spacer()
            .with_child(
                EditableLabel::parse()
                    .with_text_alignment(TextAlignment::End)
                    .lens(lens::Identity.map(
                        |d: &Task| d.break_duration.num_minutes(),
                        |d: &mut Task, x: i64| {
                            if *d.break_duration != Duration::minutes(x){
                                d.break_duration = Rc::new(Duration::minutes(x));
                            }
                        },
                    )).fix_width(40.0))
            .with_default_spacer()
            .with_child(Label::new("min").with_font(FONT_CAPTION_DESCR.clone()))
    );

    column.add_spacer(15.0);

    column.add_child(
        Flex::row()
        .with_child(Label::new("Color").with_font(FONT_CAPTION_DESCR.clone()))
        .with_default_spacer()
        .with_flex_child(
            SizedBox::new(Painter::new(|ctx, _data: &_, _env| {
                let bounds = ctx.size().to_rect();
                let gradient = LinearGradient::new(
                    UnitPoint::LEFT,
                    UnitPoint::RIGHT,
                    &GRADIENT_COLORS[0..],
                );
                ctx.fill(bounds, &gradient);
            })).expand_width().height(25.0).controller(ColorPickerController)
                , 1.0)
    );


    // DropdownSelect from widget nursery creates separated window
    // column.add_flex_child(
    //     DropdownSelect::new(vec![
    //         ("needs action" , TaskStatus::NeedsAction),
    //         ("in process"   , TaskStatus::InProcess),
    //         ("completed"    , TaskStatus::Completed),
    //         ("cancelled"    , TaskStatus::Cancelled),
    //     ])
    //     .align_left()
    //     .lens(Task::task_status),
    //     1.0,
    // );

    Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)

        .with_flex_child(
            Either::new(|(_, visible): &(Task, bool), _env: &Env| *visible == true,
                        column
                        .controller(TaskDetailsController)
                        .with_id(TASK_EDIT_WIDGET)
                        .lens(druid::lens!((Task, bool), 0)),
                        SizedBox::empty().expand_width()),
            1.0)

        .with_child(Button::dynamic(move |data, _env| (if *data {"▲"} else {"Edit task ▼"}).to_string())
                    .on_click(|_ctx, visible: &mut bool, _env| {
                        *visible = !*visible;
                    })
                    .lens(druid::lens!((Task, bool), 1)))
}

struct TaskDetailsController;

impl<T, W: Widget<T>> Controller<T, W> for TaskDetailsController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {

        match event {
            Event::Command(cmd) if cmd.is(COMMAND_EDIT_REQUEST_FOCUS) => {
                let command =
                    Command::new(editable_label::BEGIN_EDITING, (),
                        Target::Widget(cmd.get(COMMAND_EDIT_REQUEST_FOCUS).unwrap().clone()));
                ctx.submit_command(command);
                ctx.set_handled();

            },

            Event::Notification(cmd) if cmd.is(editable_label::FOCUS_RESIGNED) => {
                ctx.set_handled();
                ctx.submit_command(COMMAND_TLIST_REQUEST_FOCUS.with(()));
            },

            Event::KeyUp(key) if key.code == druid::Code::Tab => {
                child.event(ctx, event, data, env);
                ctx.set_handled();
            },


            _ => child.event(ctx, event, data, env),
        }
    }

    fn lifecycle(&mut self, child: &mut W, ctx: &mut LifeCycleCtx, event: &LifeCycle,
                 data: &T, env: &Env)
    {
        match event {
            _ => child.lifecycle(ctx, event, data, env)
        }
    }
}

struct ColorPickerController;

fn lerp(a: &druid::Color, b: &druid::Color, t: f64) -> druid::Color {
    match (a.as_rgba(), b.as_rgba()) {
        ((r1, g1, b1, a1), (r2, g2, b2, a2)) => {
            druid::Color::rgba(
                lerp::Lerp::lerp(r1, r2, t),
                lerp::Lerp::lerp(g1, g2, t),
                lerp::Lerp::lerp(b1, b2, t),
                lerp::Lerp::lerp(a1, a2, t))
        }
    }
}

impl<W: Widget<Task>> Controller<Task, W> for ColorPickerController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut Task, env: &Env) {

        match event {
            Event::MouseUp(ref mouse) => {
                let idx_f = (COLORS_COUNT-1) as f64 * mouse.pos.x / ctx.size().width;
                let left_idx = idx_f.floor().round() as usize;
                let right_idx = idx_f.ceil().round() as usize;
                data.color = lerp(&GRADIENT_COLORS[left_idx], &GRADIENT_COLORS[right_idx],
                                  idx_f - left_idx as f64);
            }
            _ => child.event(ctx, event, data, env),
        }

        ctx.set_cursor(&Cursor::Crosshair);
    }
}
