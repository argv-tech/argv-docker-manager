use crate::app::App;

pub(super) struct Keys {
    pub quit: char,
    pub search: char,
    pub stop: char,
    pub start: char,
    pub daemon: char,
    pub scroll_down: char,
    pub scroll_up: char,
    pub switch_tab_left: char,
    pub switch_tab_right: char,
    pub toggle: char,
    pub refresh: char,
}

impl Keys {
    pub(super) fn from_app(app: &App) -> Self {
        Self {
            quit: app.keybinds.app.quit.chars().next().unwrap_or('q'),
            search: app.keybinds.app.search.chars().next().unwrap_or('/'),
            stop: app.keybinds.services.stop.chars().next().unwrap_or('s'),
            start: app.keybinds.services.start.chars().next().unwrap_or('S'),
            daemon: app.keybinds.app.daemon_menu.chars().next().unwrap_or('d'),
            scroll_down: app.keybinds.app.scroll_down.chars().next().unwrap_or('j'),
            scroll_up: app.keybinds.app.scroll_up.chars().next().unwrap_or('k'),
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
            toggle: app.keybinds.services.toggle.chars().next().unwrap_or(' '),
            refresh: app.keybinds.app.refresh.chars().next().unwrap_or('r'),
        }
    }
}
