use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, KeyEventKind};

use crate::app::App;
use crate::status::Status;

mod input;
mod keys;
mod overlays;

use input::handle_key;
use keys::Keys;

pub async fn handle_events(app: &mut App, poll_timeout: Duration) -> io::Result<bool> {
    app.animation_tick = app.animation_tick.wrapping_add(1);
    if app.daemon_probe_cooldown_ticks > 0 {
        app.daemon_probe_cooldown_ticks = app.daemon_probe_cooldown_ticks.saturating_sub(1);
    }

    let keys = Keys::from_app(app);

    if event::poll(poll_timeout)? {
        let event = event::read()?;
        if let event::Event::Key(key) = event
            && key.kind == KeyEventKind::Press
            && !handle_key(app, key.code, &keys)
        {
            return Ok(false);
        }
    } else {
        refresh_if_transitioning(app);
    }

    update_toast_timer(app);
    app.sync_live_log_listener();
    Ok(true)
}

pub(crate) fn refresh_if_transitioning(app: &mut App) {
    const STATUS_REFRESH_COOLDOWN_TICKS: u8 = 24;

    if app.status_refresh_cooldown_ticks > 0 {
        app.status_refresh_cooldown_ticks = app.status_refresh_cooldown_ticks.saturating_sub(1);
        return;
    }

    let needs_refresh = app.services.iter().any(|service| {
        matches!(
            service.status(),
            Status::Pulling | Status::Starting | Status::Stopping
        )
    });

    if needs_refresh {
        app.refresh_statuses();
        app.status_refresh_cooldown_ticks = STATUS_REFRESH_COOLDOWN_TICKS;
    }
}

fn update_toast_timer(app: &mut App) {
    const TOAST_TICKS_PER_SECOND: u8 = 30;

    if app.toast_timer > 0 {
        app.toast_tick_accumulator = app.toast_tick_accumulator.saturating_add(1);
        if app.toast_tick_accumulator >= TOAST_TICKS_PER_SECOND {
            app.toast_tick_accumulator = 0;
            app.toast_timer = app.toast_timer.saturating_sub(1);
            if app.toast_timer == 0 {
                app.toast = None;
            }
        }
    }
}
