// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use std::rc::Rc;
use std::time::SystemTime;
use std::env;

use druid::widget::prelude::*;
use druid::im::{vector, Vector};
use druid::lens::{self, LensExt};
use druid::widget::{CrossAxisAlignment, Flex, Label, SizedBox, List, Scroll, Container, Painter};

use druid::{
    AppLauncher, Application, Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    LifeCycle, Point,
    Menu, MenuItem, TimerToken, KeyOrValue,
    LocalizedString, UnitPoint, Widget, WidgetPod, WidgetExt, WindowDesc, WindowId};

use chrono::prelude::*;

use netupi::maybe::Maybe;
use netupi::task::*;
use netupi::db;
use netupi::app_model::*;
use netupi::task_list::*;
use netupi::task_details::*;
use netupi::activity_log::*;
use netupi::common::*;
use netupi::time;
use netupi::widgets;

pub fn main() -> anyhow::Result<()> {
    let _args: Vec<String> = env::args().collect();

    let conn = db::init()?;
    let db = Rc::new(conn);

    let (tasks, tags) = db::get_tasks(db.clone())?;
    let records = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone(), &TimeRecordSet::new());
        task_sums.insert(uid.clone(), sum);
    }

    let selected_task = "".to_string();

    let mut data = AppModel{
        db,
        tasks,
        records,
        records_killed: Rc::new(TimeRecordSet::new()),
        task_sums,
        tags,
        tracking: TrackingCtx{state: TrackingState::Inactive,
                              timestamp: Rc::new(Utc::now()),
                              timer_id: Rc::new(TimerToken::INVALID),
                              elapsed: Rc::new(chrono::Duration::zero())},
        selected_task: selected_task,
        focus_filter: FocusFilter::All, // select filter of last tracked task
        tag_filter: None,
        hot_log_entry: None,
        show_task_edit: true,
        show_task_summary: true,
    };

    let selected = data.get_uids_filtered().front().unwrap_or(&"".to_string()).clone();
    data.selected_task = selected;

    // TODO should be done in ctor
    data.update_tags();

    let main_window = WindowDesc::new(ui_builder())
        .window_size((1200.0, 800.0))
        .menu(make_menu)
        .title(LocalizedString::new("netupi-window-title").with_placeholder("netupi"));
    
    AppLauncher::with_window(main_window)
        .log_to_console()
        .launch(data)
        .expect("launch failed");

    Ok(())
}



#[allow(unused_assignments)]
fn make_menu(_: Option<WindowId>, model: &AppModel, _: &Env) -> Menu<AppModel> {
    let base = Menu::empty();

    let mut file = Menu::new(LocalizedString::new("File"));

    file = file.entry(
        MenuItem::new(LocalizedString::new("Exit"))
            .on_activate(move |_ctx, _data, _env| {Application::global().quit();})
    );
    
    let mut task = make_task_menu(model, &model.selected_task);
    task = task.rebuild_on(|prev: &AppModel, now: &AppModel, _env: &Env| {
        !prev.tasks.same(&now.tasks) |
        !prev.selected_task.same(&now.selected_task) |
        !prev.tracking.same(&now.tracking)
    });

    base.entry(file).entry(task)
}


struct StatusBar {
    inner: WidgetPod<String, Label<String>>,
    timer_id: TimerToken,
}

fn get_status_string(d: &AppModel) -> String {
    match d.tracking.state {
        TrackingState::Active(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            let duration = d.tracking.elapsed.checked_add(&Utc::now()
                .signed_duration_since(d.tracking.timestamp.as_ref().clone()))
                .unwrap_or(chrono::Duration::zero());

            let total = get_work_interval(d, uid);

            format!("Active: '{}' | Elapsed: {} / {}",
                    active_task.name, time::format_duration(&duration), time::format_duration(&total))
        },
        TrackingState::Break(ref uid) => {
            let rest_task = &d.tasks.get(uid).expect("unknown uid");

            let duration =
                Utc::now().signed_duration_since(d.tracking.timestamp.as_ref().clone());

            let total = get_rest_interval(d, uid);

            format!("Break: '{}' | Elapsed: {} / {}",
                    rest_task.name, time::format_duration(&duration), time::format_duration(&total))
        },
        TrackingState::Paused(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            format!("Paused: '{}' | Elapsed: {} / {}",
                    active_task.name,
                    time::format_duration(&d.tracking.elapsed),
                    time::format_duration(&get_work_interval(d, uid)))
        },

        _ => format!("")
    }

}

impl StatusBar {
    fn new() -> StatusBar {
        StatusBar{inner: WidgetPod::new(Label::dynamic(|d: &String, _env| d.clone())),
                  timer_id: TimerToken::INVALID}
    }
}

impl Widget<AppModel> for StatusBar {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppModel, env: &Env) {

        if self.timer_id == TimerToken::INVALID {
            self.timer_id = ctx.request_timer(UI_TIMER_INTERVAL);
        }

        let mut status = get_status_string(&data);

        match event {
            Event::Timer(id) => {
                if *id == self.timer_id {
                    self.timer_id = ctx.request_timer(UI_TIMER_INTERVAL);
                    ctx.request_update();
                    self.inner.event(ctx, event, &mut status, env);
                }
            },
            _ => {self.inner.event(ctx, event, &mut status, env)},
        }
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.lifecycle(ctx, event, &status, env);
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &AppModel, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.update(ctx, &status, env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &AppModel, env: &Env) -> Size {
        let status = get_status_string(&data);
        let ret = self.inner.layout(ctx, &bc.loosen(), &status, env);
        self.inner.set_origin(ctx, &status, env, Point::ORIGIN);
        return ret;
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppModel, env: &Env) {
        let status = get_status_string(&data);
        self.inner.paint(ctx, &status, env);
    }
}

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    let mut tasks_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    focus_column.add_child(Label::new("Focus")
                           .with_font(FONT_CAPTION_DESCR.clone()));

    focus_column.add_spacer(10.0);

    focus_column.add_child(
        List::new(|| {
            Container::new(
                Label::new(|item: &(AppModel, FocusFilter), _env: &_| format!("{}", item.1.to_string()))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .background(
                        Painter::new(|ctx: &mut PaintCtx, (shared, filter): &(AppModel, FocusFilter), _env| {
                            let bounds = ctx.size().to_rect();
                            if shared.focus_filter.eq(filter) {
                                ctx.fill(bounds, &TASK_COLOR_BG);
                            }
                            else {
                                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                            }
                        })
                    )
            )
            .on_click(|_ctx, (model, what): &mut (AppModel, FocusFilter), _env| {
                model.focus_filter = what.clone();
                model.check_update_selected();
            })
        })
        .with_spacing(10.0)
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), vector![FocusFilter::Status(TaskStatus::NeedsAction),
                                               FocusFilter::Status(TaskStatus::InProcess),
                                               FocusFilter::Status(TaskStatus::Completed),
                                               FocusFilter::All]),
            |d: &mut AppModel, x: (AppModel, Vector<FocusFilter>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        ))
    );

    focus_column.add_spacer(15.0);

    focus_column.add_child(Label::new("Tags")
                           .with_font(FONT_CAPTION_DESCR.clone()));

    focus_column.add_default_spacer();

    focus_column.add_flex_child(
        Scroll::new(
            List::new(|| {
                Container::new(
                    Label::new(|item: &(AppModel, String), _env: &_| format!("{}", item.1))
                        .align_vertical(UnitPoint::LEFT)
                        .padding(10.0)
                        .background(
                            Painter::new(|ctx: &mut PaintCtx, (shared, id): &(AppModel, String), _env| {
                                let bounds = ctx.size().to_rect();
                                if shared.tag_filter.is_some() &&
                                    shared.tag_filter.as_ref().unwrap().eq(id) {
                                        ctx.fill(bounds, &TASK_COLOR_BG);
                                    }
                                else {
                                    ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                                }
                            })
                        )
                )
                    .on_click(|_ctx, (data, what): &mut (AppModel, String), _env| {
                        data.tag_filter = match data.tag_filter {
                            Some(ref filter) if filter.eq(what) => None,
                            Some(_)                             => Some(what.clone()),
                            None                                => Some(what.clone())
                        };

                        data.check_update_selected();
                    })
            })
                .with_spacing(10.0)
                .padding((0.0, 0.0, 15.0, 0.0)))
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.tags.iter().map(|x : &String| {x.clone()}).collect()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        )),
        1.0
    );

    main_row.add_child(focus_column.padding(10.0));
    main_row.add_default_spacer();

    let task_list_widget = TaskListWidget::new()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.get_uids_filtered()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        ));

    // Build a list with shared data
    tasks_column.add_flex_child(task_list_widget, 2.0);

    tasks_column.add_spacer(10.0);

    tasks_column.add_child(
        Maybe::new(
            || task_edit_widget().boxed(),
            || SizedBox::empty().expand_width().boxed(),
        )
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel|
                match d.tasks.get(&d.selected_task)
                {
                    Some(task) => Some((task.clone(), d.show_task_edit)),
                    _ => None,
                },

                |d: &mut AppModel, x: Option<(Task, bool)>| {
                    if let Some((mut new_task, vis)) = x {
                        d.show_task_edit = vis;

                        if let Some(prev) = d.tasks.get(&d.selected_task) {
                            if !prev.same(&new_task) {

                                if let Err(what) = db::update_task(d.db.clone(), &new_task) {
                                    println!("db error: {}", what);
                                }

                                new_task.seq += 1;

                                let mut deleted_filter = None;

                                if let Some(ref filt) = d.tag_filter {
                                    if !new_task.tags.contains(filt) {
                                        deleted_filter = Some(filt.clone());
                                    }
                                }

                                d.tasks = d.tasks.update(d.selected_task.clone(), new_task);
                                d.update_tags();

                                // if currently select tag filter is missing
                                // from updated task and this tag isn't
                                // present anymore in other tags then clear
                                // tag filter

                                if let Some(ref filt) = deleted_filter {
                                    if !d.tags.contains(filt) {
                                        d.tag_filter = None;
                                    }
                                }

                                d.check_update_selected();
                            }
                        }
                    }
                },
            )),
    );

    tasks_column.add_spacer(15.0);

    tasks_column.add_child(
        Maybe::new(
            || task_summary_widget().boxed(),
            || SizedBox::empty().expand_width().boxed(),
        )
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel|
                match (d.tasks.get(&d.selected_task).map_or(None, |r| Some(r.clone())),
                       d.task_sums.get(&d.selected_task).map_or(TimePrefixSum::new(), |r| r.clone()))
                {
                    (Some(task), time) => Some(((task, time), d.show_task_summary)),
                    _ => None,
                },

                |d: &mut AppModel, x: Option<((Task, TimePrefixSum), bool)>| {
                    if let Some(((_, _), vis)) = x {
                        d.show_task_summary = vis;
                    }
                },
            )),
    );


    main_row.add_flex_child(tasks_column
                            .padding(10.0)
                            .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0),
                            2.0);

    let mut time_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    time_column.add_child(Label::new("Total time log")
                          .with_font(FONT_CAPTION_DESCR.clone())
                          .padding(10.0));

    time_column.add_child(
        widgets::duration_widget()
            .lens(lens::Identity.map(
                |model: &AppModel| Rc::new(time::get_durations(&model.task_sums)),
                |_, _ | {},
        )));

    time_column.add_default_spacer();

    time_column.add_child(Label::new("Activity log").with_font(FONT_CAPTION_DESCR.clone()).padding(10.0));

    time_column.add_flex_child(
        Scroll::new(ActivityLogWidget::new())
            .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0), 1.0);

    main_row.add_child(time_column);

    root.add_flex_child(main_row, 1.0);

    // bottom row 
    // root.add_child(
    //     Button::new("Save")
    //         .on_click(|_ctx, (model): &mut (AppModel), _env| {
    //             // todo dont clone IcalCalendar
    //             // let newcal = update_ical(&mut IcalCalendar::clone(&model.cal), &model.tasks);
    //             // emit(&newcal);
    //             // model.cal = Rc::new(newcal)
    //         })
    //         .fix_size(120.0, 20.0)
    //         .align_vertical(UnitPoint::CENTER),
    // );

    root.with_child(Container::new(StatusBar::new()
                                   .align_horizontal(UnitPoint::CENTER))
                    .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0),
    )
        // .debug_paint_layout()
}

