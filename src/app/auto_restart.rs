use crate::app::state::App;
use crate::status::ToastState;
use crate::systemd;

impl App {
    pub fn toggle_selected_auto_restart(&mut self) {
        let Some(index) = self.state.selected() else {
            return;
        };
        let Some(service) = self.services.get(index) else {
            return;
        };
        let service_name = service.name.clone();
        let enabled = self.auto_restart.toggle(&service_name);

        if let Err(error) = self.auto_restart.save(&self.project_root) {
            self.auto_restart.toggle(&service_name);
            self.set_toast(
                ToastState::Error,
                format!("Failed to save auto-restart: {error}"),
                5,
            );
            return;
        }

        if enabled {
            if systemd::is_auto_restart_unit_installed() {
                self.set_toast(
                    ToastState::Success,
                    format!("Auto-restart enabled for {service_name}"),
                    4,
                );
            } else {
                self.set_toast(
                    ToastState::Warning,
                    format!("Enabled for {service_name}; install the boot unit once"),
                    5,
                );
            }
        } else {
            self.set_toast(
                ToastState::Info,
                format!("Auto-restart disabled for {service_name}"),
                4,
            );
        }
    }
}
