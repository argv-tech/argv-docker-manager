use std::fs;

use serde::Deserialize;
use serde::{Serialize, Serializer};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    #[default]
    Docker,
    Podman,
}

impl Runtime {
    pub fn name(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }

    pub fn command(self) -> std::process::Command {
        std::process::Command::new(self.name())
    }

    pub fn daemon_service_active(self) -> bool {
        match self {
            Self::Docker => crate::docker::daemon::docker_service_active(),
            Self::Podman => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub general: GeneralSettings,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct GeneralSettings {
    #[serde(default)]
    pub runtime: Runtime,
}

impl Serialize for Runtime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.name())
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let content = toml::to_string_pretty(self).expect("settings serialize");
        std::fs::write(path, content)
    }
}

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
    #[serde(default = "default_focus_services_key")]
    pub focus_services: String,
    #[serde(default = "default_focus_logs_key")]
    pub focus_logs: String,
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

fn default_focus_services_key() -> String {
    "h".to_string()
}

fn default_focus_logs_key() -> String {
    "l".to_string()
}

impl Keybinds {
    pub fn load() -> Self {
        let content = fs::read_to_string("keybinds.toml")
            .unwrap_or_else(|_| include_str!("../keybinds.toml").to_string());
        toml::from_str(&content).expect("Failed to parse keybinds.toml")
    }
}
