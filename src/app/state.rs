use std::collections::VecDeque;
use std::path::PathBuf;

use ratatui::text::Text;

use crate::auto_restart::AutoRestartConfig;
use crate::config::Keybinds;
use crate::podman::events::EventListenerHandle;
use crate::service::Service;
use crate::status::ToastState;
use crate::toast::Toast;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Focus {
    #[default]
    Services,
    Logs,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum LogTab {
    #[default]
    Events,
    LiveLogs,
}

pub struct LogsRenderCacheEntry {
    pub service_index: Option<usize>,
    pub tab: LogTab,
    pub buffer_revision: u64,
    pub body: Text<'static>,
    pub body_line_count: u16,
}

pub struct LogsRenderCache {
    entries: VecDeque<LogsRenderCacheEntry>,
}

impl LogsRenderCache {
    const CAPACITY: usize = 8;

    pub fn entry(
        &self,
        service_index: Option<usize>,
        tab: LogTab,
    ) -> Option<&LogsRenderCacheEntry> {
        self.entries
            .iter()
            .find(|entry| entry.service_index == service_index && entry.tab == tab)
    }

    #[cfg(test)]
    pub fn entry_mut(
        &mut self,
        service_index: Option<usize>,
        tab: LogTab,
    ) -> Option<&mut LogsRenderCacheEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.service_index == service_index && entry.tab == tab)
    }

    pub fn insert(&mut self, entry: LogsRenderCacheEntry) {
        if let Some(position) = self.entries.iter().position(|cached| {
            cached.service_index == entry.service_index && cached.tab == entry.tab
        }) {
            self.entries.remove(position);
        }

        if self.entries.len() == Self::CAPACITY {
            self.entries.pop_front();
        }

        self.entries.push_back(entry);
    }
}

impl Default for LogsRenderCache {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(Self::CAPACITY),
        }
    }
}

pub struct App {
    pub state: ratatui::widgets::ListState,
    pub services: Vec<Service>,
    pub project_root: PathBuf,
    pub auto_restart: AutoRestartConfig,
    pub toast: Option<Toast>,
    pub toast_timer: u32,

    pub search_mode: bool,
    pub search_query: String,
    pub podman_available: bool,
    pub podman_command_available: bool,
    pub podman_compose_available: bool,
    pub focus: Focus,
    pub first_status_check: bool,
    pub log_scroll: u16,
    pub log_auto_scroll: bool,
    pub log_tab: LogTab,
    pub animation_tick: u64,
    pub status_refresh_cooldown_ticks: u8,
    pub runtime_probe_cooldown_ticks: u8,
    pub event_listener_running: bool,
    pub event_listener_handle: Option<EventListenerHandle>,
    pub toast_tick_accumulator: u8,
    pub live_log_retry_cooldown_ticks: u8,
    pub logs_render_cache: LogsRenderCache,
    pub keybinds: Keybinds,
}

impl App {
    pub fn next(&mut self) {
        if self.services.is_empty() {
            self.state.select(None);
            return;
        }
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
        if self.services.is_empty() {
            self.state.select(None);
            return;
        }
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
