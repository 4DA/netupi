use std::rc::Rc;
use std::time::SystemTime;
use std::path::PathBuf;

use std::io::{self, stdout};

use crossterm::{
    event::{self, poll, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{prelude::*, widgets::*};

use chrono::prelude::*;

use clap::Parser;

use netupi::task::*;
use netupi::task_editor::*;
use netupi::theme;
use netupi::db;
use netupi::app_model::*;
use netupi::task_list::*;
use netupi::task_details::*;
use netupi::activity_log::*;
use netupi::time;
use netupi::widgets;

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

struct StatusList {
    state: ListState,
    items: Vec<FocusFilter>,
}

impl StatusList {
    fn update(&mut self, filter: &FocusFilter) {
        self.state.select(Some(filter.to_int() as usize));
    }

    fn new(filter: &FocusFilter) -> Self {
        let mut state = ListState::default();
        state.select(Some(filter.to_int() as usize));

        let items = vec![FocusFilter::Status(TaskStatus::NeedsAction),
                         FocusFilter::Status(TaskStatus::Completed),
                         FocusFilter::Status(TaskStatus::InProcess),
                         FocusFilter::Status(TaskStatus::Archived),
                         FocusFilter::All];

        Self{state, items}
    }
}

fn filter_to_list_item(filter: &FocusFilter) -> ListItem {
    let line = filter.to_string();
    ListItem::new(line).bg(theme::ROW_BG)
}

#[derive(PartialEq)]
enum ActiveWidget {
    TaskWidget,
    FocusWidget,
    TagWidget,
    ActivityLogWidget,
}

enum AppMode {
    Browse,
    Editing { editor: TaskEditor, original_uid: String },
    Renaming { uid: String, buf: String, cursor: usize },
}

struct App {
    model: AppModel,
    task_list: TaskList,
    filter_list: StatusList,
    active_widget: ActiveWidget,
    mode: AppMode,
    log_cursor: usize,
    tag_cursor: usize,
}

impl App {
    fn new(model: AppModel) -> App {

        let mut state = ListState::default();

        let tasks = model.get_tasks_filtered();

        let items = tasks
            .iter()
            .enumerate()
            .map(|(i, t)| {
                model.selected_task.as_ref().map(|val| if val.eq(&t.uid) {state.select(Some(i))});
                TaskItem{uid: t.uid.clone(), name: t.name.clone()}})
            .collect();

        let last_selected = None;

        let task_list = TaskList{state, items, last_selected};
        let filter_list = StatusList::new(&model.focus_filter);

        return App{model, task_list, filter_list, active_widget: ActiveWidget::TaskWidget, mode: AppMode::Browse, log_cursor: 0, tag_cursor: 0};
    }

    fn keymap_filter_list(&mut self, key: event::KeyCode) {
        use KeyCode::*;
        match key {
            Char('j') | Down => {
                self.model.focus_filter = self.model.focus_filter.cycle_next();
                self.filter_list.update(&self.model.focus_filter);
                self.task_list.update(&self.model);
            }

            Char('k') | Up => {
                self.model.focus_filter = self.model.focus_filter.cycle_prev();
                self.filter_list.update(&self.model.focus_filter);
                self.task_list.update(&self.model);
            }
            _ => {}
        }
    }

    fn get_log_records(&self) -> Vec<DateTime<Utc>> {
        self.model.records.keys().rev().cloned().collect()
    }

    fn get_tag_list(&self) -> Vec<String> {
        self.model.tags.iter().cloned().collect()
    }

    fn keymap_tag_list(&mut self, key: event::KeyCode) {
        use KeyCode::*;
        let tags = self.get_tag_list();
        // +1 for the "All" entry at the top
        let count = tags.len() + 1;

        match key {
            Char('j') | Down => {
                self.tag_cursor = (self.tag_cursor + 1).min(count - 1);
            }
            Char('k') | Up => {
                if self.tag_cursor > 0 {
                    self.tag_cursor -= 1;
                }
            }
            Enter => {
                if self.tag_cursor == 0 {
                    self.model.tag_filter = None;
                } else if let Some(tag) = tags.get(self.tag_cursor - 1) {
                    self.model.tag_filter = Some(tag.clone());
                }
                self.task_list.update(&self.model);
            }
            _ => {}
        }
    }

    fn keymap_activity_log(&mut self, key: event::KeyCode) {
        use KeyCode::*;
        let record_keys = self.get_log_records();
        let count = record_keys.len();
        if count == 0 { return; }

        match key {
            Char('j') | Down => {
                self.log_cursor = (self.log_cursor + 1).min(count - 1);
            }
            Char('k') | Up => {
                if self.log_cursor > 0 {
                    self.log_cursor -= 1;
                }
            }
            Char('x') => {
                if let Some(key) = record_keys.get(self.log_cursor) {
                    self.model.toggle_kill_record(key);
                }
            }
            _ => {}
        }
    }

    fn start_renaming(&mut self) {
        if let Some(ref uid) = self.model.selected_task {
            if let Some(task) = self.model.tasks.get(uid) {
                let name = task.name.clone();
                let cursor = name.len();
                self.mode = AppMode::Renaming { uid: uid.clone(), buf: name, cursor };
            }
        }
    }

    fn finish_renaming(&mut self) {
        if let AppMode::Renaming { ref uid, ref buf, .. } = self.mode {
            let new_name = buf.trim().to_string();
            if !new_name.is_empty() {
                if let Some(task) = self.model.tasks.get_mut(uid) {
                    task.name = new_name;
                    if let Err(what) = db::update_task(self.model.db.clone(), task) {
                        eprintln!("db error: {}", what);
                    }
                }
                self.task_list.update(&self.model);
            }
        }
        self.mode = AppMode::Browse;
    }

    fn start_new_task(&mut self) {
        let task = Task::new_simple(String::new());
        let uid = task.uid.clone();

        if let Err(what) = db::add_task(self.model.db.clone(), &task) {
            eprintln!("db error: {}", what);
        }

        self.model.focus_filter = FocusFilter::Status(task.task_status.clone());
        self.model.selected_task = Some(uid.clone());
        self.model.task_sums.insert(uid.clone(), TimePrefixSum::new());
        let editor = TaskEditor::new_task(&task);
        self.model.tasks.insert(uid.clone(), task);
        self.model.update_tags();
        self.task_list.update(&self.model);
        self.filter_list.update(&self.model.focus_filter);
        self.mode = AppMode::Editing { editor, original_uid: uid };
    }

    fn start_editing(&mut self) {
        if let Some(ref uid) = self.model.selected_task {
            if let Some(task) = self.model.tasks.get(uid) {
                let editor = TaskEditor::from_task(task);
                self.mode = AppMode::Editing { editor, original_uid: uid.clone() };
            }
        }
    }

    fn finish_editing(&mut self) {
        if let AppMode::Editing { ref editor, ref original_uid } = self.mode {
            if let Some(original) = self.model.tasks.get(original_uid) {
                let updated = editor.to_task(&original);
                if let Err(what) = db::update_task(self.model.db.clone(), &updated) {
                    eprintln!("db error: {}", what);
                }
                self.model.tasks = self.model.tasks.update(original_uid.clone(), updated);
                self.model.update_tags();
                self.task_list.update(&self.model);
                self.filter_list.update(&self.model.focus_filter);
            }
        }
        self.mode = AppMode::Browse;
    }

    fn cancel_editing(&mut self) {
        // if cancelling a new task with empty name, delete it
        if let AppMode::Editing { ref editor, ref original_uid } = self.mode {
            if editor.name.is_empty() {
                let _ = db::delete_task(self.model.db.clone(), original_uid);
                self.model.tasks.remove(original_uid);
                self.model.task_sums.remove(original_uid);
                self.model.check_update_selected();
                self.model.update_tags();
                self.task_list.update(&self.model);
            }
        }
        self.mode = AppMode::Browse;
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {

        loop {
            handle_timer_event(&mut self.model);

            self.draw(&mut terminal)?;

            if poll(std::time::Duration::from_millis(500))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match &mut self.mode {
                            AppMode::Editing { ref mut editor, .. } => {
                                match editor.handle_key(key) {
                                    EditAction::Confirm => self.finish_editing(),
                                    EditAction::Cancel => self.cancel_editing(),
                                    EditAction::Continue => {}
                                }
                            }
                            AppMode::Renaming { ref mut buf, ref mut cursor, .. } => {
                                use KeyCode::*;
                                match key.code {
                                    Enter => self.finish_renaming(),
                                    Esc => { self.mode = AppMode::Browse; }
                                    Backspace => {
                                        if *cursor > 0 {
                                            buf.remove(*cursor - 1);
                                            *cursor -= 1;
                                        }
                                    }
                                    Left => { if *cursor > 0 { *cursor -= 1; } }
                                    Right => { if *cursor < buf.len() { *cursor += 1; } }
                                    Char(c) => {
                                        buf.insert(*cursor, c);
                                        *cursor += 1;
                                    }
                                    _ => {}
                                }
                            }
                            AppMode::Browse => {
                                use KeyCode::*;
                                match key.code {
                                    Char('q') => {
                                        stop_tracking(&mut self.model, TrackingState::Inactive);
                                        return Ok(());
                                    }
                                    Char('n') => {
                                        self.start_new_task();
                                        self.active_widget = ActiveWidget::TaskWidget;
                                    }
                                    Char('r') if self.active_widget == ActiveWidget::TaskWidget => {
                                        self.start_renaming();
                                    }
                                    Char('e') if self.active_widget == ActiveWidget::TaskWidget => {
                                        self.start_editing();
                                    }
                                    Tab => {
                                        self.active_widget = match self.active_widget {
                                            ActiveWidget::FocusWidget => ActiveWidget::TagWidget,
                                            ActiveWidget::TagWidget => ActiveWidget::TaskWidget,
                                            ActiveWidget::TaskWidget => ActiveWidget::ActivityLogWidget,
                                            ActiveWidget::ActivityLogWidget => ActiveWidget::FocusWidget,
                                        };
                                    }
                                    BackTab => {
                                        self.active_widget = match self.active_widget {
                                            ActiveWidget::FocusWidget => ActiveWidget::ActivityLogWidget,
                                            ActiveWidget::TagWidget => ActiveWidget::FocusWidget,
                                            ActiveWidget::TaskWidget => ActiveWidget::TagWidget,
                                            ActiveWidget::ActivityLogWidget => ActiveWidget::TaskWidget,
                                        };
                                    }
                                    _ => match self.active_widget {
                                        ActiveWidget::TaskWidget => {
                                            self.task_list.keymap_task_list(&mut self.model, key.code);
                                            self.filter_list.update(&self.model.focus_filter);
                                        },
                                        ActiveWidget::FocusWidget => self.keymap_filter_list(key.code),
                                        ActiveWidget::TagWidget => self.keymap_tag_list(key.code),
                                        ActiveWidget::ActivityLogWidget => self.keymap_activity_log(key.code),
                                    }
                                }
                            }
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
            Constraint::Min(30),
            Constraint::Length(45),
        ]);

        let vertical = Layout::vertical([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]);

        let right_vertical = Layout::vertical([
            Constraint::Length(8),
            Constraint::Min(20),
        ]);

        let left_vertical = Layout::vertical([
            Constraint::Length(7), // 5 focus items + border
            Constraint::Min(5),
        ]);

        let [focus_area, center_area, right_area] = horizontal.areas(area);
        let [filter_area, tag_area] = left_vertical.areas(focus_area);
        let [task_list_area, task_stats_area] = vertical.areas(center_area);
        let [total_time_log_area, activity_log_area] = right_vertical.areas(right_area);

        self.render_focus(filter_area, buf);
        self.render_tags(tag_area, buf);

        self.render_task_list(task_list_area, buf);

        if self.model.selected_task.is_some() {
            self.render_task_stats(task_stats_area, buf);
        }

        self.render_total_time_log(total_time_log_area, buf);
        self.render_activity_log(activity_log_area, buf);
    }

    fn render_focus(&mut self, area: Rect, buf: &mut Buffer) {

        let is_active = self.active_widget == ActiveWidget::FocusWidget;
        let outer_block = pane_block("Focus", is_active);

        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(theme::TEXT)
            .bg(theme::ROW_BG);

        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let items: Vec<ListItem> = self.filter_list.items.iter().map(|x| filter_to_list_item(x)).collect();

        let items = List::new(items)
            .block(inner_block)
            .highlight_style(if is_active { theme::highlight_active() } else { theme::highlight_inactive() })
            .highlight_symbol(if is_active { ">" } else { " " })
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(items, inner_area, buf, &mut self.filter_list.state);
    }

    fn render_tags(&mut self, area: Rect, buf: &mut Buffer) {
        let is_active = self.active_widget == ActiveWidget::TagWidget;
        let outer_block = pane_block("Tags", is_active);

        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(theme::TEXT)
            .bg(theme::ROW_BG);

        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let tags = self.get_tag_list();
        let active_style = Style::default().fg(theme::TRACKING_ACTIVE).add_modifier(Modifier::BOLD);
        let mut items: Vec<ListItem> = Vec::with_capacity(tags.len() + 1);
        let all_style = if self.model.tag_filter.is_none() { active_style } else { Style::default() };
        items.push(ListItem::new(Span::styled("All", all_style)).bg(theme::ROW_BG));
        for tag in &tags {
            let style = if self.model.tag_filter.as_ref() == Some(tag) { active_style } else { Style::default() };
            items.push(ListItem::new(Span::styled(tag.as_str(), style)).bg(theme::ROW_BG));
        }

        let mut state = ListState::default();
        state.select(Some(self.tag_cursor));

        let list = List::new(items)
            .block(inner_block)
            .highlight_style(if is_active { theme::highlight_active() } else { theme::highlight_inactive() })
            .highlight_symbol(if is_active { ">" } else { " " })
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, inner_area, buf, &mut state);
    }

    fn render_task_list(&mut self, area: Rect, buf: &mut Buffer) {
        let is_active = self.active_widget == ActiveWidget::TaskWidget;
        let outer_block = pane_block("Task list", is_active);

        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(theme::TEXT)
            .bg(theme::ROW_BG);

        let inner_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let tasks = self.model.get_tasks_filtered();

        let items: Vec<ListItem> = tasks
            .iter()
            .map(|t| {
                let bg_color = theme::ROW_BG;

                let priority_indicator = match t.priority.into() {
                    CuaPriority::High => "! ",
                    CuaPriority::Low => "v ",
                    _ => "  ",
                };

                let is_active = matches!(&self.model.tracking.state,
                    TrackingState::Active(uid) if uid == &t.uid);
                let is_paused = matches!(&self.model.tracking.state,
                    TrackingState::Paused(uid) if uid == &t.uid);

                let tracking_indicator = match &self.model.tracking.state {
                    TrackingState::Paused(uid) if uid == &t.uid => "| ",
                    TrackingState::Break(uid) if uid == &t.uid => "~ ",
                    _ => "  ",
                };

                let row_fg = if is_active { theme::TRACKING_ACTIVE } else { theme::TEXT };
                let row_bg = if is_paused { theme::TRACKING_PAUSED_BG } else { bg_color };

                let task_color = color_for_task(t);
                let color_block = if task_color == Color::Reset {
                    Span::styled(" ", Style::default())
                } else {
                    Span::styled(" ", Style::default().bg(task_color))
                };
                let is_renaming = matches!(&self.mode,
                    AppMode::Renaming { uid, .. } if uid == &t.uid);

                let mut text_style = Style::default().fg(row_fg);
                if is_active {
                    text_style = text_style.add_modifier(Modifier::BOLD);
                }

                let display_name = if is_renaming {
                    if let AppMode::Renaming { ref buf, cursor, .. } = self.mode {
                        let mut s = buf.clone();
                        s.insert(cursor, '|');
                        s
                    } else { t.name.clone() }
                } else {
                    t.name.clone()
                };

                let text = Span::styled(
                    format!("{}{}{}", tracking_indicator, priority_indicator, display_name),
                    text_style,
                );

                ListItem::new(Line::from(vec![color_block, text])).bg(row_bg)
            })
            .collect();

        let items = List::new(items)
            .block(inner_block)
            .highlight_style(if is_active { theme::highlight_active() } else { theme::highlight_inactive() })
            .highlight_symbol(if is_active { "> " } else { "  " })
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(items, inner_area, buf, &mut self.task_list.state);
    }

    fn render_task_stats(&mut self, area: Rect, buf: &mut Buffer) {
        let selected_uid = self.model.selected_task.clone().unwrap();
        let task = self.model.tasks.get(&selected_uid).unwrap();
        let task_sum = self.model.task_sums.get(&selected_uid).unwrap();

        let has_desc = !task.description.trim().is_empty();
        let desc_lines = if has_desc {
            task.description.lines().count().min(3) as u16 + 2 // +2 for borders
        } else {
            0
        };

        let chunks = Layout::vertical([
            Constraint::Length(if has_desc { desc_lines } else { 0 }),
            Constraint::Length(7),
            Constraint::Min(5),
        ]).split(area);
        let (desc_area, agg_area, retro_area) = (chunks[0], chunks[1], chunks[2]);

        if has_desc {
            let desc_block = pane_block("Description", false);
            let desc_inner = desc_block.inner(desc_area);
            desc_block.render(desc_area, buf);

            Paragraph::new(task.description.as_str())
                .block(Block::default().bg(theme::ROW_BG).padding(Padding::horizontal(1)))
                .fg(theme::TEXT_SECONDARY)
                .wrap(Wrap { trim: false })
                .render(desc_inner, buf);
        }

        // --- aggregate stats ---
        let agg_block = pane_block("Task stats", false);
        let agg_inner = agg_block.inner(agg_area);
        agg_block.render(agg_area, buf);

        let agg = time::get_duration(&task_sum, &Local::now());
        let durations = widgets::get_task_durations(&agg);
        let captions: String = "Today\nWeek\nMonth\nYear\nAll time".into();

        let horizontal = Layout::horizontal([
            Constraint::Length(12),
            Constraint::Min(10),
        ]);
        let [cap_area, dur_area] = horizontal.areas(agg_inner);

        Paragraph::new(captions)
            .block(Block::default().bg(theme::ROW_BG).padding(Padding::horizontal(1)))
            .fg(theme::TEXT)
            .render(cap_area, buf);

        Paragraph::new(durations)
            .block(Block::default().bg(theme::ROW_BG).padding(Padding::horizontal(1)))
            .fg(theme::TEXT)
            .render(dur_area, buf);

        // --- daily retrospective ---
        let retro_block = pane_block("Daily", false);
        let retro_inner = retro_block.inner(retro_area);
        retro_block.render(retro_area, buf);

        let mut retro = String::new();
        for i in 0..28 {
            retro.push_str(&get_day_time2(task_sum, i));
            retro.push('\n');
        }

        Paragraph::new(retro)
            .block(Block::default().bg(theme::ROW_BG).padding(Padding::horizontal(1)))
            .fg(theme::TEXT)
            .render(retro_inner, buf);
    }

    fn render_total_time_log(&mut self, area: Rect, buf: &mut Buffer) {

        let outer_block = pane_block("Total time log", false);

        let left_block = Block::default()
            .borders(Borders::NONE)
            .bg(theme::ROW_BG)
            .padding(Padding::horizontal(1));

        let right_block = Block::default()
            .borders(Borders::NONE)
            .bg(theme::ROW_BG)
            .padding(Padding::horizontal(1));

        let inner_info_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let captions:String = "Today\nWeek\nMonth\nYear\nAll time".into();
        let agg = time::get_durations(&self.model.task_sums);
        let durations = widgets::get_task_durations(&agg);

        let horizontal = Layout::horizontal([
            Constraint::Length(15),
            Constraint::Min(20),
        ]);

        let [left_area, right_area] = horizontal.areas(inner_info_area);

        let captions_paragraph = Paragraph::new(captions)
            .block(left_block)
            .fg(theme::TEXT)
            .wrap(Wrap { trim: false });

        let durations_paragraph = Paragraph::new(durations)
            .block(right_block)
            .fg(theme::TEXT)
            .wrap(Wrap { trim: false });

        captions_paragraph.render(left_area, buf);
        durations_paragraph.render(right_area, buf);
    }

    fn render_activity_log(&mut self, area: Rect, buf: &mut Buffer) {
        let is_active = self.active_widget == ActiveWidget::ActivityLogWidget;

        let outer_block = pane_block("Activity log", is_active);

        let inner_info_block = Block::default()
            .borders(Borders::NONE)
            .bg(theme::ROW_BG)
            .padding(Padding::horizontal(1));

        let inner_info_area = outer_block.inner(area);
        outer_block.render(area, buf);

        let mut lines: Vec<Line> = Vec::new();

        for (i, rec) in self.model.records.iter().rev().enumerate() {
            if let Some(task) = self.model.tasks.get(&rec.1.uid) {
                let text = format_time_record(task, &rec.1);
                let is_killed = self.model.records_killed.contains(rec.0);
                let is_cursor = i == self.log_cursor;

                let mut style = Style::default().fg(theme::TEXT);
                if is_killed {
                    style = theme::killed_record();
                }
                if is_cursor && is_active {
                    style = style.patch(theme::cursor_active());
                } else if is_cursor {
                    style = style.patch(theme::cursor_inactive());
                }

                lines.push(Line::from(Span::styled(text, style)));
            }
        }

        let log_paragraph = Paragraph::new(lines)
            .block(inner_info_block)
            .scroll((self.log_cursor.saturating_sub(
                inner_info_area.height.saturating_sub(2) as usize) as u16, 0));

        log_paragraph.render(inner_info_area, buf);
    }
}

fn pane_block(title: &str, is_active: bool) -> Block {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if is_active { theme::BORDER_ACTIVE } else { theme::BORDER_INACTIVE }))
        .fg(theme::TEXT)
        .bg(theme::HEADER_BG)
        .title(title.to_string())
        .title_style(Style::default().fg(if is_active { theme::TITLE_ACTIVE } else { theme::TITLE_INACTIVE }))
        .title_alignment(Alignment::Center)
}

fn render_editor(editor: &TaskEditor, area: Rect, buf: &mut Buffer) {
    // centered popup
    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 18u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup = Rect::new(x, y, popup_width, popup_height);

    // clear background
    Clear.render(popup, buf);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::editor_border())
        .title(" Edit Task ")
        .title_alignment(Alignment::Center)
        .bg(theme::ROW_BG);

    let inner = block.inner(popup);
    block.render(popup, buf);

    let label_width = 14u16;
    let fields: Vec<(EditField, &str, String)> = vec![
        (EditField::Name, "Name", editor.name.clone()),
        (EditField::Priority, "Priority", editor.priority.label().to_string()),
        (EditField::Status, "Status", editor.status.to_string().to_string()),
        (EditField::WorkMinutes, "Work (min)", format!("{}", editor.work_minutes)),
        (EditField::BreakMinutes, "Break (min)", format!("{}", editor.break_minutes)),
        (EditField::Tags, "Tags", {
            let is_renaming = editor.editing_tag && editor.renaming_tag.is_some();
            let tags: Vec<String> = editor.tags.iter().enumerate().map(|(i, t)| {
                if is_renaming && i == editor.tag_cursor {
                    format!("[{}|]", editor.tag_input)
                } else if editor.focused_field == EditField::Tags && i == editor.tag_cursor {
                    format!("[{}]", t)
                } else {
                    t.clone()
                }
            }).collect();
            let s = tags.join(", ");
            if editor.editing_tag && !is_renaming {
                format!("{} + {}_", s, editor.tag_input)
            } else {
                s
            }
        }),
        (EditField::Description, "Description", editor.description.clone()),
        (EditField::Color, "Color", COLOR_PALETTE[editor.color_index].label.to_string()),
    ];

    for (i, (field, label, value)) in fields.iter().enumerate() {
        let row_y = inner.y + i as u16;
        if row_y >= inner.y + inner.height.saturating_sub(1) {
            break;
        }
        let row = Rect::new(inner.x, row_y, inner.width, 1);

        let is_focused = editor.focused_field == *field;
        let is_text_editing = is_focused && editor.editing_text;

        let display_value = if is_text_editing {
            let (text, cursor) = editor.current_text_display();
            let mut s = text.to_string();
            // show cursor position with a block char
            if cursor <= s.len() {
                s.insert(cursor, '|');
            }
            s
        } else {
            value.clone()
        };

        let style = if is_focused {
            theme::editor_focused()
        } else {
            theme::editor_unfocused()
        };

        let color_preview = if *field == EditField::Color {
            Style::default().fg(COLOR_PALETTE[editor.color_index].color)
        } else {
            style
        };

        // render label
        let label_area = Rect::new(row.x + 1, row.y, label_width, 1);
        let value_area = Rect::new(row.x + 1 + label_width, row.y, row.width.saturating_sub(label_width + 2), 1);

        Paragraph::new(format!("{}:", label))
            .style(style)
            .render(label_area, buf);

        Paragraph::new(display_value)
            .style(color_preview)
            .render(value_area, buf);
    }

    // help line at bottom of popup
    let help_y = inner.y + inner.height.saturating_sub(1);
    if help_y > inner.y {
        let help_area = Rect::new(inner.x + 1, help_y, inner.width.saturating_sub(2), 1);
        let help = if editor.editing_text {
            "Enter:confirm  Esc:cancel"
        } else if editor.editing_tag && editor.renaming_tag.is_some() {
            "Enter:confirm rename  Esc:cancel"
        } else if editor.editing_tag {
            "Enter:add tag  Esc:cancel"
        } else if editor.focused_field == EditField::Tags {
            "Enter:add  r:rename  x:delete  h/l:select  Ctrl-S:save  Esc:cancel"
        } else {
            "j/k:nav  h/l:adjust  Enter:edit text  Ctrl-S:save  Esc:cancel"
        };
        Paragraph::new(help)
            .style(theme::help_editor())
            .render(help_area, buf);
    }
}

fn render_title(model: &AppModel, area: Rect, buf: &mut Buffer) {
    let now = Local::now();
    let day_start: DateTime<Utc> = DateTime::from(now.date().and_hms(0, 0, 0));
    let utc_now = Utc::now();

    let agg = time::get_durations(&model.task_sums);
    let today_time = time::format_duration(&agg.day);

    let tasks_today: usize = model.task_sums.iter()
        .filter(|(_, sum)| get_total_time(sum, &day_start, &utc_now) > chrono::Duration::zero())
        .count();

    let date_time = now.format("%a, %d %b %H:%M").to_string();

    let left = Span::styled(" netupi", Style::default().fg(theme::TEXT).add_modifier(Modifier::BOLD));
    let right = Span::styled(
        format!("Today: {} ({} tasks)     {} ", today_time, tasks_today, date_time),
        Style::default().fg(theme::HEADER_STATS),
    );

    let spacer_width = area.width.saturating_sub(left.width() as u16 + right.width() as u16);
    let spacer = Span::raw(" ".repeat(spacer_width as usize));

    Paragraph::new(Line::from(vec![left, spacer, right]))
        .render(area, buf);
}

fn help_keys(active_widget: &ActiveWidget, model: &AppModel, mode: &AppMode) -> Vec<(&'static str, &'static str)> {
    match mode {
        AppMode::Renaming { .. } => return vec![
            ("Enter", "confirm"), ("Esc", "cancel"),
        ],
        AppMode::Editing { .. } => return vec![
            ("Ctrl-S", "save"), ("Esc", "cancel"), ("j/k", "nav"), ("Enter/h/l", "edit"),
        ],
        AppMode::Browse => {}
    }
    match active_widget {
        ActiveWidget::ActivityLogWidget => vec![
            ("j/k", "nav"), ("x", "kill/unkill"), ("Tab", "switch"), ("q", "quit"),
        ],
        _ if model.focus_filter == FocusFilter::Status(TaskStatus::Archived) => vec![
            ("n", "new"), ("e", "edit"), ("r", "rename"), ("d", "delete"), ("Tab", "switch"), ("q", "quit"),
        ],
        _ => vec![
            ("space", "start/pause"), ("Esc", "stop"),
            ("n", "new"), ("e", "edit"), ("r", "rename"), ("c", "complete"), ("a", "archive"), ("Tab", "switch"), ("q", "quit"),
        ],
    }
}

fn render_footer(model: &AppModel, active_widget: &ActiveWidget, mode: &AppMode, area: Rect, buf: &mut Buffer) {
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
    ]);
    let [status_area, help_area] = vertical.areas(area);

    // status line with progress bar background
    let (status, progress, is_break) = get_status_info(model);

    // first render centered text
    Paragraph::new(status.clone())
        .centered()
        .fg(theme::TEXT)
        .render(status_area, buf);

    // then paint background per-cell based on progress
    if progress > 0.0 {
        let bar_color = if is_break { theme::PROGRESS_BAR_BREAK } else { theme::PROGRESS_BAR_WORK };
        let filled_width = (status_area.width as f64 * progress) as u16;
        for x in status_area.x..status_area.x + filled_width {
            let cell = buf.get_mut(x, status_area.y);
            cell.set_bg(bar_color);
        }
    }

    // help line with bold keys
    let keys = help_keys(active_widget, model, mode);
    let mut spans: Vec<Span> = Vec::new();
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(format!(" {}", key), theme::help_key()));
        spans.push(Span::styled(format!(" {} │", desc), theme::help_desc()));
    }

    Paragraph::new(Line::from(spans))
        .centered()
        .render(help_area, buf);
}



impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2), // status + help
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        render_title(&self.model, header_area, buf);
        self.render_main_widget(rest_area, buf);
        render_footer(&self.model, &self.active_widget, &self.mode, footer_area, buf);

        if let AppMode::Editing { ref editor, .. } = self.mode {
            render_editor(editor, area, buf);
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let conn = db::init(get_db_path(&args))?;
    let db = Rc::new(conn);

    let (tasks, tags) = db::get_tasks(db.clone())?;
    let (records, records_killed) = db::get_time_records(db.clone(),
        &DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
        &DateTime::from(SystemTime::now()))?;

    let mut task_sums = TaskSums::new();

    for (uid, _) in &tasks {
        let sum = build_time_prefix_sum(&tasks, &records, uid.clone(), &records_killed);
        task_sums.insert(uid.clone(), sum);
    }

    let last_task = get_last_task(&tasks, &records);

    let filter = if let Some(ref uid) = last_task {
        FocusFilter::Status(tasks.get(uid).unwrap().task_status.clone())
    } else {
        FocusFilter::All
    };

    let data = AppModel{
        db,
        tasks,
        records,
        records_killed,
        task_sums,
        tags,
        tracking: TrackingCtx{state: TrackingState::Inactive,
                              timestamp: Rc::new(Utc::now()),
                              timer: None,
                              elapsed: Rc::new(chrono::Duration::zero())},

        selected_task: last_task,
        focus_filter: filter,
        tag_filter: None,
    };

    let mut app = App::new(data);

    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let result = app.run(terminal);

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result?;

    Ok(())
}

/// Returns (status_text, progress_fraction 0.0..1.0, is_break)
fn get_status_info(d: &AppModel) -> (String, f64, bool) {
    match d.tracking.state {
        TrackingState::Active(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");

            let duration = d.tracking.elapsed.checked_add(&Utc::now()
                .signed_duration_since(d.tracking.timestamp.as_ref().clone()))
                .unwrap_or(chrono::Duration::zero());

            let total = get_work_interval(d, uid);
            let progress = if total.num_milliseconds() > 0 {
                (duration.num_milliseconds() as f64 / total.num_milliseconds() as f64).min(1.0)
            } else { 0.0 };

            (format!("Active: '{}' | Elapsed: {} / {}",
                    active_task.name, time::format_duration(&duration), time::format_duration(&total)),
             progress, false)
        },
        TrackingState::Break(ref uid) => {
            let rest_task = &d.tasks.get(uid).expect("unknown uid");

            let duration =
                Utc::now().signed_duration_since(d.tracking.timestamp.as_ref().clone());

            let total = get_rest_interval(d, uid);
            let progress = if total.num_milliseconds() > 0 {
                (duration.num_milliseconds() as f64 / total.num_milliseconds() as f64).min(1.0)
            } else { 0.0 };

            (format!("Break: '{}' | Elapsed: {} / {}",
                    rest_task.name, time::format_duration(&duration), time::format_duration(&total)),
             progress, true)
        },
        TrackingState::Paused(ref uid) => {
            let active_task = &d.tasks.get(uid).expect("unknown uid");
            let total = get_work_interval(d, uid);
            let progress = if total.num_milliseconds() > 0 {
                (d.tracking.elapsed.num_milliseconds() as f64 / total.num_milliseconds() as f64).min(1.0)
            } else { 0.0 };

            (format!("Paused: '{}' | Elapsed: {} / {}",
                    active_task.name,
                    time::format_duration(&d.tracking.elapsed),
                    time::format_duration(&total)),
             progress, false)
        },

        _ => (String::new(), 0.0, false)
    }
}
