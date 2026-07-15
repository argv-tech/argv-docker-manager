use crate::config::Keybinds;
use crate::docker::events::EventListenerHandle;
use crate::service::Service;
use crate::status::ToastState;
use crate::toast::Toast;
use ratatui::text::Text;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Focus {
    #[default]
    Services,
    Logs,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum LogTab {
    #[default]
    Events,
    LiveLogs,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum DaemonAction {
    #[default]
    Start,
    Stop,
    Restart,
}

pub struct LogsRenderCache {
    pub service_index: Option<usize>,
    pub tab: LogTab,
    pub buffer_revision: u64,
    pub body: Text<'static>,
    pub body_line_count: u16,
}

impl Default for LogsRenderCache {
    fn default() -> Self {
        Self {
            service_index: None,
            tab: LogTab::Events,
            buffer_revision: 0,
            body: Text::from(""),
            body_line_count: 0,
        }
    }
}

pub struct App {
    pub state: ratatui::widgets::ListState,
    pub services: Vec<Service>,
    pub toast: Option<Toast>,
    pub toast_timer: u32,

    pub search_mode: bool,
    pub search_query: String,
    pub docker_daemon_running: bool,
    pub docker_command_available: bool,
    pub docker_compose_available: bool,
    pub daemon_menu_mode: bool,
    pub daemon_action_selected: DaemonAction,
    pub daemon_start_mode: bool,
    pub password_input: String,
    pub focus: Focus,
    pub first_status_check: bool,
    pub log_scroll: u16,
    pub log_auto_scroll: bool,
    pub log_tab: LogTab,
    pub animation_tick: u64,
    pub status_refresh_cooldown_ticks: u8,
    pub daemon_probe_cooldown_ticks: u8,
    pub event_listener_running: bool,
    pub event_listener_handle: Option<EventListenerHandle>,
    pub toast_tick_accumulator: u8,
    pub logs_render_cache: LogsRenderCache,
    pub keybinds: Keybinds,
}

impl App {
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.services.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.log_auto_scroll = true;
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.services.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.log_auto_scroll = true;
    }

    pub fn set_toast(&mut self, state: ToastState, message: impl Into<String>, timer: u32) {
        self.toast = Some(Toast {
            state,
            message: message.into(),
        });
        self.toast_timer = timer;
    }
}
