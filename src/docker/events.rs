use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::status::Status;

const MAX_EVENT_LOG_SIZE: usize = 100 * 1024;

pub struct ProjectEventTargets {
    pub status: Arc<Mutex<Status>>,
    pub events: Arc<Mutex<String>>,
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
        seed_initial_events(&project_targets);

        loop {
            if shutdown_clone.load(Ordering::Relaxed) {
                break;
            }

            let mut cmd = std::process::Command::new("docker");
            cmd.arg("events")
                .arg("--filter")
                .arg("type=container")
                .arg("--filter")
                .arg("label=com.docker.compose.project")
                .arg("--format")
                .arg("{{.Action}}\t{{index .Actor.Attributes \"com.docker.compose.project\"}}\t{{index .Actor.Attributes \"name\"}}\t{{index .Actor.Attributes \"exitCode\"}}")
                .arg("--since")
                .arg("0")
                .arg("--until")
                .arg("2");

            match cmd.stdout(Stdio::piped()).spawn() {
                Ok(mut child) => {
                    if let Some(stdout) = child.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            if shutdown_clone.load(Ordering::Relaxed) {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            handle_event_line(&line, &project_targets);
                        }
                    }
                    let _ = child.wait();
                }
                Err(_) => {
                    // Docker events stream unavailable, retry shortly.
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

pub fn append_project_event(logs: &Arc<Mutex<String>>, project: &str, action: &str) {
    append_event_log(logs, project, "", action);
}

pub fn append_running_snapshot(logs: &Arc<Mutex<String>>, project: &str) {
    for container_name in list_project_containers(project) {
        append_event_log(logs, project, &container_name, "running (snapshot)");
        append_runtime_details(logs, &container_name);
    }
}

pub fn append_project_runtime_details(logs: &Arc<Mutex<String>>, project: &str) {
    for container_name in list_project_containers(project) {
        append_runtime_details(logs, &container_name);
    }
}

fn handle_event_line(line: &str, project_targets: &HashMap<String, ProjectEventTargets>) {
    let mut parts = line.splitn(4, '\t');
    let action = parts.next().unwrap_or("").trim();
    let project = normalize_template_value(parts.next().unwrap_or("").trim());
    let container_name = parts.next().unwrap_or("").trim();
    let exit_code = parts.next().unwrap_or("").trim();

    if action.is_empty() {
        return;
    }

    let project = if project.is_empty() {
        resolve_project_from_container(container_name).unwrap_or_default()
    } else {
        project
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
            "create" | "restart" | "unpause" => Some(Status::Starting),
            "start" => Some(Status::Running),
            "stop" | "destroy" | "pause" => Some(Status::Stopped),
            "die" | "kill" => {
                if currently_stopping || matches!(*status, Status::Stopped) || exit_code == "0" {
                    Some(Status::Stopped)
                } else {
                    Some(Status::Error)
                }
            }
            _ if action.starts_with("health_status: ") => {
                if action.ends_with("healthy") {
                    Some(Status::Running)
                } else if action.ends_with("unhealthy") {
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

    docker_inspect_field(
        container_name,
        "{{index .Config.Labels \"com.docker.compose.project\"}}",
    )
}

fn append_runtime_details(logs: &Arc<Mutex<String>>, container_name: &str) {
    if container_name.is_empty() {
        return;
    }

    let ips = docker_inspect_field(
        container_name,
        "{{range $k, $v := .NetworkSettings.Networks}}{{$k}}={{$v.IPAddress}} {{end}}",
    )
    .unwrap_or_else(|| "unknown".to_string());

    let ports = docker_inspect_field(
        container_name,
        "{{range $p, $v := .NetworkSettings.Ports}}{{$p}}={{if $v}}{{(index $v 0).HostIp}}:{{(index $v 0).HostPort}}{{else}}internal{{end}} {{end}}",
    )
    .unwrap_or_else(|| "none".to_string());

    let ips = normalize_runtime_value(&ips, "pending");
    let ports = normalize_runtime_value(&ports, "none");

    let mut logs_lock = logs.lock().unwrap();
    logs_lock.push_str(&format!(
        "[event] {} runtime ips=[{}] ports=[{}]\n",
        container_name, ips, ports
    ));
}

fn list_project_containers(project: &str) -> Vec<String> {
    let output = Command::new("docker")
        .arg("ps")
        .arg("--filter")
        .arg(format!("label=com.docker.compose.project={}", project))
        .arg("--format")
        .arg("{{.Names}}")
        .output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn docker_inspect_field(container_name: &str, template: &str) -> Option<String> {
    let output = Command::new("docker")
        .arg("inspect")
        .arg("--format")
        .arg(template)
        .arg(container_name)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(", ")
}

fn normalize_runtime_value(value: &str, fallback: &str) -> String {
    let normalized = normalize_whitespace(value);
    if normalized.is_empty()
        || normalized.eq_ignore_ascii_case("unknown")
        || normalized.eq_ignore_ascii_case("none")
        || normalized.eq_ignore_ascii_case("invalid, IP")
        || normalized.eq_ignore_ascii_case("invalid IP")
        || normalized.eq_ignore_ascii_case("<no, value>")
        || normalized.eq_ignore_ascii_case("<no value>")
    {
        fallback.to_string()
    } else {
        normalized
    }
}

fn append_event_log(logs: &Arc<Mutex<String>>, project: &str, container_name: &str, action: &str) {
    let mut logs_lock = logs.lock().unwrap();
    append_event_log_entry(&mut logs_lock, project, container_name, action);
}

fn append_event_log_entry(logs: &mut String, project: &str, container_name: &str, action: &str) {
    let scope = if container_name.is_empty() {
        project
    } else {
        container_name
    };
    let new_entry = format!("[event] {} {}\n", scope, action);

    if logs.len() + new_entry.len() > MAX_EVENT_LOG_SIZE {
        let truncate_point = logs.len().saturating_sub(MAX_EVENT_LOG_SIZE / 2);
        if truncate_point > 0
            && let Some(pos) = logs[truncate_point..].find('\n')
        {
            logs.drain(0..truncate_point + pos + 1);
        }
    }

    logs.push_str(&new_entry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_project_event_uses_project_scope() {
        let logs = Arc::new(Mutex::new(String::new()));

        append_project_event(&logs, "mailpit", "start requested");

        assert_eq!(
            logs.lock().unwrap().as_str(),
            "[event] mailpit start requested\n"
        );
    }

    #[test]
    fn append_event_log_entry_uses_container_scope_when_available() {
        let mut logs = String::new();

        append_event_log_entry(&mut logs, "mailpit", "mailpit-1", "start");

        assert_eq!(logs, "[event] mailpit-1 start\n");
    }

    #[test]
    fn append_event_log_entry_truncates_old_entries() {
        let mut logs = "old\n".repeat(MAX_EVENT_LOG_SIZE / 4 + 1_000);

        append_event_log_entry(&mut logs, "mailpit", "", "updated");

        assert!(logs.len() <= MAX_EVENT_LOG_SIZE);
        assert!(logs.ends_with("[event] mailpit updated\n"));
    }

    #[test]
    fn handle_event_line_updates_matching_project_status() {
        let events = Arc::new(Mutex::new(String::new()));
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

        handle_event_line("stop\tmailpit\tmailpit-1\t0", &targets);

        assert!(matches!(*status.lock().unwrap(), Status::Stopped));
        assert!(pull_progress.lock().unwrap().is_none());
        assert_eq!(events.lock().unwrap().as_str(), "[event] mailpit-1 stop\n");
    }
}
