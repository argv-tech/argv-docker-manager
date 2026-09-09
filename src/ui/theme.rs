use ratatui::{
    style::{Color, Style},
    text::Line,
    widgets::{Block, BorderType},
};

use crate::status::Status;

pub const ACCENT: Color = Color::Cyan;
pub const BRAND: Color = Color::Magenta;
pub const ERROR: Color = Color::Red;
pub const FOCUS: Color = Color::Blue;
pub const MUTED: Color = Color::DarkGray;
pub const RUNNING: Color = Color::Green;
pub const TEXT: Color = Color::Gray;
pub const TRANSITION: Color = Color::Yellow;

pub fn panel<'a>(title: Line<'a>, focused: bool) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(panel_border(focused))
        .title(title)
}

pub fn panel_border(focused: bool) -> Style {
    Style::new().fg(if focused { FOCUS } else { MUTED })
}

pub fn status_color(status: &Status) -> Color {
    match status {
        Status::Running => RUNNING,
        Status::Pulling => ACCENT,
        Status::Starting | Status::Stopping => TRANSITION,
        Status::Error | Status::RuntimeUnavailable => ERROR,
        Status::Stopped => TEXT,
    }
}
