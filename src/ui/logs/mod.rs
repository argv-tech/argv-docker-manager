use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Text},
};

use crate::app::{App, Focus, LogTab};
mod colorize;
mod progress;
mod title;

use colorize::{colorize_events, colorize_logs};
use progress::{event_progress_line, placeholder_text};
use title::logs_title;

use super::theme;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let progress_line = refresh_logs_cache(app);
    let progress_line_count = if progress_line.is_some() { 2 } else { 0 };
    let total_lines = app
        .logs_render_cache
        .entry(app.state.selected(), app.log_tab)
        .map(|cache| cache.body_line_count)
        .unwrap_or(0)
        .saturating_add(progress_line_count);

    if app.log_auto_scroll {
        let visible_lines = area.height.saturating_sub(2);
        app.log_scroll = total_lines.saturating_sub(visible_lines);
    }

    let block = theme::panel(logs_title(app), app.focus == Focus::Logs);
    let body_area = block.inner(area);
    frame.render_widget(block, area);
    frame
        .buffer_mut()
        .set_style(body_area, Style::new().fg(theme::TEXT));

    if let Some(cache) = app
        .logs_render_cache
        .entry(app.state.selected(), app.log_tab)
    {
        render_log_lines(
            frame,
            &cache.body,
            progress_line.as_ref(),
            body_area,
            app.log_scroll,
        );
    }
}

fn refresh_logs_cache(app: &mut App) -> Option<Line<'static>> {
    let Some(index) = app.state.selected() else {
        if app.logs_render_cache.entry(None, app.log_tab).is_none() {
            let (body, body_line_count) = placeholder_text("Select a service to view activity");
            app.logs_render_cache
                .insert(crate::app::state::LogsRenderCacheEntry {
                    service_index: None,
                    tab: app.log_tab,
                    buffer_revision: 0,
                    body,
                    body_line_count,
                });
        }
        return None;
    };

    let cached_revision = app
        .logs_render_cache
        .entry(Some(index), app.log_tab)
        .map(|cache| cache.buffer_revision);

    let snapshot = {
        let service = &app.services[index];
        match app.log_tab {
            LogTab::Events => {
                let buffer = service.events.lock().unwrap();
                buffer.snapshot_if_changed(cached_revision)
            }
            LogTab::LiveLogs => {
                let buffer = service.live_logs.lock().unwrap();
                buffer.snapshot_if_changed(cached_revision)
            }
        }
    };

    if let Some((buffer_revision, logs_snapshot)) = snapshot {
        let body = match app.log_tab {
            LogTab::Events => {
                if logs_snapshot.is_empty() {
                    placeholder_text("No events yet — start the service").0
                } else {
                    colorize_events(&logs_snapshot)
                }
            }
            LogTab::LiveLogs => {
                if logs_snapshot.is_empty() {
                    placeholder_text("No live logs yet — start the service").0
                } else {
                    colorize_logs(&logs_snapshot)
                }
            }
        };

        app.logs_render_cache
            .insert(crate::app::state::LogsRenderCacheEntry {
                service_index: Some(index),
                tab: app.log_tab,
                buffer_revision,
                body_line_count: u16::try_from(body.lines.len()).unwrap_or(u16::MAX),
                body,
            });
    }

    if app.log_tab == LogTab::Events {
        let service = &app.services[index];
        let status = service.status();
        let pull_progress = service.pull_progress.lock().unwrap().clone();

        return event_progress_line(&status, pull_progress.as_deref(), app.animation_tick);
    }

    None
}

fn render_log_lines(
    frame: &mut Frame,
    body: &Text<'_>,
    progress_line: Option<&Line<'_>>,
    area: Rect,
    scroll: u16,
) {
    let progress_line_count = if progress_line.is_some() { 2 } else { 0 };
    let blank_line = Line::from("");

    for (row, logical_index) in (scroll as usize..).take(area.height as usize).enumerate() {
        let line = match (progress_line, logical_index) {
            (Some(line), 0) => Some(line),
            (Some(_), 1) => Some(&blank_line),
            _ => body
                .lines
                .get(logical_index.saturating_sub(progress_line_count)),
        };
        let Some(line) = line else {
            break;
        };

        let line_area = Rect::new(area.x, area.y + row as u16, area.width, 1);
        frame.render_widget(line, line_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Focus, LogTab};
    use crate::config::{AppKeys, Keybinds, LogsKeys, ServicesKeys};
    use crate::service::Service;
    use ratatui::{Terminal, backend::TestBackend};

    fn test_app() -> App {
        App {
            state: ratatui::widgets::ListState::default(),
            services: vec![
                Service::new("redis".to_string()),
                Service::new("postgres".to_string()),
            ],
            project_root: std::env::temp_dir(),
            auto_restart: Default::default(),
            toast: None,
            toast_timer: 0,
            search_mode: false,
            search_query: String::new(),
            podman_available: true,
            podman_command_available: true,
            podman_compose_available: true,
            focus: Focus::Logs,
            first_status_check: false,
            log_scroll: 0,
            log_auto_scroll: true,
            log_tab: LogTab::Events,
            animation_tick: 0,
            next_status_refresh: std::time::Instant::now(),
            status_refresh_task: None,
            runtime_probe_cooldown_ticks: 0,
            event_listener_running: false,
            event_listener_handle: None,
            toast_tick_accumulator: 0,
            live_log_retry_cooldown_ticks: 0,
            logs_render_cache: Default::default(),
            keybinds: Keybinds {
                app: AppKeys {
                    quit: "q".to_string(),
                    search: "/".to_string(),
                    refresh: "r".to_string(),
                    switch_tab_left: "[".to_string(),
                    switch_tab_right: "]".to_string(),
                    scroll_down: "j".to_string(),
                    scroll_up: "k".to_string(),
                    focus_services: "h".to_string(),
                    focus_logs: "l".to_string(),
                },
                services: ServicesKeys {
                    toggle: "s".to_string(),
                    auto_restart: "a".to_string(),
                },
                logs: LogsKeys {
                    toggle_auto_scroll: " ".to_string(),
                },
            },
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn navigation_does_not_wait_for_pending_status_refresh() {
        let mut app = test_app();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        app.status_refresh_task = Some(tokio::task::spawn_blocking(move || {
            let _ = wait.recv();
            Default::default()
        }));

        // The worker cannot finish until after navigation and result polling return.
        app.refresh_statuses();
        app.next();
        app.apply_status_refresh().await;
        app.next();
        app.previous();
        assert_eq!(app.state.selected(), Some(0));
        assert!(!app.status_refresh_task.as_ref().unwrap().is_finished());

        release.send(()).unwrap();
        assert!(app.status_refresh_task.take().unwrap().await.is_ok());
    }

    #[test]
    fn refresh_logs_cache_reuses_body_when_revision_is_unchanged() {
        let mut app = test_app();
        app.state.select(Some(0));

        let _ = refresh_logs_cache(&mut app);
        let cache = app
            .logs_render_cache
            .entry_mut(Some(0), LogTab::Events)
            .unwrap();
        cache.body = Text::from("sentinel");
        cache.body_line_count = 1;

        let _ = refresh_logs_cache(&mut app);

        assert_eq!(
            app.logs_render_cache
                .entry(Some(0), LogTab::Events)
                .unwrap()
                .body
                .lines[0]
                .spans[0]
                .content
                .as_ref(),
            "sentinel"
        );
        assert_eq!(
            app.logs_render_cache
                .entry(Some(0), LogTab::Events)
                .unwrap()
                .body_line_count,
            1
        );
    }

    #[test]
    fn refresh_logs_cache_invalidates_body_when_buffer_revision_changes() {
        let mut app = test_app();
        app.state.select(Some(0));

        let _ = refresh_logs_cache(&mut app);
        let cache = app
            .logs_render_cache
            .entry_mut(Some(0), LogTab::Events)
            .unwrap();
        cache.body = Text::from("stale");
        cache.body_line_count = 1;
        app.services[0]
            .events
            .lock()
            .unwrap()
            .push_line("[event] redis start");

        let _ = refresh_logs_cache(&mut app);

        assert_ne!(
            app.logs_render_cache
                .entry(Some(0), LogTab::Events)
                .unwrap()
                .body
                .lines[0]
                .spans[0]
                .content
                .as_ref(),
            "stale"
        );
    }

    #[test]
    fn refresh_logs_cache_reuses_unchanged_service_body_after_switching_back() {
        let mut app = test_app();
        app.state.select(Some(0));

        let _ = refresh_logs_cache(&mut app);
        let cache = app
            .logs_render_cache
            .entry_mut(Some(0), LogTab::Events)
            .unwrap();
        cache.body = Text::from("sentinel");
        cache.body_line_count = 1;

        app.state.select(Some(1));
        let _ = refresh_logs_cache(&mut app);
        app.state.select(Some(0));
        let _ = refresh_logs_cache(&mut app);

        assert_eq!(
            app.logs_render_cache
                .entry(Some(0), LogTab::Events)
                .unwrap()
                .body
                .lines[0]
                .spans[0]
                .content
                .as_ref(),
            "sentinel"
        );
    }

    #[test]
    fn render_log_lines_applies_scroll_across_progress_and_body() {
        let backend = TestBackend::new(12, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let body = Text::from(vec![Line::from("first"), Line::from("second")]);
        let progress = Line::from("progress");

        terminal
            .draw(|frame| {
                render_log_lines(frame, &body, Some(&progress), frame.area(), 1);
            })
            .unwrap();

        terminal
            .backend()
            .assert_buffer_lines(["            ", "first       ", "second      "]);
    }
}
