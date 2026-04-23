use crate::app::{App, DaemonAction};

pub(super) fn in_overlay_mode(app: &App) -> bool {
    app.search_mode || app.daemon_start_mode || app.daemon_menu_mode
}

pub(super) fn select_searched_service(app: &mut App) {
    if let Some(index) = app.services.iter().position(|service| {
        service
            .name
            .to_lowercase()
            .starts_with(&app.search_query.to_lowercase())
    }) {
        app.state.select(Some(index));
    }
}

pub(super) fn daemon_next(app: &mut App) {
    app.daemon_action_selected = match app.daemon_action_selected {
        DaemonAction::Start => DaemonAction::Stop,
        DaemonAction::Stop => DaemonAction::Restart,
        DaemonAction::Restart => DaemonAction::Start,
    };
}

pub(super) fn daemon_previous(app: &mut App) {
    app.daemon_action_selected = match app.daemon_action_selected {
        DaemonAction::Start => DaemonAction::Restart,
        DaemonAction::Stop => DaemonAction::Start,
        DaemonAction::Restart => DaemonAction::Stop,
    };
}
