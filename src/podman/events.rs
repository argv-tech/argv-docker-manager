use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::{COMPOSE_PROJECT_LABEL, LEGACY_COMPOSE_PROJECT_LABEL, PODMAN_COMMAND};
use crate::podman::inspect;
use crate::service::{LogBuffer, SharedLogBuffer};
use crate::status::Status;

pub struct ProjectEventTargets {
    pub status: Arc<Mutex<Status>>,
    pub events: SharedLogBuffer,
    pub pull_progress: Arc<Mutex<Option<String>>>,
}

pub struct EventListenerHandle {
    shutdown: Arc<AtomicBool>,
}

impl EventListenerHandle {
    pub fn signal_shutdown(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn spawn_projects_listener(
    project_targets: HashMap<String, ProjectEventTargets>,
) -> EventListenerHandle {
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    thread::spawn(move || {
        let mut since = chrono::Utc::now();
        seed_initial_events(&project_targets);

        loop {
            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }

            let until = chrono::Utc::now();
            let mut cmd = std::process::Command::new(PODMAN_COMMAND);
            cmd.arg("events")
                .arg("--stream=false")
                .arg("--filter")
                .arg("type=container")
                .arg("--format")
                .arg(format_event_template())
                .arg("--since")
                .arg(since.to_rfc3339())
                .arg("--until")
                .arg(until.to_rfc3339());

            match cmd.stderr(Stdio::null()).output() {
                Ok(output) if output.status.success() => {
                    for line in String::from_utf8_lossy(&output.stdout).lines() {
                        if shutdown_clone.load(Ordering::Relaxed) {
                            break;
                        }
                        if let Some(event) = event_in_window(line, since, until) {
                            handle_event_line(event, &project_targets);
                        }
                    }
                    since = until;
                }
                Ok(_) | Err(_) => {
                    // Podman events stream unavailable, retry shortly.
                }
            }

            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }

            thread::sleep(Duration::from_millis(500));
        }
    });

    EventListenerHandle { shutdown }
}

fn seed_initial_events(project_targets: &HashMap<String, ProjectEventTargets>) {
    for (project, target) in project_targets {
        append_running_snapshot(&target.events, project);
    }
}

pub fn append_project_event(logs: &SharedLogBuffer, project: &str, action: &str) {
    append_event_log(logs, project, "", action);
}

pub fn append_running_snapshot(logs: &SharedLogBuffer, project: &str) {
    for container_name in inspect::project_containers(project) {
        append_event_log(logs, project, &container_name, "running (snapshot)");
        append_runtime_details(logs, &container_name);
    }
}

pub fn append_project_runtime_details(logs: &SharedLogBuffer, project: &str) {
    for container_name in inspect::project_containers(project) {
        append_runtime_details(logs, &container_name);
    }
}

fn handle_event_line(line: &str, project_targets: &HashMap<String, ProjectEventTargets>) {
    let mut parts = line.splitn(5, '\t');
    let action = parts.next().unwrap_or("").trim();
    let container_name = parts.next().unwrap_or("").trim();
    let exit_code = parts.next().unwrap_or("").trim();
    let podman_project = normalize_template_value(parts.next().unwrap_or("").trim());
    let legacy_project = normalize_template_value(parts.next().unwrap_or("").trim());

    if action.is_empty() {
        return;
    }

    let project = if !podman_project.is_empty() {
        podman_project
    } else if !legacy_project.is_empty() {
        legacy_project
    } else {
        resolve_project_from_container(container_name).unwrap_or_default()
    };

    if project.is_empty() {
        return;
    }

    if let Some(target) = project_targets.get(&project) {
        append_event_log(&target.events, &project, container_name, action);
        if matches!(action, "start" | "restart" | "unpause") {
            append_runtime_details(&target.events, container_name);
        }

        let mut status = target.status.lock().unwrap();
        let currently_stopping = matches!(*status, Status::Stopping);
        let next_status = match action {
            // Creation does not imply a successful start (e.g. port binding can fail).
            "create" => None,
            "restart" | "unpause" => Some(Status::Running),
            "start" => Some(Status::Running),
            "stop" | "destroy" | "pause" => Some(Status::Stopped),
            "die" | "died" | "exited" | "kill" => {
                if currently_stopping || matches!(*status, Status::Stopped) || exit_code == "0" {
                    Some(Status::Stopped)
                } else {
                    Some(Status::Error)
                }
            }
            "remove" => Some(Status::Stopped),
            _ if action.starts_with("health_status: ") => {
                if action == "health_status: healthy" {
                    Some(Status::Running)
                } else if action == "health_status: unhealthy" {
                    Some(Status::Error)
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(next_status) = next_status {
            if next_status != Status::Pulling {
                *target.pull_progress.lock().unwrap() = None;
            }

            if *status != Status::Pulling || matches!(next_status, Status::Running | Status::Error)
            {
                *status = next_status;
            }
        }
    }
}

fn format_event_template() -> String {
    format!(
        "{{{{.TimeNano}}}}\t{{{{.Status}}}}{{{{if eq .Status \"health_status\"}}}}: {{{{.HealthStatus}}}}{{{{end}}}}\t{{{{.Name}}}}\t{{{{.ContainerExitCode}}}}\t{{{{index .Attributes \"{COMPOSE_PROJECT_LABEL}\"}}}}\t{{{{index .Attributes \"{LEGACY_COMPOSE_PROJECT_LABEL}\"}}}}"
    )
}

fn event_in_window(
    line: &str,
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
) -> Option<&str> {
    let (timestamp, event) = line.split_once('\t')?;
    let timestamp = chrono::DateTime::from_timestamp_nanos(timestamp.parse().ok()?);
    // Adjacent polls share a boundary, but each event belongs to exactly one window.
    (timestamp > since && timestamp <= until).then_some(event)
}

fn normalize_template_value(value: &str) -> String {
    if value.is_empty() || value == "<no value>" {
        String::new()
    } else {
        value.to_string()
    }
}

fn resolve_project_from_container(container_name: &str) -> Option<String> {
    if container_name.is_empty() {
        return None;
    }

    inspect::project_name(container_name)
}

fn append_runtime_details(logs: &SharedLogBuffer, container_name: &str) {
    if container_name.is_empty() {
        return;
    }

    let details = inspect::runtime_details(container_name);

    let mut logs_lock = logs.lock().unwrap();
    logs_lock.push_str(&format!(
        "[event] {} runtime ips=[{}] ports=[{}]\n",
        container_name, details.ips, details.ports
    ));
}

fn append_event_log(logs: &SharedLogBuffer, project: &str, container_name: &str, action: &str) {
    let mut logs_lock = logs.lock().unwrap();
    append_event_log_entry(&mut logs_lock, project, container_name, action);
}

fn append_event_log_entry(logs: &mut LogBuffer, project: &str, container_name: &str, action: &str) {
    let scope = if container_name.is_empty() {
        project
    } else {
        container_name
    };
    let new_entry = format!("[event] {} {}\n", scope, action);

    logs.push_str(&new_entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polling_windows_reject_history_and_do_not_repeat_boundary_events() {
        let since = chrono::DateTime::from_timestamp_nanos(100);
        let until = chrono::DateTime::from_timestamp_nanos(200);
        assert_eq!(event_in_window("99\tcreate", since, until), None);
        assert_eq!(event_in_window("100\tcreate", since, until), None);
        assert_eq!(event_in_window("150\tstart", since, until), Some("start"));
        assert_eq!(event_in_window("200\tstop", since, until), Some("stop"));
        assert_eq!(event_in_window("201\tstop", since, until), None);
    }

    #[test]
    fn create_event_preserves_a_failed_start() {
        let status = Arc::new(Mutex::new(Status::Error));
        let targets = HashMap::from([(
            "mysql".to_string(),
            ProjectEventTargets {
                status: Arc::clone(&status),
                events: Arc::new(Mutex::new(LogBuffer::events())),
                pull_progress: Arc::new(Mutex::new(None)),
            },
        )]);
        handle_event_line("create\tmysql\t0\tmysql\t", &targets);
        assert_eq!(*status.lock().unwrap(), Status::Error);
    }

    #[test]
    fn unhealthy_event_is_an_error_not_healthy() {
        let status = Arc::new(Mutex::new(Status::Running));
        let targets = HashMap::from([(
            "mysql".to_string(),
            ProjectEventTargets {
                status: Arc::clone(&status),
                events: Arc::new(Mutex::new(LogBuffer::events())),
                pull_progress: Arc::new(Mutex::new(None)),
            },
        )]);
        handle_event_line("health_status: unhealthy\tmysql\t0\tmysql\t", &targets);
        assert_eq!(*status.lock().unwrap(), Status::Error);
    }

    #[test]
    fn append_project_event_uses_project_scope() {
        let logs = Arc::new(Mutex::new(LogBuffer::events()));

        append_project_event(&logs, "mailpit", "start requested");

        assert_eq!(
            logs.lock().unwrap().as_str(),
            "[event] mailpit start requested\n"
        );
    }

    #[test]
    fn append_event_log_entry_uses_container_scope_when_available() {
        let mut logs = LogBuffer::events();

        append_event_log_entry(&mut logs, "mailpit", "mailpit-1", "start");

        assert_eq!(logs.as_str(), "[event] mailpit-1 start\n");
    }

    #[test]
    fn append_event_log_entry_truncates_old_entries() {
        let mut logs = LogBuffer::events();
        logs.push_str(&"old\n".repeat(30_000));

        append_event_log_entry(&mut logs, "mailpit", "", "updated");

        assert!(logs.as_str().len() <= 100 * 1024);
        assert!(logs.as_str().ends_with("[event] mailpit updated\n"));
    }

    #[test]
    fn handle_event_line_updates_matching_project_status() {
        let events = Arc::new(Mutex::new(LogBuffer::events()));
        let status = Arc::new(Mutex::new(Status::Running));
        let pull_progress = Arc::new(Mutex::new(Some("queued".to_string())));
        let mut targets = HashMap::new();
        targets.insert(
            "mailpit".to_string(),
            ProjectEventTargets {
                status: Arc::clone(&status),
                events: Arc::clone(&events),
                pull_progress: Arc::clone(&pull_progress),
            },
        );

        handle_event_line("stop\tmailpit-1\t0\tmailpit\t", &targets);

        assert!(matches!(*status.lock().unwrap(), Status::Stopped));
        assert!(pull_progress.lock().unwrap().is_none());
        assert_eq!(events.lock().unwrap().as_str(), "[event] mailpit-1 stop\n");
    }
}
