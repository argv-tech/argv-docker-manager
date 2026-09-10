use ratatui::crossterm::event::KeyCode;

use crate::app::{App, Focus, LogTab};
use crate::status::{Status, ToastState};

use super::keys::Keys;
use super::overlays::{in_overlay_mode, select_searched_service};

pub(super) fn handle_key(app: &mut App, code: KeyCode, keys: &Keys) -> bool {
    if matches!(code, KeyCode::Char(c) if c == keys.quit) && !in_overlay_mode(app) {
        return false;
    }

    if matches!(code, KeyCode::Char(c) if c == keys.search)
        && !in_overlay_mode(app)
        && app.focus == Focus::Services
    {
        app.search_mode = true;
        app.search_query.clear();
        return true;
    }

    match code {
        KeyCode::Esc if app.search_mode => {
            app.search_mode = false;
            app.search_query.clear();
        }
        KeyCode::Enter if app.search_mode => {
            select_searched_service(app);
            app.search_mode = false;
            app.search_query.clear();
        }
        _ if app.search_mode => match code {
            KeyCode::Char(c) => app.search_query.push(c),
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            _ => {}
        },
        _ => handle_normal_mode(app, code, keys),
    }

    true
}

fn handle_normal_mode(app: &mut App, code: KeyCode, keys: &Keys) {
    match code {
        KeyCode::Char(c) if c == keys.focus_services => app.focus = Focus::Services,
        KeyCode::Char(c) if c == keys.focus_logs => app.focus = Focus::Logs,
        KeyCode::Char(c) if c == keys.scroll_down => move_down(app),
        KeyCode::Down => move_down(app),
        KeyCode::Char(c) if c == keys.scroll_up => move_up(app),
        KeyCode::Up => move_up(app),
        KeyCode::PageDown if app.focus == Focus::Logs => scroll_logs(app, 10),
        KeyCode::PageUp if app.focus == Focus::Logs => scroll_logs(app, -10),
        KeyCode::Tab => {
            if app.focus == Focus::Services {
                app.next();
            }
        }
        KeyCode::BackTab => {
            if app.focus == Focus::Services {
                app.previous();
            }
        }
        KeyCode::Char(c) if c == keys.service_toggle && app.focus == Focus::Services => {
            if selected_service_transitioning(app) {
                app.set_toast(ToastState::Info, "Service is busy, wait for transition", 2);
            } else {
                app.toggle_service();
            }
        }
        KeyCode::Char(c) if c == keys.log_auto_scroll && app.focus == Focus::Logs => {
            app.log_auto_scroll = !app.log_auto_scroll;
        }
        KeyCode::Char(c) if c == keys.auto_restart => {
            if app.focus == Focus::Services {
                app.toggle_selected_auto_restart();
            }
        }
        KeyCode::Char(c) if c == keys.refresh => {
            app.refresh_statuses();
            app.set_toast(ToastState::Info, "Refreshing statuses", 3);
        }
        KeyCode::Char(c) if c == keys.switch_tab_left => toggle_log_tab(app),
        KeyCode::Char(c) if c == keys.switch_tab_right => toggle_log_tab(app),
        _ => {}
    }
}

fn selected_service_transitioning(app: &App) -> bool {
    app.state
        .selected()
        .map(|index| {
            matches!(
                app.services[index].status(),
                Status::Pulling | Status::Starting | Status::Stopping
            )
        })
        .unwrap_or(false)
}

fn move_down(app: &mut App) {
    if app.focus == Focus::Services {
        app.next();
    } else {
        app.log_scroll += 1;
        app.log_auto_scroll = false;
    }
}

fn move_up(app: &mut App) {
    if app.focus == Focus::Services {
        app.previous();
    } else {
        app.log_scroll = app.log_scroll.saturating_sub(1);
        app.log_auto_scroll = false;
    }
}

fn scroll_logs(app: &mut App, amount: i16) {
    app.log_scroll = if amount.is_negative() {
        app.log_scroll.saturating_sub(amount.unsigned_abs())
    } else {
        app.log_scroll.saturating_add(amount as u16)
    };
    app.log_auto_scroll = false;
}

fn toggle_log_tab(app: &mut App) {
    app.log_tab = match app.log_tab {
        LogTab::Events => LogTab::LiveLogs,
        LogTab::LiveLogs => LogTab::Events,
    };

    if app.log_tab == LogTab::LiveLogs {
        app.log_auto_scroll = true;
    }
}
