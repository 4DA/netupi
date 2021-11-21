// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use std::rc::Rc;
use std::any::type_name;
use std::time::SystemTime;
use std::env;


use druid::widget::prelude::*;
use druid::im::{vector, Vector, OrdSet};
use druid::lens::{self, LensExt};
use druid::widget::{CrossAxisAlignment, Flex, Label, SizedBox, List, Scroll, Container, Painter};

use druid::{
    AppLauncher, Application, Data, PaintCtx, RenderContext, Env, Event, EventCtx,
    FontWeight, FontDescriptor, FontFamily, Point,
    Menu, MenuItem, TimerToken, KeyOrValue,
    LocalizedString, UnitPoint, Widget, WidgetPod, WidgetExt, WindowDesc, WindowId};

use chrono::prelude::*;

mod editable_label;

mod maybe;
use crate::maybe::Maybe;

mod task;
use task::*;

mod db;

mod app_model;
use app_model::*;

mod task_list;
use task_list::*;

mod task_details;
use task_details::*;

mod constants;
use constants::*;


#[allow(unused)]
fn type_of<T>(_: T) -> &'static str {
    type_name::<T>()
}


impl AppModel {
    fn get_uids_filtered(&self) -> impl Iterator<Item = String> + '_ {
        self.tasks.keys().cloned().filter(move |uid| {
            let task = self.tasks.get(uid).expect("unknown uid");

            let focus_ok = match self.focus_filter.as_str() {
                TASK_FOCUS_CURRENT => {task.task_status == TaskStatus::NeedsAction ||
                              task.task_status == TaskStatus::InProcess},
                TASK_FOCUS_COMPLETED => task.task_status == TaskStatus::Completed,
                TASK_FOCUS_ALL => task.task_status != TaskStatus::Archived,
                _ => panic!("Unknown focus filter {}", &self.focus_filter),
            };

            let tag_ok =
                if let Some(ref tag_filter) = self.tag_filter {
                    task.tags.contains(tag_filter)
                } else {
                    true
                };

            return focus_ok && tag_ok;
        })
    }

    fn check_update_selected(&mut self) {
        let filtered: Vector<String> = self.get_uids_filtered().collect();

        // select any task if currently selected is filtered out
        if !filtered.contains(&self.selected_task) {
            self.selected_task = filtered.front().unwrap_or(&"".to_string()).clone();
        }
    }

    fn get_tags(&self) -> OrdSet<String> {
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

    fn update_tags(&mut self) {
        self.tags.clear();
        self.tags = self.get_tags();
    }
}


pub fn main() -> anyhow::Result<()> {
    let _args: Vec<String> = env::args().collect();

    let conn = db::init()?;
    let db = Rc::new(conn);

    let focus = vector![TASK_FOCUS_CURRENT.to_string(),
                        TASK_FOCUS_COMPLETED.to_string(),
                        TASK_FOCUS_ALL.to_string()];

    let (tasks, tags) = db::get_tasks(db.clone())?;
    let records = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone());
        task_sums.insert(uid.clone(), sum);
    }

    let selected_task = "".to_string();

    let mut data = AppModel{
        db,
        tasks,
        records,
        task_sums,
        tags,
        focus,
        tracking: TrackingCtx{state: TrackingState::Inactive,
                              timestamp: Rc::new(Utc::now()),
                              timer_id: Rc::new(TimerToken::INVALID),
                              elapsed: Rc::new(chrono::Duration::zero())},
        selected_task: selected_task,
        focus_filter: TASK_FOCUS_CURRENT.to_string(),
        tag_filter: None
    };

    let selected = data.get_uids_filtered().nth(0).unwrap_or("".to_string()).clone();
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

            let total = get_work_interval(uid);

            format!("Active: '{}' | Elapsed: {} / {}",
                    active_task.name, format_duration(duration), format_duration(total))
        },
        TrackingState::Break(ref uid) => {
            let rest_task = &d.tasks.get(uid).expect("unknown uid");

            let duration =
                Utc::now().signed_duration_since(d.tracking.timestamp.as_ref().clone());

            let total = get_rest_interval(uid);

            format!("Break: '{}' | Elapsed: {} / {}",
                    rest_task.name, format_duration(duration), format_duration(total))
        },
        TrackingState::Paused(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            format!("Paused: '{}' | Elapsed: {} / {}",
                    active_task.name,
                    format_duration(*(&d.tracking.elapsed).clone()),
                    format_duration(get_work_interval(uid)))
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


fn activity_log_widget() -> impl Widget<AppModel> {
    static FONT_CAPTION_DESCR: FontDescriptor =
        FontDescriptor::new(FontFamily::SYSTEM_UI)
        .with_weight(FontWeight::BOLD)
        .with_size(14.0);

    Flex::row()
        .with_child(
            Flex::column().with_child(Label::new("Task")
                                      .with_font(FONT_CAPTION_DESCR.clone()))
            .with_child(
                List::new(||{
                    Label::new(|(model, record): &(AppModel, TimeRecord), _env: &_| {
                        if let Some(task) = model.tasks.get(&record.uid) {
                            format!("{}", task.name)
                        } else {
                            "".to_string()
                        }
                    })
                })
                .with_spacing(10.0)
                .padding(10.0)))

        .with_child(
            Flex::column().with_child(Label::new("Duration")
                                      .with_font(FONT_CAPTION_DESCR.clone()))
            .with_child(
                List::new(||{
                    Label::new(|(model, record): &(AppModel, TimeRecord), _env: &_| {
                        if let Some(_) = model.tasks.get(&record.uid) {
                            format_duration(record.to.signed_duration_since(*record.from))
                        } else {
                            "".to_string()
                        }
                    })
                })
                .with_spacing(10.0)
                .padding(10.0)))
        .with_child(
            Flex::column().with_child(Label::new("When")
                                      .with_font(FONT_CAPTION_DESCR.clone()))
            .with_child(
                List::new(||{
                    Label::new(|(model, record): &(AppModel, TimeRecord), _env: &_| {
                        if let Some(_) = model.tasks.get(&record.uid) {
                            let now: DateTime<Local> = DateTime::from(SystemTime::now());
                            let when: DateTime<Local> = DateTime::<Local>::from(*record.from);

                            let time = if now.year() > when.year() || now.day() > when.day() {
                                when.format("%b %-d %H:%M").to_string()
                            } else {
                                when.format("%H:%M").to_string()
                            };

                            format!("{}", time)
                        } else {
                            "".to_string()
                        }
                    })
                })
                .with_spacing(10.0)
                .padding(10.0)))
        .padding((0.0, 10.0, 15.0, 0.0))
        .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0)
        .lens(lens::Identity.map(
            |m: &AppModel| (m.clone(), m.records.values().map(|v| v.clone()).rev().collect()),
            |_data: &mut AppModel, _m: (AppModel, Vector<TimeRecord>)| {},
        ))
}

fn ui_builder() -> impl Widget<AppModel> {
    let mut root = Flex::column();

    let mut main_row = Flex::row().cross_axis_alignment(CrossAxisAlignment::Start);

    let mut tasks_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);
    let mut focus_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    static FONT_CAPTION_DESCR: FontDescriptor = FontDescriptor::new(FontFamily::SYSTEM_UI)
    .with_weight(FontWeight::BOLD)
    .with_size(18.0);

    focus_column.add_child(Label::new("Focus")
                           .with_font(FONT_CAPTION_DESCR.clone()));

    focus_column.add_spacer(10.0);

    focus_column.add_child(
        List::new(|| {
            Container::new(
                Label::new(|item: &(AppModel, String), _env: &_| format!("{}", item.1))
                    .align_vertical(UnitPoint::LEFT)
                    .padding(10.0)
                    .background(
                        Painter::new(|ctx: &mut PaintCtx, (shared, id): &(AppModel, String), _env| {
                            let bounds = ctx.size().to_rect();
                            if shared.focus_filter.eq(id) {
                                ctx.fill(bounds, &TASK_COLOR_BG);
                            }
                            else {
                                ctx.stroke(bounds, &TASK_COLOR_BG, 2.0);
                            }
                        })
                    )
            )
            .on_click(|_ctx, (model, what): &mut (AppModel, String), _env| {
                model.focus_filter = what.clone();
                model.check_update_selected();
            })
        })
        .with_spacing(10.0)
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.focus.clone()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
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

    let tasks_scroll = Scroll::new(
            TaskListWidget::new()
        )
        .vertical()
        .lens(lens::Identity.map(
            // Expose shared data with children data
            |d: &AppModel| (d.clone(), d.get_uids_filtered().collect()),
            |d: &mut AppModel, x: (AppModel, Vector<String>)| {
                // If shared data was changed reflect the changes in our AppModel
                *d = x.0
            },
        ));

    // Build a list with shared data
    tasks_column.add_flex_child(tasks_scroll, 2.0);

    tasks_column.add_spacer(10.0);

    tasks_column.add_child(
        Maybe::new(
            || task_details_widget().boxed(),
            || SizedBox::empty().expand_width().boxed(),
        )
            .lens(lens::Identity.map(
                // Expose shared data with children data
                |d: &AppModel|
                match (d.tasks.get(&d.selected_task).map_or(None, |r| Some(r.clone())),
                       d.task_sums.get(&d.selected_task).map_or(TimePrefixSum::new(), |r| r.clone()))
                {
                    (Some(task), time) => Some((task, time)),
                    _ => None,
                },

                |d: &mut AppModel, x: Option<(Task, TimePrefixSum)>| {
                    if let Some((mut new_task, _)) = x {

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


    main_row.add_flex_child(tasks_column
                            .padding(10.0)
                            .border(KeyOrValue::Concrete(APP_BORDER.clone()), 1.0),
                            2.0);

    let mut time_column = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    time_column.add_child(Label::new("Total time log")
                          .with_font(FONT_CAPTION_DESCR.clone())
                          .padding(10.0));

    time_column.add_child(
        Flex::row()
        .with_child(Label::new("Today:\nWeek:\nMonth:\nAll time:"))
        .with_child(
            Label::new(|model: &AppModel, _env: &_| {

                let mut result = String::new();

                let now = Local::now();
                let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));

                let epoch = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc);
                let total_day = get_total_time_from_sums(&model.task_sums, &day_start);

                let total = get_total_time_from_sums(&model.task_sums, &epoch);

                result.push_str(&format_duration(total_day.clone()));
                result.push_str("\n");

                Utc.from_local_datetime(
                    &NaiveDate::from_isoywd(now.year(), now.iso_week().week(), Weekday::Mon)
                        .and_time(NaiveTime::from_hms(0,0,0)))
                    .single()
                    .map(|utc| result.push_str(
                        & format_duration(get_total_time_from_sums(&model.task_sums, &utc))));
                result.push_str("\n");

                Utc.from_local_datetime(
                    &NaiveDate::from_ymd(now.year(), now.month(), 1)
                        .and_time(NaiveTime::from_hms(0, 0, 0)))
                    .single()
                    .map(|utc| result.push_str(
                        & format_duration(get_total_time_from_sums(&model.task_sums, &utc))));
                result.push_str("\n");

                result.push_str(&format_duration(total.clone()));

                result
        }))
        .padding(10.0)
        .lens(lens::Identity.map(
                    |m: &AppModel| m.clone(),
                    |_data: &mut AppModel, _m: AppModel| {},
        )));

    time_column.add_default_spacer();

    time_column.add_flex_child(
        Scroll::new(
            activity_log_widget()
        ), 1.0);

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

