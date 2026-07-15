use std::fs;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Keybinds {
    pub app: AppKeys,
    pub services: ServicesKeys,
    pub logs: LogsKeys,
}

#[derive(Deserialize, Debug)]
pub struct AppKeys {
    pub quit: String,
    pub search: String,
    pub daemon_menu: String,
    pub refresh: String,
    pub switch_tab_left: String,
    pub switch_tab_right: String,
    pub scroll_down: String,
    pub scroll_up: String,
}

#[derive(Deserialize, Debug)]
pub struct ServicesKeys {
    pub toggle: String,
    #[serde(default = "default_auto_restart_key")]
    pub auto_restart: String,
}

#[derive(Deserialize, Debug)]
pub struct LogsKeys {
    pub toggle_auto_scroll: String,
}

fn default_auto_restart_key() -> String {
    "a".to_string()
}

impl Keybinds {
    pub fn load() -> Self {
        let content = fs::read_to_string("keybinds.toml")
            .unwrap_or_else(|_| include_str!("../keybinds.toml").to_string());
        toml::from_str(&content).expect("Failed to parse keybinds.toml")
    }
}
