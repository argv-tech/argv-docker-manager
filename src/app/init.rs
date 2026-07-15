use std::path::Path;

use crate::app::state::{App, DaemonAction, Focus, LogTab};
use crate::auto_restart::AutoRestartConfig;
use crate::config::Keybinds;
use crate::docker::client::DockerClient;
use crate::service::Service;
use crate::status::ToastState;

impl App {
    pub fn new(keybinds: Keybinds) -> Self {
        let project_root = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let service_names = get_service_names(&project_root);
        let (auto_restart, auto_restart_error) = match AutoRestartConfig::load(&project_root) {
            Ok(config) => (config, None),
            Err(error) => (AutoRestartConfig::default(), Some(error.to_string())),
        };

        let docker_running = DockerClient::docker_info_ok();
        let docker_command_available = DockerClient::docker_cli_ok();
        let docker_compose_available = DockerClient::compose_cli_ok();

        let (toast, toast_timer) = if let Some(error) = auto_restart_error {
            (
                Some(crate::toast::Toast {
                    state: ToastState::Error,
                    message: format!("Auto-restart config error: {error}"),
                }),
                5,
            )
        } else if !docker_compose_available {
            (
                Some(crate::toast::Toast {
                    state: ToastState::Error,
                    message: "Docker Compose not found. Services may not work.".to_string(),
                }),
                5,
            )
        } else if !docker_command_available {
            (
                Some(crate::toast::Toast {
                    state: ToastState::Error,
                    message: "Docker CLI not found.".to_string(),
                }),
                5,
            )
        } else if !docker_running {
            (
                Some(crate::toast::Toast {
                    state: ToastState::Warning,
                    message: "Docker daemon not running.".to_string(),
                }),
                4,
            )
        } else {
            (
                Some(crate::toast::Toast {
                    state: ToastState::Info,
                    message: "Welcome to Docker Manager".to_string(),
                }),
                3,
            )
        };

        let mut app = Self {
            state: ratatui::widgets::ListState::default(),
            services: service_names.into_iter().map(Service::new).collect(),
            project_root,
            auto_restart,
            toast,
            toast_timer,

            search_mode: false,
            search_query: String::new(),
            docker_daemon_running: docker_running,
            docker_command_available,
            docker_compose_available,
            daemon_menu_mode: false,
            daemon_action_selected: DaemonAction::Start,
            daemon_start_mode: false,
            password_input: String::new(),
            focus: Focus::Services,
            first_status_check: true,
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
            keybinds,
        };
        app.refresh_statuses();
        app.populate_initial_logs();
        app.start_event_listeners();
        app
    }
}

fn get_service_names(project_root: &Path) -> Vec<String> {
    match std::fs::read_dir(project_root.join("containers")) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .filter_map(|dir| {
                    let compose_path = dir.path().join("docker-compose.yml");
                    if compose_path.exists() {
                        dir.file_name().to_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            names.sort_by_key(|a| a.to_lowercase());
            names
        }
        Err(_) => vec![],
    }
}
