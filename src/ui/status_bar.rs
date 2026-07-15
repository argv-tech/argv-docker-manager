use chrono::Local;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::status::Status;

use super::theme;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let running_services = app
        .services
        .iter()
        .filter(|service| service.status() == Status::Running)
        .count();
    let workspace = app
        .project_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");

    let identity = Line::from(vec![
        Span::styled("◆", Style::new().fg(theme::BRAND)),
        Span::styled(
            " ARGV  DOCKER MANAGER",
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  v{}", env!("CARGO_PKG_VERSION")),
            Style::new().fg(theme::MUTED),
        ),
        Span::styled("  /  ", Style::new().fg(theme::MUTED)),
        Span::styled(workspace.to_string(), Style::new().fg(theme::TEXT)),
        Span::styled("  /  ", Style::new().fg(theme::MUTED)),
        Span::styled(
            Local::now().format("%H:%M").to_string(),
            Style::new().fg(theme::TEXT),
        ),
    ]);

    let health = Line::from(vec![
        signal("DAEMON", app.docker_daemon_running),
        Span::raw("  "),
        signal("CLI", app.docker_command_available),
        Span::raw("  "),
        signal("COMPOSE", app.docker_compose_available),
        Span::styled("  │  ", Style::new().fg(theme::MUTED)),
        Span::styled(
            format!("{running_services}/{} RUNNING", app.services.len()),
            Style::new().fg(if running_services > 0 {
                theme::RUNNING
            } else {
                theme::TEXT
            }),
        ),
        Span::styled(
            format!("  {} AUTO-START", app.auto_restart.len()),
            Style::new().fg(if app.auto_restart.len() == 0 {
                theme::MUTED
            } else {
                theme::ACCENT
            }),
        ),
    ]);

    let header = Paragraph::new(vec![identity, health]).block(
        Block::new()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(theme::MUTED)),
    );
    frame.render_widget(header, area);
}

fn signal(label: &'static str, available: bool) -> Span<'static> {
    let (symbol, state, color) = if available {
        ("●", "READY", theme::RUNNING)
    } else {
        ("×", "DOWN", theme::ERROR)
    };

    Span::styled(
        format!("{symbol} {label} {state}"),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    )
}
