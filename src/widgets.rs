use std::rc::Rc;
use druid::widget::{Flex, Label, Painter};
use druid::{Widget, WidgetExt, PaintCtx, RenderContext};

use crate::common::*;
use crate::time;

pub fn duration_widget() -> impl Widget<Rc<time::AggregateDuration>> {
    let label = Label::new(|duration: &Rc<time::AggregateDuration>, _env: &_| {
        let mut result = String::new();

        result.push_str(&format!("{:>12}", time::format_duration(&duration.day)));
        result.push_str("\n");

        result.push_str(&format!("{:>12}", time::format_duration(&duration.week)));
        result.push_str("\n");

        result.push_str(&format!("{:>12}", time::format_duration(&duration.month)));
        result.push_str("\n");

        result.push_str(&format!("{:>12}", time::format_duration(&duration.year)));
        result.push_str("\n");

        result.push_str(&format!("{:>12}", time::format_duration(&duration.total)));

        return result;
    }).with_font(FONT_LOG_DESCR.clone());

    Flex::row()
        .with_child(Label::new("Today\nWeek\nMonth\nYear\nAll time")
                    .with_font(FONT_LOG_DESCR.clone()))
        .with_default_spacer()
        .with_child(label)
        .padding(10.0)
        .background(
            Painter::new(|ctx: &mut PaintCtx, _item: &_, _env| {
                let bounds = ctx.size().to_rect();
                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
            }))
}

