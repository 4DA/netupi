use druid::im::{Vector, OrdSet};
use druid::lens::{self, LensExt};
use druid::widget::{CrossAxisAlignment, Flex, Label, List, Scroll, Controller, Painter, Radio};

use druid::{
    Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    FontWeight, FontDescriptor, FontFamily,
    UnitPoint, Widget, WidgetExt, Target, Command};

use crate::editable_label;
use crate::editable_label::EditableLabel;
use crate::task::*;
use crate::constants::*;

use chrono::prelude::*;

pub fn task_details_widget() -> impl Widget<(Task, TimePrefixSum)> {
    static FONT_CAPTION_DESCR: FontDescriptor =
        FontDescriptor::new(FontFamily::SYSTEM_UI)
        .with_weight(FontWeight::BOLD)
        .with_size(16.0);

    let mut column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let edit_widget = task_edit_widget().lens(druid::lens!((Task, TimePrefixSum), 0));
    column.add_child(edit_widget);

    column.add_spacer(15.0);

    column.add_child(Label::new("Task time log").with_font(FONT_CAPTION_DESCR.clone()));
    column.add_default_spacer();
    column.add_child(
        Flex::row()
        .with_child(Label::new("Today\nWeek\nMonth\nTotal"))
        .with_child(
            Label::new(|(_, sum): &(Task, TimePrefixSum), _env: &_| {
                let mut result = String::new();

                let now = Local::now();
                let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));

                let epoch = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
                let total_day = get_total_time(sum, &day_start);

                let total = get_total_time(sum, &epoch);

                result.push_str(&format_duration(total_day.clone()));
                result.push_str("\n");

                Utc.from_local_datetime(
                    &NaiveDate::from_isoywd(now.year(), now.iso_week().week(), Weekday::Mon)
                        .and_time(NaiveTime::from_hms(0,0,0)))
                    .single()
                    .map(|utc| result.push_str(&format_duration(get_total_time(sum, &utc))));
                result.push_str("\n");

                Utc.from_local_datetime(
                    &NaiveDate::from_ymd(now.year(), now.month(), 1)
                        .and_time(NaiveTime::from_hms(0, 0, 0)))
                    .single()
                    .map(|utc| result.push_str(&format_duration(get_total_time(sum, &utc))));
                result.push_str("\n");

                result.push_str(&format_duration(total.clone()));

                return result;
            }))
            .padding(10.0)
            .background(
                Painter::new(|ctx: &mut PaintCtx, _item: &_, _env| {
                    let bounds = ctx.size().to_rect();
                    ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                }))
    );

    return column.controller(TaskDetailsController);
}

fn task_edit_widget() -> impl Widget<Task> {
    static FONT_CAPTION_DESCR: FontDescriptor =
        FontDescriptor::new(FontFamily::SYSTEM_UI)
        .with_weight(FontWeight::BOLD)
        .with_size(16.0);

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
            .with_child(Radio::new("cancelled"    , TaskStatus::Cancelled))
            .lens(lens::Map::new(
                |task: &Task| task.task_status.clone(),
                |task: &mut Task, status| task.task_status = status))
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

    return column;
}

struct TaskDetailsController;

impl<T, W: Widget<T>> Controller<T, W> for TaskDetailsController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {

        match event {
            Event::Command(cmd) if cmd.is(COMMAND_DETAILS_REQUEST_FOCUS) => {
                let command = Command::new(editable_label::BEGIN_EDITING, (),
                                           Target::Widget(TASK_NAME_EDIT_WIDGET));
                ctx.submit_command(command);

            },

            _ => child.event(ctx, event, data, env),
        }
    }
}
