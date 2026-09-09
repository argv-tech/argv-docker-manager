use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use crate::app::state::App;
use crate::podman::compose::ComposeProject;
use crate::status::Status;

#[derive(serde::Deserialize)]
struct Compose {
    services: std::collections::HashMap<String, serde_yaml::Value>,
}

impl App {
    const LIVE_LOG_RETRY_COOLDOWN_TICKS: u8 = 15;

    pub fn populate_initial_logs(&self) {
        if !self.podman_available {
            return;
        }
        for service in &self.services {
            let service_name = service.name.clone();
            let logs = Arc::clone(&service.logs);
            thread::spawn(move || {
                let project = ComposeProject::new(service_name.clone());
                if let Ok(output) = project.ps_output() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("Up") {
                        let compose_path = project.compose_file();
                        let mut text = String::new();
                        if let Ok(content) = fs::read_to_string(&compose_path)
                            && let Ok(compose) = serde_yaml::from_str::<Compose>(&content)
                        {
                            let services = compose.services.keys().cloned().collect::<Vec<_>>();
                            let network = format!("{}_default", service_name);
                            text = format!("Up output:\nNetwork {} Running\n", network);
                            for svc in services {
                                text.push_str(&format!("Container {} Running\n", svc));
                            }
                        }
                        let mut logs_lock = logs.lock().unwrap();
                        if logs_lock.is_empty() {
                            logs_lock.push_str(&text);
                        }
                    }
                }
            });
        }
    }

    pub fn sync_live_log_listener(&mut self) {
        if !self.podman_available {
            self.stop_live_logs_for_all_services();
            self.live_log_retry_cooldown_ticks = 0;
            return;
        }

        let should_start_listeners = self.log_tab == crate::app::LogTab::LiveLogs;
        let can_retry = self.live_log_retry_cooldown_ticks == 0;
        if !can_retry {
            self.live_log_retry_cooldown_ticks =
                self.live_log_retry_cooldown_ticks.saturating_sub(1);
        }

        let mut retry_needed = false;
        for index in 0..self.services.len() {
            if self.services[index].status() != Status::Running {
                if self.has_live_log_child(index) {
                    self.stop_live_logs_for_service(index);
                }
                continue;
            }

            if !should_start_listeners || !can_retry {
                continue;
            }

            if self.services[index].live_logs_process_active() {
                continue;
            }

            if self.has_live_log_child(index) {
                self.stop_live_logs_for_service(index);
            }

            if !self.ensure_live_logs_for_service(index) {
                retry_needed = true;
            }
        }

        if retry_needed {
            self.live_log_retry_cooldown_ticks = Self::LIVE_LOG_RETRY_COOLDOWN_TICKS;
        }
    }

    fn has_live_log_child(&self, index: usize) -> bool {
        self.services[index].logs_child.lock().unwrap().is_some()
    }

    fn ensure_live_logs_for_service(&self, index: usize) -> bool {
        let service = &self.services[index];
        if service.logs_child.lock().unwrap().is_some() {
            return true;
        }

        service.reset_live_logs();
        let generation = service.live_logs_generation.load(Ordering::Relaxed);
        let project = ComposeProject::new(service.name.clone());
        let live_logs = Arc::clone(&service.live_logs);
        let logs_child = Arc::clone(&service.logs_child);
        let live_logs_generation = Arc::clone(&service.live_logs_generation);

        if let Ok(mut child) = project.logs_follow() {
            if let Some(stdout) = child.stdout.take() {
                *logs_child.lock().unwrap() = Some(child);
                thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let mut logs = live_logs.lock().unwrap();
                        if live_logs_generation.load(Ordering::Relaxed) != generation {
                            break;
                        }
                        logs.push_line(&line);
                    }

                    if live_logs_generation.load(Ordering::Relaxed) == generation
                        && let Some(mut child) = logs_child.lock().unwrap().take()
                    {
                        let _ = child.wait();
                    }
                });
                return true;
            }

            ComposeProject::stop_logs_child(&mut child);
        }

        false
    }

    fn stop_live_logs_for_service(&self, index: usize) {
        let service = &self.services[index];
        service.reset_live_logs();
        if let Some(mut child) = service.logs_child.lock().unwrap().take() {
            ComposeProject::stop_logs_child(&mut child);
        }
    }

    fn stop_live_logs_for_all_services(&self) {
        for index in 0..self.services.len() {
            self.stop_live_logs_for_service(index);
        }
    }

    pub fn kill_all_live_logs(&self) {
        self.stop_live_logs_for_all_services();
    }
}
