use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph},
};

use crate::app::{App, DaemonAction};

use super::theme;

pub fn render(frame: &mut Frame, app: &App) {
    if app.daemon_menu_mode {
        render_daemon_menu(frame, app);
    }

    if app.daemon_start_mode {
        render_password_prompt(frame, app);
    }

    if let Some(toast) = &app.toast {
        let frame_area = frame.area();
        let width = frame_area.width.saturating_sub(2).min(50);
        let area = Rect {
            x: frame_area
                .x
                .saturating_add(frame_area.width.saturating_sub(width).saturating_sub(1)),
            y: frame_area.y.saturating_add(1),
            width,
            height: frame_area.height.min(3),
        };
        if area.width > 0 && area.height > 0 {
            frame.render_widget(
                crate::toast::create_toast_widget(toast, app.animation_tick),
                area,
            );
        }
    }
}

fn render_daemon_menu(frame: &mut Frame, app: &App) {
    let area = centered_rect(72, 14, frame.area());
    frame.render_widget(Clear, area);

    let popup = theme::panel(
        Line::from(Span::styled(
            " DAEMON CONTROL ",
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )),
        true,
    );

    let inner = popup.inner(area);
    frame.render_widget(popup, area);

    let [status_area, list_area, hints_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(2),
    ])
    .areas(inner);

    let (status_label, status_color) = daemon_status_style(app);
    let status_line = Line::from(vec![
        Span::styled("Docker status: ", Style::new().fg(Color::Gray)),
        Span::styled(
            status_label,
            Style::new().fg(status_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    frame.render_widget(Paragraph::new(status_line), status_area);

    let actions = [
        DaemonAction::Start,
        DaemonAction::Stop,
        DaemonAction::Restart,
    ];
    let items: Vec<ListItem> = actions
        .iter()
        .map(|action| {
            let unavailable = matches!(
                (action, app.docker_daemon_running),
                (DaemonAction::Start, true) | (DaemonAction::Stop, false)
            );
            let (label_color, desc_color) = if unavailable {
                (Color::DarkGray, Color::DarkGray)
            } else {
                (Color::White, Color::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(action_label(*action), Style::new().fg(label_color)),
                Span::styled(
                    format!("  - {}", action_description(*action)),
                    Style::new().fg(desc_color),
                ),
            ]))
        })
        .collect();

    let selected_index = match app.daemon_action_selected {
        DaemonAction::Start => 0,
        DaemonAction::Stop => 1,
        DaemonAction::Restart => 2,
    };

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(selected_index));

    let list = List::new(items)
        .block(theme::panel(Line::from(" ACTIONS "), false))
        .highlight_style(
            Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌ ");

    frame.render_stateful_widget(list, list_area, &mut state);

    let scroll_key = &app.keybinds.app.scroll_down;
    let hints = format!("{scroll_key}/k or ↓/↑: move   Enter: continue   Esc: cancel");
    frame.render_widget(
        Paragraph::new(hints)
            .alignment(Alignment::Left)
            .style(Style::new().fg(Color::DarkGray)),
        hints_area,
    );
}

fn render_password_prompt(frame: &mut Frame, app: &App) {
    let area = centered_rect(72, 10, frame.area());
    frame.render_widget(Clear, area);

    let popup = theme::panel(
        Line::from(Span::styled(
            " CONFIRM DAEMON ACTION ",
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )),
        true,
    );
    let inner = popup.inner(area);
    frame.render_widget(popup, area);

    let [action_area, input_area, hints_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .areas(inner);

    let title = match app.daemon_action_selected {
        DaemonAction::Start => "Start Docker daemon",
        DaemonAction::Stop => "Stop Docker daemon",
        DaemonAction::Restart => "Restart Docker daemon",
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Action: ", Style::new().fg(Color::Gray)),
            Span::styled(
                title,
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ])),
        action_area,
    );

    let password_mask = "*".repeat(app.password_input.chars().count());
    let input_text = if password_mask.is_empty() {
        "Type sudo password...".to_string()
    } else {
        password_mask
    };

    frame.render_widget(
        Paragraph::new(input_text)
            .alignment(Alignment::Left)
            .style(if app.password_input.is_empty() {
                Style::new().fg(Color::DarkGray)
            } else {
                Style::new().fg(Color::White)
            })
            .block(theme::panel(Line::from(" PASSWORD "), false)),
        input_area,
    );

    frame.render_widget(
        Paragraph::new("Enter: run action   Esc: cancel")
            .alignment(Alignment::Left)
            .style(Style::new().fg(Color::DarkGray)),
        hints_area,
    );
}

fn action_label(action: DaemonAction) -> &'static str {
    match action {
        DaemonAction::Start => "Start",
        DaemonAction::Stop => "Stop",
        DaemonAction::Restart => "Restart",
    }
}

fn action_description(action: DaemonAction) -> &'static str {
    match action {
        DaemonAction::Start => "Bring up docker.service and docker.socket",
        DaemonAction::Stop => "Stop active services first, then shut daemon down",
        DaemonAction::Restart => "Stop active services first, then restart daemon",
    }
}

fn daemon_status_style(app: &App) -> (&'static str, Color) {
    if app.docker_daemon_running {
        ("RUNNING", Color::Green)
    } else {
        ("STOPPED", Color::Red)
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2)).max(1);
    let height = height.min(area.height.saturating_sub(2)).max(1);
    let [_, vertical, _] = Layout::vertical([
        Constraint::Length(area.height.saturating_sub(height) / 2),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .areas(area);

    let [_, centered, _] = Layout::horizontal([
        Constraint::Length(area.width.saturating_sub(width) / 2),
        Constraint::Length(width),
        Constraint::Min(0),
    ])
    .areas(vertical);

    centered
}
