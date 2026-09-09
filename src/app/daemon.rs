use crate::app::state::{App, DaemonAction};
use crate::podman::compose::ComposeProject;
use crate::podman::daemon;
use crate::podman::process::run_capture;
use crate::status::{Status, ToastState};

impl App {
    fn refresh_statuses_now(&mut self) {
        self.daemon_probe_cooldown_ticks = 0;
        self.refresh_statuses();
    }

    fn complete_daemon_action(&mut self) {
        self.daemon_start_mode = false;
    }

    pub fn start_daemon(&mut self) {
        match daemon::start() {
            Ok(()) => {
                self.set_toast(ToastState::Success, "Podman API socket started", 3);
                self.refresh_statuses_now();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.complete_daemon_action();
    }

    pub fn stop_all_services(&mut self) -> Result<usize, String> {
        let mut services_to_stop: Vec<String> = self
            .services
            .iter()
            .filter(|s| {
                matches!(
                    s.status(),
                    Status::Running | Status::Starting | Status::Stopping | Status::Pulling
                )
            })
            .map(|s| s.name.clone())
            .collect();

        if services_to_stop.is_empty() {
            return Ok(0);
        }

        let total = services_to_stop.len();
        services_to_stop.sort();

        for service_name in services_to_stop {
            let project = ComposeProject::new(service_name.clone());
            let cmd = project.down_cmd();
            match run_capture(cmd) {
                Ok(out) => {
                    if !out.status.success() {
                        return Err(format!("Failed to stop service {}", service_name));
                    }
                }
                Err(e) => {
                    return Err(format!("Error stopping service {}: {}", service_name, e));
                }
            }
        }

        self.refresh_statuses_now();
        Ok(total)
    }

    pub fn restart_daemon(&mut self) {
        match self.stop_all_services() {
            Ok(count) if count > 0 => {
                self.set_toast(
                    ToastState::Info,
                    format!("Stopped {} service(s) before restart", count),
                    2,
                );
            }
            Ok(_) => {}
            Err(e) => {
                self.set_toast(
                    ToastState::Error,
                    format!("Failed to stop services: {}", e),
                    5,
                );
                self.complete_daemon_action();
                return;
            }
        }

        match daemon::restart() {
            Ok(()) => {
                self.set_toast(
                    ToastState::Success,
                    "Podman API socket restarted (services stopped first)",
                    4,
                );
                self.refresh_statuses_now();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.complete_daemon_action();
    }

    pub fn stop_daemon(&mut self) {
        match self.stop_all_services() {
            Ok(count) if count > 0 => {
                self.set_toast(
                    ToastState::Info,
                    format!("Stopped {} service(s) before stopping daemon", count),
                    2,
                );
            }
            Ok(_) => {}
            Err(e) => {
                self.set_toast(
                    ToastState::Error,
                    format!("Failed to stop services: {}", e),
                    5,
                );
                self.complete_daemon_action();
                return;
            }
        }

        match daemon::stop() {
            Ok(()) => {
                self.set_toast(
                    ToastState::Success,
                    "Podman API socket stopped (services stopped first)",
                    4,
                );
                self.refresh_statuses_now();
            }
            Err(error_msg) => {
                self.set_toast(ToastState::Error, error_msg, 5);
            }
        }

        self.complete_daemon_action();
    }

    pub fn execute_daemon_action(&mut self) {
        match self.daemon_action_selected {
            DaemonAction::Start => self.start_daemon(),
            DaemonAction::Stop => self.stop_daemon(),
            DaemonAction::Restart => self.restart_daemon(),
        }
    }
}
