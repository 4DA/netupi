use ratatui::prelude::*;
use ratatui::style::palette::tailwind;

// --- Base colors ---

pub const HEADER_BG: Color = tailwind::BLUE.c950;
pub const ROW_BG: Color = tailwind::SLATE.c950;
pub const ROW_ALT_BG: Color = tailwind::SLATE.c900;

pub const TEXT: Color = tailwind::SLATE.c200;
pub const TEXT_SECONDARY: Color = tailwind::SLATE.c500;
pub const HEADER_STATS: Color = tailwind::SLATE.c300;
pub const TEXT_MUTED: Color = tailwind::SLATE.c600;
pub const TEXT_INACTIVE: Color = tailwind::SLATE.c400;

pub const ACCENT: Color = tailwind::BLUE.c300;
pub const ACCENT_BORDER: Color = tailwind::BLUE.c400;

#[allow(unused)]
pub const COMPLETED: Color = tailwind::GREEN.c500;
pub const TRACKING_ACTIVE: Color = tailwind::ORANGE.c400;
pub const TRACKING_PAUSED_BG: Color = tailwind::SLATE.c700;
pub const PROGRESS_BAR_WORK: Color = tailwind::ORANGE.c900;
pub const PROGRESS_BAR_BREAK: Color = tailwind::CYAN.c900;
pub const BORDER_ACTIVE: Color = tailwind::BLUE.c400;
pub const BORDER_INACTIVE: Color = tailwind::SLATE.c700;
pub const TITLE_ACTIVE: Color = tailwind::BLUE.c300;
pub const TITLE_INACTIVE: Color = tailwind::SLATE.c300;

// --- Composite styles ---

pub fn highlight_active() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn highlight_inactive() -> Style {
    Style::default().fg(TEXT_INACTIVE)
}

pub fn killed_record() -> Style {
    Style::default()
        .fg(TEXT_MUTED)
        .add_modifier(Modifier::CROSSED_OUT)
}

pub fn cursor_active() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

pub fn cursor_inactive() -> Style {
    Style::default().fg(TEXT_INACTIVE)
}

pub fn editor_focused() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn editor_unfocused() -> Style {
    Style::default().fg(TEXT)
}

pub fn editor_border() -> Style {
    Style::default().fg(ACCENT_BORDER)
}

pub fn help_key() -> Style {
    Style::default()
        .fg(TEXT)
        .add_modifier(Modifier::BOLD)
}

pub fn help_desc() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

pub fn help_editor() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}
