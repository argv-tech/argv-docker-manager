use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use crate::app::{App, Focus};
use crate::service::Service;
use crate::status::Status;

use super::theme;

pub fn render(frame: &mut Frame, app: &mut App, list_area: Rect, search_area: Option<Rect>) {
    if let Some(search_area) = search_area {
        render_search(frame, app, search_area);
    }

    let filtered_services: Vec<&Service> =
        if app.focus == Focus::Services && app.search_mode && !app.search_query.is_empty() {
            let query = app.search_query.to_lowercase();
            app.services
                .iter()
                .filter(|service| service.name.to_lowercase().contains(&query))
                .collect()
        } else {
            app.services.iter().collect()
        };

    let content_width = list_area.width.saturating_sub(4) as usize;
    let mut items: Vec<ListItem> = filtered_services
        .iter()
        .map(|service| service_item(service, app, content_width))
        .collect();

    if items.is_empty() {
        let message = if app.search_mode {
            "No matching services  ·  Esc to clear"
        } else {
            "No services found in ./containers/"
        };
        items.push(ListItem::new(Line::from(Span::styled(
            format!("  {message}"),
            Style::new().fg(theme::MUTED),
        ))));
    }

    let running_count = app
        .services
        .iter()
        .filter(|service| service.status() == Status::Running)
        .count();
    let title = services_title(
        running_count,
        app.services.len(),
        app.auto_restart.len(),
        filtered_services.len(),
        app.search_mode && !app.search_query.is_empty(),
    );

    let list = List::new(items)
        .block(theme::panel(title, app.focus == Focus::Services))
        .style(Style::new().fg(theme::TEXT))
        .highlight_style(
            Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌ ");

    if app.search_mode {
        let mut search_state = ratatui::widgets::ListState::default();
        if !filtered_services.is_empty() {
            search_state.select(Some(0));
        }
        frame.render_stateful_widget(list, list_area, &mut search_state);
    } else {
        frame.render_stateful_widget(list, list_area, &mut app.state);
    }
}

fn render_search(frame: &mut Frame, app: &App, area: Rect) {
    let (query, style) = if app.search_query.is_empty() {
        (
            "Filter by project name…".to_string(),
            Style::new().fg(theme::MUTED),
        )
    } else {
        (
            format!("{}▌", app.search_query),
            Style::new().fg(theme::TRANSITION),
        )
    };

    let search = Paragraph::new(Line::from(vec![
        Span::styled(" / ", Style::new().fg(theme::TRANSITION)),
        Span::styled(query, style),
    ]))
    .block(theme::panel(
        Line::from(Span::styled(
            " FILTER ",
            Style::new()
                .fg(theme::TRANSITION)
                .add_modifier(Modifier::BOLD),
        )),
        true,
    ));

    frame.render_widget(search, area);
}

fn service_item(service: &Service, app: &App, width: usize) -> ListItem<'static> {
    let status = service.status();
    let indicator = status_indicator(&status, app.animation_tick);
    let auto_restart = app.auto_restart.contains(&service.name);

    if width < 24 {
        return ListItem::new(Line::from(vec![
            Span::styled(
                format!("{indicator} "),
                Style::new().fg(theme::status_color(&status)),
            ),
            Span::styled(
                truncate(&service.name, width.saturating_sub(2)),
                Style::new().fg(theme::TEXT),
            ),
        ]));
    }

    let status_label = status_label(&status);
    let reserved_width = status_label.len() + 7;
    let name_width = width.saturating_sub(reserved_width).max(4);
    let name = pad_right(truncate(&service.name, name_width), name_width);
    let boot_symbol = if auto_restart { "↻" } else { " " };
    let boot_color = if auto_restart {
        theme::ACCENT
    } else {
        theme::MUTED
    };

    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{indicator} "),
            Style::new().fg(theme::status_color(&status)),
        ),
        Span::styled(name, Style::new().fg(theme::TEXT)),
        Span::styled(
            format!(" {status_label:>9} "),
            Style::new()
                .fg(theme::status_color(&status))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(boot_symbol, Style::new().fg(boot_color)),
    ]))
}

fn services_title(
    running_count: usize,
    total_count: usize,
    auto_restart_count: usize,
    filtered_count: usize,
    is_filtered: bool,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            " SERVICES ",
            Style::new().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{running_count}/{total_count} UP"),
            Style::new().fg(theme::RUNNING),
        ),
    ];

    if auto_restart_count > 0 {
        spans.push(Span::styled(
            format!("  {auto_restart_count} ↻"),
            Style::new().fg(theme::ACCENT),
        ));
    }

    if is_filtered && filtered_count < total_count {
        spans.push(Span::styled(
            format!("  {filtered_count}/{total_count} shown"),
            Style::new().fg(theme::TRANSITION),
        ));
    }

    Line::from(spans)
}

fn status_label(status: &Status) -> &'static str {
    match status {
        Status::Running => "RUNNING",
        Status::Stopped => "STOPPED",
        Status::Starting => "STARTING",
        Status::Stopping => "STOPPING",
        Status::Pulling => "PULLING",
        Status::Error => "ERROR",
        Status::RuntimeUnavailable => "NO PODMAN",
    }
}

fn status_indicator(status: &Status, tick: u64) -> &'static str {
    match status {
        Status::Running => "●",
        Status::Pulling => "◌",
        Status::Starting => {
            const FRAMES: [&str; 4] = ["◜", "◠", "◝", "◞"];
            FRAMES[((tick / 2) % FRAMES.len() as u64) as usize]
        }
        Status::Stopping => {
            const FRAMES: [&str; 4] = ["◟", "◡", "◞", "◜"];
            FRAMES[((tick / 2) % FRAMES.len() as u64) as usize]
        }
        Status::Stopped | Status::RuntimeUnavailable => "○",
        Status::Error => "×",
    }
}

fn truncate(value: &str, max_width: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_width {
        return value.to_string();
    }
    if max_width <= 1 {
        return "…".chars().take(max_width).collect();
    }

    let mut truncated: String = value.chars().take(max_width - 1).collect();
    truncated.push('…');
    truncated
}

fn pad_right(mut value: String, width: usize) -> String {
    let padding = width.saturating_sub(value.chars().count());
    value.extend(std::iter::repeat_n(' ', padding));
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_preserves_short_names() {
        assert_eq!(truncate("redis", 8), "redis");
    }

    #[test]
    fn truncate_marks_clipped_names() {
        assert_eq!(truncate("postgresql", 6), "postg…");
    }
}
