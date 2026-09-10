use std::sync::Arc;
use std::thread;

use crate::app::compose_images;
use crate::app::pull_progress;
use crate::app::state::App;
use crate::podman::client::PodmanClient;
use crate::podman::compose::ComposeProject;
use crate::podman::events::{append_project_event, append_project_runtime_details};
use crate::podman::process::run_stream_with_line_callback;
use crate::status::{Status, ToastState};

impl App {
    pub fn refresh_statuses(&mut self) {
        const RUNTIME_PROBE_COOLDOWN_TICKS: u8 = 60;

        let should_probe_runtime = self.first_status_check
            || self.runtime_probe_cooldown_ticks == 0
            || !self.podman_available;
        let runtime_available = if should_probe_runtime {
            self.runtime_probe_cooldown_ticks = RUNTIME_PROBE_COOLDOWN_TICKS;
            PodmanClient::podman_info_ok()
        } else {
            self.podman_available
        };
        self.podman_available = runtime_available;

        if !self.podman_available {
            self.stop_event_listeners();
            for service in &self.services {
                service.set_status(Status::RuntimeUnavailable);
                service.clear_pull_progress();
            }
        } else {
            let batch_statuses = PodmanClient::get_batch_statuses(
                self.services.iter().map(|service| service.name.as_str()),
            );

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
                            if actual_status == Status::Stopped {
                                service.clear_pull_progress();
                                *status_lock = Status::Stopped;
                            }
                        }
                        _ => {
                            if *status_lock != Status::Error || actual_status != Status::Stopped {
                                *status_lock = actual_status;
                            }
                        }
                    }
                }
            }
            self.first_status_check = false;
        }

        if self.podman_available && !self.event_listener_running {
            self.start_event_listeners();
        }
    }

    pub fn start_service(&mut self) {
        if let Some(i) = self.state.selected() {
            if !self.podman_available {
                self.set_toast(
                    ToastState::Error,
                    "Cannot start service: Podman runtime unavailable",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = PodmanClient::get_status(&service_name);
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
                        let events = Arc::clone(&events);
                        let service_name = service_name.clone();
                        Arc::new(move |line: &str| {
                            append_project_event(&events, &service_name, line);
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

                let output_events = Arc::clone(&events);
                let output_project = service_name.clone();
                match run_stream_with_line_callback(
                    project.up_detached_cmd(),
                    Arc::clone(&logs),
                    Some("Up output:\n"),
                    Some(Arc::new(move |line| {
                        append_project_event(&output_events, &output_project, line);
                    })),
                ) {
                    Ok(true) => {
                        let actual_status = PodmanClient::get_status(&service_name_for_status);
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
            if !self.podman_available {
                self.set_toast(
                    ToastState::Error,
                    "Cannot stop service: Podman runtime unavailable",
                    5,
                );
                return;
            }

            let service_name = self.services[i].name.clone();
            let current_status = PodmanClient::get_status(&service_name);
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

            service.reset_live_logs();
            if let Some(mut child) = service.logs_child.lock().unwrap().take() {
                ComposeProject::stop_logs_child(&mut child);
            }

            let service_name_for_toast = service_name.clone();
            let logs = Arc::clone(&service.logs);
            let events = Arc::clone(&service.events);
            let status = Arc::clone(&service.status);
            let project = ComposeProject::new(service_name.clone());

            thread::spawn(move || {
                let output_events = Arc::clone(&events);
                let output_project = service_name.clone();
                match run_stream_with_line_callback(
                    project.down_cmd(),
                    Arc::clone(&logs),
                    Some("Down output:\n"),
                    Some(Arc::new(move |line| {
                        append_project_event(&output_events, &output_project, line);
                    })),
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
