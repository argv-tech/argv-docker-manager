use crate::app::App;
use crate::status::ToastState;

impl App {
    pub fn save_settings(&mut self) {
        let path = self.project_root.join("config.toml");
        if let Err(error) = self.settings.save(&path) {
            self.set_toast(
                ToastState::Error,
                format!("Could not save settings: {error}"),
                5,
            );
            return;
        }

        let changed = self.runtime != self.settings.general.runtime;
        self.runtime = self.settings.general.runtime;
        self.settings_mode = false;
        if changed {
            self.kill_all_live_logs();
            self.stop_event_listeners();
            self.event_listener_running = false;
            self.first_status_check = true;
            self.docker_daemon_running = false;
            self.refresh_statuses();
            self.start_event_listeners();
            self.set_toast(
                ToastState::Success,
                format!("Runtime changed to {}", self.runtime.name()),
                3,
            );
        } else {
            self.set_toast(ToastState::Success, "Settings saved", 3);
        }
    }
}
