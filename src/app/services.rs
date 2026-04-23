use std::sync::Arc;
use std::thread;

use crate::app::compose_images;
use crate::app::pull_progress;
use crate::app::state::App;
use crate::docker::client::DockerClient;
use crate::docker::compose::ComposeProject;
use crate::docker::daemon;
use crate::docker::events::{append_project_event, append_project_runtime_details};
use crate::docker::process::{run_stream, run_stream_with_line_callback};
use crate::status::{Status, ToastState};

impl App {
    pub fn refresh_statuses(&mut self) {
        const DAEMON_PROBE_COOLDOWN_TICKS: u8 = 60;

        let should_probe_daemon = self.first_status_check
            || self.daemon_probe_cooldown_ticks == 0
            || !self.docker_daemon_running;
        let daemon_running = if should_probe_daemon {
            self.daemon_probe_cooldown_ticks = DAEMON_PROBE_COOLDOWN_TICKS;
            daemon::docker_service_active() && DockerClient::docker_info_ok()
        } else {
            self.docker_daemon_running
        };
        let daemon_changed = daemon_running != self.docker_daemon_running;
        self.docker_daemon_running = daemon_running;
        let has_transitioning_services = self
            .services
            .iter()
            .any(|service| service.is_transitioning());

        if !self.docker_daemon_running {
            self.stop_event_listeners();
            for service in &self.services {
                service.set_status(Status::DaemonNotRunning);
                service.clear_pull_progress();
            }
        } else if self.first_status_check || daemon_changed || has_transitioning_services {
            let service_names: Vec<String> = self.services.iter().map(|s| s.name.clone()).collect();
            let batch_statuses = DockerClient::get_batch_statuses(&service_names);

            for service in &self.services {
                if let Some(actual_status) = batch_statuses.get(&service.name).copied() {
                    let mut status_lock = service.status.lock().unwrap();
                    match *status_lock {
                        Status::Pulling => {
                            if actual_status == Status::Running {
                                service.clear_pull_progress();
                                *status_lock = Status::Running;
                            }
                        }
                        Status::Starting => {
                            if actual_status == Status::Running {
                                service.clear_pull_progress();
                                *status_lock = Status::Running;
                            }
                        }
                        Status::Stopping => {
                            if actual_status == Status::Stopped
                                && DockerClient::all_containers_stopped(&service.name)
                            {
                                service.clear_pull_progress();
                                *status_lock = Status::Stopped;
                            }
                        }
                        _ => {
                            *status_lock = actual_status;
                        }
                    }
                }
            }
            self.first_status_check = false;
        }

        if self.docker_daemon_running && !self.event_listener_running {
            self.start_event_listeners();
        }
    }

    pub fn start_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !daemon::docker_service_active() {
                self.set_toast(
                    ToastState::Error,
                    "Cannot start service: Docker service not running",
                    5,
                );
                return;
            }
            if !self.docker_daemon_running {
                self.set_toast(
                    ToastState::Error,
                    "Cannot start service: Docker daemon not responding",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = DockerClient::get_status(&service_name);
            if current_status == Status::Running {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} already running", service_name),
                    4,
                );
                return;
            }

            let service = &mut self.services[i];

            if service.is_transitioning() {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} is busy, wait for operation to finish", service_name),
                    3,
                );
                return;
            }

            service.set_status(Status::Pulling);
            service.set_pull_progress(Some("queued".to_string()));
            append_project_event(&service.events, &service_name, "start requested");
            append_project_event(&service.events, &service_name, "pulling images");

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let events = Arc::clone(&service.events);
            let status = Arc::clone(&service.status);
            let pull_progress = Arc::clone(&service.pull_progress);
            let project = ComposeProject::new(service_name.clone());
            let service_name_for_status = service_name.clone();

            thread::spawn(move || {
                {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.clear();
                }

                let skip_pull = compose_images::all_images_cached(&service_name);
                if skip_pull {
                    let mut logs_lock = logs.lock().unwrap();
                    logs_lock.push_str("All images already present, skipping pull.\n");
                    *pull_progress.lock().unwrap() = Some("cached".to_string());
                    append_project_event(&events, &service_name, "pull cached");
                }

                let pull_success = if skip_pull {
                    true
                } else {
                    let progress_callback = {
                        let pull_progress = Arc::clone(&pull_progress);
                        Arc::new(move |line: &str| {
                            if let Some(progress) = pull_progress::extract(line) {
                                *pull_progress.lock().unwrap() = Some(progress);
                            }
                        })
                    };

                    match run_stream_with_line_callback(
                        project.pull_cmd(),
                        Arc::clone(&logs),
                        Some("Pull output:\n"),
                        Some(progress_callback),
                    ) {
                        Ok(true) => true,
                        Ok(false) => {
                            append_project_event(
                                &events,
                                &service_name,
                                "pull failed: command exited with non-zero status",
                            );
                            false
                        }
                        Err(e) => {
                            let mut logs_lock = logs.lock().unwrap();
                            logs_lock.push_str(&format!("Pull failed: {}\n", e));
                            append_project_event(
                                &events,
                                &service_name,
                                &format!("pull failed: {}", e),
                            );
                            false
                        }
                    }
                };

                if !pull_success {
                    *pull_progress.lock().unwrap() = None;
                    *status.lock().unwrap() = Status::Error;
                    append_project_event(&events, &service_name, "error");
                    return;
                }

                *pull_progress.lock().unwrap() = None;
                *status.lock().unwrap() = Status::Starting;
                append_project_event(&events, &service_name, "pull complete");
                append_project_event(&events, &service_name, "up requested");

                match run_stream(
                    project.up_detached_cmd(),
                    Arc::clone(&logs),
                    Some("Up output:\n"),
                ) {
                    Ok(true) => {
                        let actual_status = DockerClient::get_status(&service_name_for_status);
                        if actual_status == Status::Running {
                            *status.lock().unwrap() = Status::Running;
                            append_project_event(&events, &service_name, "running confirmed");
                            append_project_runtime_details(&events, &service_name);
                        } else {
                            *status.lock().unwrap() = Status::Error;
                            append_project_event(
                                &events,
                                &service_name,
                                "up completed but service not running",
                            );
                        }
                    }
                    Ok(false) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str("Up failed: command exited with non-zero status\n");
                        *status.lock().unwrap() = Status::Error;
                        append_project_event(
                            &events,
                            &service_name,
                            "up failed: command exited with non-zero status",
                        );
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Up failed: {}\n", e));
                        *status.lock().unwrap() = Status::Error;
                        append_project_event(&events, &service_name, &format!("up failed: {}", e));
                    }
                }
            });

            self.set_toast(
                ToastState::Success,
                format!("Starting {}", service_name_for_toast),
                3,
            );
        }
    }

    pub fn stop_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !daemon::docker_service_active() {
                self.set_toast(
                    ToastState::Error,
                    "Cannot stop service: Docker service not running",
                    5,
                );
                return;
            }
            if !self.docker_daemon_running {
                self.set_toast(
                    ToastState::Error,
                    "Cannot stop service: Docker daemon not responding",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = DockerClient::get_status(&service_name);
            if current_status != Status::Running {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} not running", service_name),
                    4,
                );
                return;
            }

            let service = &mut self.services[i];

            if service.is_transitioning() {
                self.set_toast(
                    ToastState::Warning,
                    format!("{} is busy, wait for operation to finish", service_name),
                    3,
                );
                return;
            }

            service.set_status(Status::Stopping);
            service.clear_pull_progress();
            append_project_event(&service.events, &service_name, "stop requested");

            service.live_logs.lock().unwrap().clear();
            if let Some(mut child) = service.logs_child.lock().unwrap().take() {
                let _ = child.kill();
            }

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let events = Arc::clone(&service.events);
            let status = Arc::clone(&service.status);
            let project = ComposeProject::new(service_name.clone());

            thread::spawn(move || {
                match run_stream(
                    project.down_cmd(),
                    Arc::clone(&logs),
                    Some("Down output:\n"),
                ) {
                    Ok(true) => {
                        *status.lock().unwrap() = Status::Stopped;
                        append_project_event(&events, &service_name, "stopped confirmed");
                    }
                    Ok(false) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str("Down failed: command exited with non-zero status\n");
                        *status.lock().unwrap() = Status::Error;
                        append_project_event(
                            &events,
                            &service_name,
                            "down failed: command exited with non-zero status",
                        );
                    }
                    Err(e) => {
                        let mut logs_lock = logs.lock().unwrap();
                        logs_lock.push_str(&format!("Down failed: {}\n", e));
                        *status.lock().unwrap() = Status::Error;
                        append_project_event(
                            &events,
                            &service_name,
                            &format!("down failed: {}", e),
                        );
                    }
                }
            });

            self.set_toast(
                ToastState::Success,
                format!("Stopping {}", service_name_for_toast),
                3,
            );
        }
    }

    pub fn toggle_service(&mut self) {
        if let Some(i) = self.state.selected() {
            let service = &self.services[i];
            if service.status() == Status::Running {
                self.stop_service();
            } else {
                self.start_service();
            }
        }
    }
}
