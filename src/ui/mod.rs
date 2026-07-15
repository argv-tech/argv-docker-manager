use std::io;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Focus};

mod controls;
mod layout;
mod logs;
mod overlays;
mod services;
mod status_bar;
mod theme;

pub fn render_ui(frame: &mut Frame, app: &mut App) -> io::Result<()> {
    let show_search = app.focus == Focus::Services && app.search_mode;
    let sections = layout::build(frame.area(), show_search);

    status_bar::render(frame, app, sections.status_bar);
    services::render(frame, app, sections.services_list, sections.search);
    render_separator(frame, sections.separator);
    logs::render(frame, app, sections.logs);
    controls::render(frame, app, sections.help);
    overlays::render(frame, app);

    Ok(())
}

fn render_separator(frame: &mut Frame, area: Option<Rect>) {
    let Some(area) = area else {
        return;
    };
    let line = Line::from(Span::styled("│", Style::new().fg(theme::MUTED)));
    for row in 0..area.height {
        let cell = Rect::new(area.x, area.y + row, 1, 1);
        frame.render_widget(Paragraph::new(line.clone()), cell);
    }
}
