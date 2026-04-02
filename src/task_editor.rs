use std::rc::Rc;

use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};
use im::OrdSet;
use ratatui::style::Color;

use crate::task::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditField {
    Name,
    Priority,
    Status,
    WorkMinutes,
    BreakMinutes,
    Tags,
    Description,
    Color,
}

const FIELD_ORDER: [EditField; 8] = [
    EditField::Name,
    EditField::Priority,
    EditField::Status,
    EditField::WorkMinutes,
    EditField::BreakMinutes,
    EditField::Tags,
    EditField::Description,
    EditField::Color,
];

pub enum EditAction {
    Continue,
    Confirm,
    Cancel,
}

pub struct ColorEntry {
    pub color: Color,
    pub value: u32,
    pub label: &'static str,
}

pub const COLOR_PALETTE: &[ColorEntry] = &[
    ColorEntry { color: Color::Reset,        value: 0,  label: "Default" },
    ColorEntry { color: Color::Red,          value: 1,  label: "Red" },
    ColorEntry { color: Color::Green,        value: 2,  label: "Green" },
    ColorEntry { color: Color::Yellow,       value: 3,  label: "Yellow" },
    ColorEntry { color: Color::Blue,         value: 4,  label: "Blue" },
    ColorEntry { color: Color::Magenta,      value: 5,  label: "Magenta" },
    ColorEntry { color: Color::Cyan,         value: 6,  label: "Cyan" },
    ColorEntry { color: Color::LightRed,     value: 7,  label: "Light Red" },
    ColorEntry { color: Color::LightGreen,   value: 8,  label: "Light Green" },
    ColorEntry { color: Color::LightYellow,  value: 9,  label: "Light Yellow" },
    ColorEntry { color: Color::LightBlue,    value: 10, label: "Light Blue" },
    ColorEntry { color: Color::LightMagenta, value: 11, label: "Light Magenta" },
    ColorEntry { color: Color::LightCyan,    value: 12, label: "Light Cyan" },
];

pub fn color_index_from_u32(val: u32) -> usize {
    COLOR_PALETTE.iter().position(|e| e.value == val).unwrap_or(0)
}

pub fn color_for_task(task: &Task) -> Color {
    let idx = color_index_from_u32(task.color);
    COLOR_PALETTE[idx].color
}

pub struct TaskEditor {
    pub uid: String,
    pub name: String,
    pub description: String,
    pub tags: OrdSet<String>,
    pub priority: CuaPriority,
    pub status: TaskStatus,
    pub work_minutes: i64,
    pub break_minutes: i64,
    pub color_index: usize,

    pub focused_field: EditField,
    pub editing_text: bool,
    pub text_buf: String,
    pub text_buf_backup: String,
    pub cursor_pos: usize,
    pub tag_input: String,
    pub editing_tag: bool,
    pub tag_cursor: usize,
}

impl TaskEditor {
    pub fn from_task(task: &Task) -> TaskEditor {
        TaskEditor {
            uid: task.uid.clone(),
            name: task.name.clone(),
            description: task.description.clone(),
            tags: task.tags.clone(),
            priority: task.priority.into(),
            status: task.task_status.clone(),
            work_minutes: task.work_duration.num_minutes(),
            break_minutes: task.break_duration.num_minutes(),
            color_index: color_index_from_u32(task.color),

            focused_field: EditField::Name,
            editing_text: false,
            text_buf: String::new(),
            text_buf_backup: String::new(),
            cursor_pos: 0,
            tag_input: String::new(),
            editing_tag: false,
            tag_cursor: 0,
        }
    }

    pub fn to_task(&self, original: &Task) -> Task {
        Task {
            uid: self.uid.clone(),
            seq: original.seq + 1,
            name: self.name.clone(),
            description: self.description.clone(),
            tags: self.tags.clone(),
            priority: self.priority.clone().into(),
            task_status: self.status.clone(),
            work_duration: Rc::new(chrono::Duration::minutes(self.work_minutes)),
            break_duration: Rc::new(chrono::Duration::minutes(self.break_minutes)),
            color: COLOR_PALETTE[self.color_index].value,
        }
    }

    fn field_index(&self) -> usize {
        FIELD_ORDER.iter().position(|f| *f == self.focused_field).unwrap_or(0)
    }

    fn next_field(&mut self) {
        let i = self.field_index();
        let next = (i + 1) % FIELD_ORDER.len();
        self.focused_field = FIELD_ORDER[next];
    }

    fn prev_field(&mut self) {
        let i = self.field_index();
        let prev = if i == 0 { FIELD_ORDER.len() - 1 } else { i - 1 };
        self.focused_field = FIELD_ORDER[prev];
    }

    fn start_text_edit(&mut self, initial: &str) {
        self.text_buf_backup = initial.to_string();
        self.text_buf = initial.to_string();
        self.cursor_pos = self.text_buf.len();
        self.editing_text = true;
    }

    fn confirm_text_edit(&mut self) {
        match self.focused_field {
            EditField::Name => self.name = self.text_buf.clone(),
            EditField::Description => self.description = self.text_buf.clone(),
            _ => {}
        }
        self.editing_text = false;
        self.text_buf.clear();
    }

    fn cancel_text_edit(&mut self) {
        self.text_buf = self.text_buf_backup.clone();
        match self.focused_field {
            EditField::Name => self.name = self.text_buf_backup.clone(),
            EditField::Description => self.description = self.text_buf_backup.clone(),
            _ => {}
        }
        self.editing_text = false;
        self.text_buf.clear();
    }

    fn handle_text_input(&mut self, key: KeyCode) -> EditAction {
        match key {
            KeyCode::Enter => {
                self.confirm_text_edit();
            }
            KeyCode::Esc => {
                self.cancel_text_edit();
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.text_buf.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.text_buf.len() {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Char(c) => {
                self.text_buf.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            _ => {}
        }
        EditAction::Continue
    }

    fn handle_tag_input(&mut self, key: KeyCode) -> EditAction {
        match key {
            KeyCode::Enter => {
                let tag = self.tag_input.trim().to_string();
                if !tag.is_empty() {
                    self.tags.insert(tag);
                }
                self.tag_input.clear();
                self.editing_tag = false;
            }
            KeyCode::Esc => {
                self.tag_input.clear();
                self.editing_tag = false;
            }
            KeyCode::Backspace => {
                self.tag_input.pop();
            }
            KeyCode::Char(c) => {
                self.tag_input.push(c);
            }
            _ => {}
        }
        EditAction::Continue
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditAction {
        // Text editing sub-mode
        if self.editing_text {
            return self.handle_text_input(key.code);
        }

        // Tag typing sub-mode
        if self.editing_tag {
            return self.handle_tag_input(key.code);
        }

        // Ctrl-S to save from anywhere
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return EditAction::Confirm;
        }

        match key.code {
            // Navigation
            KeyCode::Esc => return EditAction::Cancel,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.next_field(),
            KeyCode::Char('k') | KeyCode::Up => self.prev_field(),

            // Field-specific
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right |
            KeyCode::Char('h') | KeyCode::Left => {
                let forward = matches!(key.code,
                    KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right);

                match self.focused_field {
                    EditField::Name => {
                        if key.code == KeyCode::Enter {
                            self.start_text_edit(&self.name.clone());
                        }
                    }
                    EditField::Description => {
                        if key.code == KeyCode::Enter {
                            self.start_text_edit(&self.description.clone());
                        }
                    }
                    EditField::Priority => {
                        self.priority = if forward {
                            self.priority.cycle_next()
                        } else {
                            self.priority.cycle_prev()
                        };
                    }
                    EditField::Status => {
                        self.status = if forward {
                            self.status.cycle_next()
                        } else {
                            self.status.cycle_prev()
                        };
                    }
                    EditField::WorkMinutes => {
                        if forward {
                            self.work_minutes = (self.work_minutes + 5).min(480);
                        } else {
                            self.work_minutes = (self.work_minutes - 5).max(5);
                        }
                    }
                    EditField::BreakMinutes => {
                        if forward {
                            self.break_minutes = (self.break_minutes + 5).min(480);
                        } else {
                            self.break_minutes = (self.break_minutes - 5).max(5);
                        }
                    }
                    EditField::Tags => {
                        if key.code == KeyCode::Enter {
                            self.editing_tag = true;
                            self.tag_input.clear();
                        } else {
                            let tag_count = self.tags.len();
                            if tag_count > 0 {
                                if forward {
                                    self.tag_cursor = (self.tag_cursor + 1) % tag_count;
                                } else if self.tag_cursor == 0 {
                                    self.tag_cursor = tag_count - 1;
                                } else {
                                    self.tag_cursor -= 1;
                                }
                            }
                        }
                    }
                    EditField::Color => {
                        if forward {
                            self.color_index = (self.color_index + 1) % COLOR_PALETTE.len();
                        } else if self.color_index == 0 {
                            self.color_index = COLOR_PALETTE.len() - 1;
                        } else {
                            self.color_index -= 1;
                        }
                    }
                }
            }

            // Remove tag with x/d
            KeyCode::Char('x') | KeyCode::Char('d') => {
                if self.focused_field == EditField::Tags && !self.tags.is_empty() {
                    let tag = self.tags.iter().nth(self.tag_cursor).cloned();
                    if let Some(t) = tag {
                        self.tags.remove(&t);
                        if self.tag_cursor >= self.tags.len() && self.tag_cursor > 0 {
                            self.tag_cursor -= 1;
                        }
                    }
                }
            }

            _ => {}
        }

        EditAction::Continue
    }

    pub fn current_text_display(&self) -> (&str, usize) {
        if self.editing_text {
            (&self.text_buf, self.cursor_pos)
        } else {
            match self.focused_field {
                EditField::Name => (&self.name, self.name.len()),
                EditField::Description => (&self.description, self.description.len()),
                _ => ("", 0),
            }
        }
    }
}
