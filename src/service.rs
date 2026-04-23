use crate::status::Status;
use std::sync::{Arc, Mutex};

const MAX_COMMAND_LOG_SIZE: usize = 512 * 1024;
const MAX_EVENT_LOG_SIZE: usize = 100 * 1024;
const MAX_LIVE_LOG_SIZE: usize = 256 * 1024;

pub type SharedLogBuffer = Arc<Mutex<LogBuffer>>;

#[derive(Debug)]
pub struct LogBuffer {
    text: String,
    max_bytes: usize,
    revision: u64,
}

impl LogBuffer {
    pub fn command() -> Self {
        Self::new(MAX_COMMAND_LOG_SIZE)
    }

    pub fn events() -> Self {
        Self::new(MAX_EVENT_LOG_SIZE)
    }

    pub fn live() -> Self {
        Self::new(MAX_LIVE_LOG_SIZE)
    }

    fn new(max_bytes: usize) -> Self {
        Self {
            text: String::new(),
            max_bytes,
            revision: 0,
        }
    }

    pub fn push_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text.push_str(text);
        self.truncate_front();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn push_line(&mut self, line: &str) {
        self.push_str(line);
        self.push_str("\n");
    }

    pub fn clear(&mut self) {
        if self.text.is_empty() {
            return;
        }
        self.text.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn snapshot(&self) -> String {
        self.as_str().to_owned()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn truncate_front(&mut self) {
        if self.text.len() <= self.max_bytes {
            return;
        }

        let target_start = self.text.len().saturating_sub(self.max_bytes / 2);
        let drain_end = self.text[target_start..]
            .find('\n')
            .map(|offset| target_start + offset + 1)
            .unwrap_or(target_start);

        if drain_end > 0 && self.text.is_char_boundary(drain_end) {
            self.text.drain(0..drain_end);
        }
    }
}

#[derive(Clone)]
pub struct Service {
    pub name: String,
    pub status: Arc<Mutex<Status>>,
    pub pull_progress: Arc<Mutex<Option<String>>>,
    pub events: SharedLogBuffer,
    pub logs: SharedLogBuffer,
    pub live_logs: SharedLogBuffer,
    pub logs_child: Arc<Mutex<Option<std::process::Child>>>,
}

impl Service {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: Arc::new(Mutex::new(Status::Stopped)),
            pull_progress: Arc::new(Mutex::new(None)),
            events: Arc::new(Mutex::new(LogBuffer::events())),
            logs: Arc::new(Mutex::new(LogBuffer::command())),
            live_logs: Arc::new(Mutex::new(LogBuffer::live())),
            logs_child: Arc::new(Mutex::new(None)),
        }
    }

    pub fn status(&self) -> Status {
        *self.status.lock().unwrap()
    }

    pub fn set_status(&self, status: Status) {
        *self.status.lock().unwrap() = status;
    }

    pub fn set_pull_progress(&self, progress: Option<String>) {
        *self.pull_progress.lock().unwrap() = progress;
    }

    pub fn clear_pull_progress(&self) {
        self.set_pull_progress(None);
    }

    pub fn is_transitioning(&self) -> bool {
        matches!(
            self.status(),
            Status::Pulling | Status::Starting | Status::Stopping
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_buffer_truncates_old_lines_when_limit_is_exceeded() {
        let mut buffer = LogBuffer::new(24);

        buffer.push_line("old one");
        buffer.push_line("old two");
        buffer.push_line("new three");
        buffer.push_line("new four");

        assert!(buffer.as_str().len() <= 24);
        assert!(buffer.as_str().contains("new four"));
        assert!(!buffer.as_str().contains("old one"));
    }

    #[test]
    fn log_buffer_revision_changes_when_content_changes() {
        let mut buffer = LogBuffer::new(24);
        let initial = buffer.revision();

        buffer.push_line("new line");
        let after_push = buffer.revision();
        buffer.clear();

        assert!(after_push > initial);
        assert!(buffer.revision() > after_push);
    }
}
