use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::{App, Focus, LogTab};

pub(super) fn logs_title(app: &App) -> Line<'static> {
    let selected_name = app
        .state
        .selected()
        .and_then(|index| app.services.get(index))
        .map(|service| service.name.clone())
        .unwrap_or_else(|| "none".to_string());

    let mut spans = vec![
        Span::styled(" Logs ", Style::default().fg(Color::White)),
        Span::styled(selected_name, Style::default().fg(Color::Cyan)),
    ];

    spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
    if app.log_tab == LogTab::Events {
        spans.push(Span::styled(
            "[Events]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Events", Style::default().fg(Color::White)));
    }

    spans.push(Span::styled("  |  ", Style::default().fg(Color::DarkGray)));
    if app.log_tab == LogTab::LiveLogs {
        spans.push(Span::styled(
            "[Live Logs]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Live Logs", Style::default().fg(Color::White)));
    }

    if app.focus == Focus::Logs && app.log_auto_scroll {
        spans.push(Span::styled(
            " [AUTO]",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }

    Line::from(spans)
}
