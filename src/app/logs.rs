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
        // Only the selected live-log project needs a follower; avoid scanning every service per frame.
        if !self.podman_available {
            if let Some(index) = self.live_log_service_index.take() {
                self.stop_live_logs_for_service(index);
            }
            return;
        }

        let selected_index = self.state.selected();
        let target_index = selected_index.filter(|&index| {
            self.log_tab == crate::app::LogTab::LiveLogs
                && self.services[index].status() == Status::Running
        });

        let listener_tracked =
            target_index.is_some() && self.live_log_service_index == target_index;
        let listener_is_active = target_index.is_some_and(|index| {
            self.live_log_service_index == Some(index)
                && self.services[index].live_logs_process_active()
        });
        if listener_is_active {
            self.live_log_retry_cooldown_ticks = 0;
            return;
        }
        if target_index.is_none() && self.live_log_service_index.is_none() {
            self.live_log_retry_cooldown_ticks = 0;
            return;
        }
        if listener_tracked && self.live_log_retry_cooldown_ticks > 0 {
            self.live_log_retry_cooldown_ticks =
                self.live_log_retry_cooldown_ticks.saturating_sub(1);
            return;
        }
        if listener_tracked {
            self.live_log_retry_cooldown_ticks = Self::LIVE_LOG_RETRY_COOLDOWN_TICKS;
        } else {
            self.live_log_retry_cooldown_ticks = 0;
        }

        if let Some(index) = self.live_log_service_index.take() {
            self.stop_live_logs_for_service(index);
        }

        if let Some(index) = target_index {
            if self.ensure_live_logs_for_service(index) {
                self.live_log_service_index = Some(index);
            } else {
                self.live_log_service_index = Some(index);
                self.live_log_retry_cooldown_ticks = Self::LIVE_LOG_RETRY_COOLDOWN_TICKS;
            }
        }
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

            let _ = child.kill();
            let _ = child.wait();
        }

        false
    }

    fn stop_live_logs_for_service(&self, index: usize) {
        let service = &self.services[index];
        service.reset_live_logs();
        if let Some(mut child) = service.logs_child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
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
