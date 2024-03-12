// On Windows platform, don't show a console when opening the app.
#![windows_subsystem = "windows"]

use std::rc::Rc;
use std::time::SystemTime;
use std::path::PathBuf;

use std::io::{self, stdout};

use druid::{TimerToken};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, style::palette::tailwind, widgets::*};

use chrono::prelude::*;

use clap::Parser;

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

const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const ALT_ROW_COLOR: Color = tailwind::SLATE.c900;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const COMPLETED_TEXT_COLOR: Color = tailwind::GREEN.c500;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config_dir: Option<PathBuf>,
}

fn get_db_path(args: &Args) -> PathBuf {
    let mut default_config_dir = dirs::config_dir().unwrap_or(PathBuf::new());
    default_config_dir.push("netupi");
    args.config_dir.clone().unwrap_or(default_config_dir)
}

fn get_last_task(tasks: &TaskMap, records: &TimeRecordMap) -> Option<String>
{
    for r in records.iter().rev() {
        if let Some(t) = tasks.get(&r.1.uid) {
            if t.task_status != TaskStatus::Archived {
                return Some(t.uid.clone());
            }
        }
    }

    return None;
}

// struct StatusItem {
//     status: TaskStatus
// }

struct StatusList {
    state: ListState,
    items: Vec<FocusFilter>,
    last_selected: Option<usize>,
}

struct TaskItem {
    todo: String,
}

struct TaskList {
    state: ListState,
    items: Vec<TaskItem>,
    last_selected: Option<usize>,
}

impl StatusList {
    fn next(&mut self) {
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
    }

    fn previous(&mut self) {
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
    }

    fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        let items = vec![FocusFilter::Status(TaskStatus::NeedsAction),
                         FocusFilter::Status(TaskStatus::InProcess),
                         FocusFilter::Status(TaskStatus::Completed),
                         FocusFilter::All];

        Self{state, items, last_selected: Some(0)}
    }
}

impl TaskList {
    fn next(&mut self) {
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
    }

    fn previous(&mut self) {
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
    }
}

fn filter_to_list_item(filter: &FocusFilter, index: usize) -> ListItem {
    let line = filter.to_string();
    ListItem::new(line).bg(NORMAL_ROW_COLOR)
}

impl TaskItem {
    fn to_list_item(&self, index: usize) -> ListItem {
        let bg_color = match index % 2 {
            0 => NORMAL_ROW_COLOR,
            _ => ALT_ROW_COLOR,
        };
        let line = format!(" ✓ {}", self.todo);

        ListItem::new(line).bg(bg_color)
    }
}

#[derive(PartialEq)]
enum ActiveWidget {
    TaskWidget,
    FocusWidget
}

struct App {
    model: AppModel,
    task_list: TaskList,
    filter_list: StatusList,
    active_widget: ActiveWidget
}

impl App {
    fn new(model: AppModel) -> App {

        let state = ListState::default();

        let items = model.tasks.iter().map(|t|
            {
                TaskItem{todo: t.1.name.clone()}
            }).collect();

        let last_selected = None;

        let mut task_list = TaskList{state, items, last_selected};
        let filter_list = StatusList::new();

        return App{model, task_list, filter_list, active_widget: ActiveWidget::TaskWidget};
    }

    fn keymap_task_list(&mut self, key: event::KeyCode) {
        use KeyCode::*;
        match key {
            Char('j') | Down => self.task_list.next(),
            Char('k') | Up => self.task_list.previous(),
            _ => {}
        }
    }

    fn keymap_filter_list(&mut self, key: event::KeyCode) {
        use KeyCode::*;
        match key {
            Char('j') | Down => self.filter_list.next(),
            Char('k') | Up => self.filter_list.previous(),
            _ => {}
        }
    }

    // TODO: use messages
    // https://ratatui.rs/concepts/application-patterns/the-elm-architecture/

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {

        loop {
            self.draw(&mut terminal)?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use KeyCode::*;
                    match key.code {
                        Char('q') | Esc => return Ok(()),
                        Left => self.active_widget = ActiveWidget::FocusWidget,
                        Right => self.active_widget = ActiveWidget::TaskWidget,
                        _ => match self.active_widget {
                            ActiveWidget::TaskWidget => self.keymap_task_list(key.code),
                            ActiveWidget::FocusWidget => self.keymap_filter_list(key.code),
                        }
                    }
                }
            }
        }
    }

    
    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    fn render_main_widget(&mut self, area: Rect, buf: &mut Buffer) {

        let horizontal = Layout::horizontal([
            Constraint::Length(20),
            Constraint::Min(0),
            Constraint::Length(20),
        ]);

        let [focus_area, tasks_area, stats_area] = horizontal.areas(area);

        self.render_focus(focus_area, buf);
        self.render_task_list(tasks_area, buf);
    }

    fn render_focus(&mut self, area: Rect, buf: &mut Buffer) {

        let outer_block = Block::default()
            .borders(if self.active_widget == ActiveWidget::FocusWidget {Borders::all()} else {Borders::NONE})
            .padding(if self.active_widget != ActiveWidget::FocusWidget {Padding::symmetric(1, 0)} else {Padding::uniform(0)})
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("Focus")
            .title_alignment(Alignment::Center);

        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        outer_block.render(outer_area, buf);

        let items: Vec<ListItem> = self.filter_list.items.iter().map(|x| filter_to_list_item(x, 0)).collect();

        let items = List::new(items)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(items, inner_area, buf, &mut self.filter_list.state);
    }

    fn render_task_list(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(if self.active_widget == ActiveWidget::TaskWidget {Borders::all()} else {Borders::NONE})
            .padding(if self.active_widget != ActiveWidget::TaskWidget {Padding::symmetric(1, 0)} else {Padding::uniform(0)})
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("Task list")
            .title_alignment(Alignment::Center);

        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        // We get the inner area from outer_block. We'll use this area later to render the table.
        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        // We can render the header in outer_area.
        outer_block.render(outer_area, buf);

        // Iterate through all elements in the `items` and stylize them.
        let items: Vec<ListItem> = self
            .task_list
            .items
            .iter()
            .enumerate()
            .map(|(i, todo_item)| todo_item.to_list_item(i))
            .collect();

        // Create a List from all list items and highlight the currently selected one
        let items = List::new(items)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        // We can now render the item list
        // (look careful we are using StatefulWidget's render.)
        // ratatui::widgets::StatefulWidget::render as stateful_render
        StatefulWidget::render(items, inner_area, buf, &mut self.task_list.state);
    }
}

    fn render_title(area: Rect, buf: &mut Buffer) {
        Paragraph::new("WIP: Title")
            .bold()
            .centered()
            .render(area, buf);
    }

    fn render_footer(model: &AppModel, area: Rect, buf: &mut Buffer) {
        let status = get_status_string(model);
        Paragraph::new(status)
            .centered()
            .render(area, buf);
    }



impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Create a space for header, todo list and the footer.
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        // Create two chunks with equal vertical screen space. One for the list and the other for
        // the info block.
        let vertical = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [upper_item_list_area, lower_item_list_area] = vertical.areas(rest_area);

        render_title(header_area, buf);
        self.render_main_widget(upper_item_list_area, buf);
        render_footer(&self.model, footer_area, buf);
    }
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let conn = db::init(get_db_path(&args))?;
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

    let last_task = get_last_task(&tasks, &records);

    let filter = if let Some(ref uid) = last_task {
        FocusFilter::Status(tasks.get(uid).unwrap().task_status.clone())
    } else {
        FocusFilter::All
    };

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

        // todo make selected_task Option
        selected_task: last_task,
        focus_filter: filter,
        tag_filter: None,
        hot_log_entry: None,
        show_task_edit: false,
        show_task_summary: true,
    };


    // TODO should be done in ctor
    data.update_tags();

    let mut app = App::new(data);

    // initialize ratatui context
    // --

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    app.run(terminal)?;

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
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

        _ => format!("STATUS TEXT")
    }

}

