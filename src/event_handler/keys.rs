use crate::app::App;

pub(super) struct Keys {
    pub quit: char,
    pub search: char,
    pub daemon: char,
    pub scroll_down: char,
    pub scroll_up: char,
    pub focus_services: char,
    pub focus_logs: char,
    pub switch_tab_left: char,
    pub switch_tab_right: char,
    pub service_toggle: char,
    pub log_auto_scroll: char,
    pub auto_restart: char,
    pub refresh: char,
}

impl Keys {
    pub(super) fn from_app(app: &App) -> Self {
        Self {
            quit: app.keybinds.app.quit.chars().next().unwrap_or('q'),
            search: app.keybinds.app.search.chars().next().unwrap_or('/'),
            daemon: app.keybinds.app.daemon_menu.chars().next().unwrap_or('d'),
            scroll_down: app.keybinds.app.scroll_down.chars().next().unwrap_or('j'),
            scroll_up: app.keybinds.app.scroll_up.chars().next().unwrap_or('k'),
            focus_services: app
                .keybinds
                .app
                .focus_services
                .chars()
                .next()
                .unwrap_or('h'),
            focus_logs: app.keybinds.app.focus_logs.chars().next().unwrap_or('l'),
            switch_tab_left: app
                .keybinds
                .app
                .switch_tab_left
                .chars()
                .next()
                .unwrap_or('['),
            switch_tab_right: app
                .keybinds
                .app
                .switch_tab_right
                .chars()
                .next()
                .unwrap_or(']'),
            service_toggle: app.keybinds.services.toggle.chars().next().unwrap_or('s'),
            log_auto_scroll: app
                .keybinds
                .logs
                .toggle_auto_scroll
                .chars()
                .next()
                .unwrap_or(' '),
            auto_restart: app
                .keybinds
                .services
                .auto_restart
                .chars()
                .next()
                .unwrap_or('a'),
            refresh: app.keybinds.app.refresh.chars().next().unwrap_or('r'),
        }
    }
}
