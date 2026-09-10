use std::collections::HashMap;

use crate::app::state::App;
use crate::podman::events::{ProjectEventTargets, append_project_event, spawn_projects_listener};

impl App {
    pub fn start_event_listeners(&mut self) {
        if self.event_listener_running {
            return;
        }

        if !self.podman_available {
            return;
        }

        let mut project_targets = HashMap::new();
        for service in &self.services {
            project_targets.insert(
                service.name.clone(),
                ProjectEventTargets {
                    status: std::sync::Arc::clone(&service.status),
                    events: std::sync::Arc::clone(&service.events),
                    pull_progress: std::sync::Arc::clone(&service.pull_progress),
                },
            );
        }

        self.event_listener_handle = Some(spawn_projects_listener(project_targets));
        self.event_listener_running = true;

        for service in &self.services {
            append_project_event(&service.events, &service.name, "listener attached");
        }
    }

    pub fn stop_event_listeners(&mut self) {
        if let Some(mut handle) = self.event_listener_handle.take() {
            handle.signal_shutdown();
        }
        self.event_listener_running = false;
    }
}
