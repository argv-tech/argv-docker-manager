use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, Focus, LogTab};
mod colorize;
mod progress;
mod title;

use colorize::{colorize_events, colorize_logs};
use progress::{event_progress_line, placeholder_text};
use title::logs_title;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let (logs_content, total_lines) = selected_logs(app);
    let title = logs_title(app);
    let border_color = if app.focus == Focus::Logs {
        Color::Blue
    } else {
        Color::White
    };

    if app.log_auto_scroll {
        let visible_lines = area.height.saturating_sub(2);
        app.log_scroll = total_lines.saturating_sub(visible_lines);
    }

    let logs_widget = Paragraph::new(logs_content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .style(Style::default().fg(Color::Gray))
        .scroll((app.log_scroll, 0));

    frame.render_widget(logs_widget, area);
}

fn selected_logs(app: &mut App) -> (Text<'static>, u16) {
    let Some(index) = app.state.selected() else {
        return placeholder_text("Select a service to view logs");
    };

    let (buffer_revision, logs_snapshot) = {
        let service = &app.services[index];
        match app.log_tab {
            LogTab::Events => {
                let buffer = service.events.lock().unwrap();
                (buffer.revision(), buffer.snapshot())
            }
            LogTab::LiveLogs => {
                let buffer = service.live_logs.lock().unwrap();
                (buffer.revision(), buffer.snapshot())
            }
        }
    };

    if app.logs_render_cache.service_index != Some(index)
        || app.logs_render_cache.tab != app.log_tab
        || app.logs_render_cache.buffer_revision != buffer_revision
    {
        let body = match app.log_tab {
            LogTab::Events => {
                if logs_snapshot.is_empty() {
                    placeholder_text("No events yet - start the service to see events").0
                } else {
                    colorize_events(&logs_snapshot)
                }
            }
            LogTab::LiveLogs => {
                if logs_snapshot.is_empty() {
                    placeholder_text("No live logs yet - start the service to see logs").0
                } else {
                    colorize_logs(&logs_snapshot)
                }
            }
        };

        app.logs_render_cache = crate::app::state::LogsRenderCache {
            service_index: Some(index),
            tab: app.log_tab,
            buffer_revision,
            body_line_count: body.lines.len() as u16,
            body,
        };
    }

    if app.log_tab == LogTab::Events {
        let service = &app.services[index];
        let status = service.status();
        let pull_progress = service.pull_progress.lock().unwrap().clone();

        if let Some(progress_line) =
            event_progress_line(&status, pull_progress.as_deref(), app.animation_tick)
        {
            let mut lines = Vec::with_capacity(app.logs_render_cache.body.lines.len() + 2);
            lines.push(progress_line);
            lines.push(Line::from(""));
            lines.extend(app.logs_render_cache.body.lines.iter().cloned());
            return (
                Text::from(lines),
                app.logs_render_cache.body_line_count.saturating_add(2),
            );
        }
    }

    (
        app.logs_render_cache.body.clone(),
        app.logs_render_cache.body_line_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DaemonAction, Focus, LogTab};
    use crate::config::{AppKeys, Keybinds, LogsKeys, ServicesKeys};
    use crate::service::Service;

    fn test_app() -> App {
        App {
            state: ratatui::widgets::ListState::default(),
            services: vec![Service::new("redis".to_string())],
            service_names: vec!["redis".to_string()],
            toast: None,
            toast_timer: 0,
            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: true,
            docker_command_available: true,
            docker_compose_available: true,
            daemon_menu_mode: false,
            daemon_action_selected: DaemonAction::Start,
            daemon_start_mode: false,
            password_input: String::new(),
            focus: Focus::Logs,
            first_status_check: false,
            log_scroll: 0,
            log_auto_scroll: true,
            log_tab: LogTab::Events,
            animation_tick: 0,
            status_refresh_cooldown_ticks: 0,
            daemon_probe_cooldown_ticks: 0,
            event_listener_running: false,
            event_listener_handle: None,
            toast_tick_accumulator: 0,
            logs_render_cache: Default::default(),
            keybinds: Keybinds {
                app: AppKeys {
                    quit: "q".to_string(),
                    search: "/".to_string(),
                    daemon_menu: "d".to_string(),
                    refresh: "r".to_string(),
                    switch_tab_left: "[".to_string(),
                    switch_tab_right: "]".to_string(),
                    scroll_down: "j".to_string(),
                    scroll_up: "k".to_string(),
                },
                services: ServicesKeys {
                    stop: "s".to_string(),
                    start: "S".to_string(),
                    toggle: " ".to_string(),
                },
                logs: LogsKeys {
                    toggle_auto_scroll: " ".to_string(),
                },
            },
        }
    }

    #[test]
    fn selected_logs_reuses_cached_body_when_revision_is_unchanged() {
        let mut app = test_app();
        app.state.select(Some(0));

        let _ = selected_logs(&mut app);
        app.logs_render_cache.body = Text::from("sentinel");
        app.logs_render_cache.body_line_count = 1;

        let (logs, line_count) = selected_logs(&mut app);

        assert_eq!(logs.lines[0].spans[0].content.as_ref(), "sentinel");
        assert_eq!(line_count, 1);
    }

    #[test]
    fn selected_logs_invalidates_cache_when_buffer_revision_changes() {
        let mut app = test_app();
        app.state.select(Some(0));

        let _ = selected_logs(&mut app);
        app.logs_render_cache.body = Text::from("stale");
        app.logs_render_cache.body_line_count = 1;
        app.services[0]
            .events
            .lock()
            .unwrap()
            .push_line("[event] redis start");

        let (logs, _) = selected_logs(&mut app);

        assert_ne!(logs.lines[0].spans[0].content.as_ref(), "stale");
    }
}
