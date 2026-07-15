use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::app::{App, LogTab};

use super::super::theme;

pub(super) fn logs_title(app: &App) -> Line<'static> {
    let selected_name = app
        .state
        .selected()
        .and_then(|index| app.services.get(index))
        .map(|service| service.name.clone())
        .unwrap_or_else(|| "NO SELECTION".to_string());

    let name_color = if app.state.selected().is_some() {
        theme::ACCENT
    } else {
        theme::MUTED
    };

    let total = app.logs_render_cache.body_line_count;
    let position = if total == 0 {
        String::new()
    } else if app.log_auto_scroll {
        format!("↓ {total}/{total}")
    } else {
        let view_top = app.log_scroll.saturating_add(1);
        format!(" L{view_top}/{total}")
    };

    Line::from(vec![
        Span::styled(
            " ACTIVITY ",
            Style::new().fg(theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(selected_name, Style::new().fg(name_color)),
        Span::styled("  ", Style::new().fg(theme::MUTED)),
        tab("EVENTS", app.log_tab == LogTab::Events),
        Span::raw(" "),
        tab("LIVE", app.log_tab == LogTab::LiveLogs),
        Span::styled("  ", Style::new().fg(theme::MUTED)),
        Span::styled(
            if app.log_auto_scroll {
                "↓ AUTO"
            } else {
                "Ⅱ PAUSED"
            },
            Style::new().fg(if app.log_auto_scroll {
                theme::RUNNING
            } else {
                theme::TRANSITION
            }),
        ),
        Span::styled(format!("  {position}"), Style::new().fg(theme::MUTED)),
    ])
}

fn tab(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            format!(" {label} "),
            Style::new()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )
    } else {
        Span::styled(format!(" {label} "), Style::new().fg(theme::MUTED))
    }
}
