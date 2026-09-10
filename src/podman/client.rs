use std::collections::HashMap;
use std::process::Command;

use super::{
    COMPOSE_PROJECT_LABEL, LEGACY_COMPOSE_PROJECT_LABEL, PODMAN_COMMAND, PODMAN_COMPOSE_COMMAND,
};
use crate::status::Status;

pub struct PodmanClient;

impl PodmanClient {
    pub fn podman_info_ok() -> bool {
        Command::new(PODMAN_COMMAND)
            .arg("info")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn podman_cli_ok() -> bool {
        Command::new(PODMAN_COMMAND)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn compose_cli_ok() -> bool {
        Command::new(PODMAN_COMPOSE_COMMAND)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn image_exists(image: &str) -> bool {
        Command::new(PODMAN_COMMAND)
            .arg("image")
            .arg("inspect")
            .arg(image)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn get_status(project: &str) -> Status {
        match Command::new(PODMAN_COMMAND)
            .arg("ps")
            .arg("--format")
            .arg(format_status_template())
            .output()
        {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.lines().any(|line| {
                    let (status, podman_project, legacy_project) = parse_status_line(line);
                    status.starts_with("Up")
                        && (podman_project == project || legacy_project == project)
                }) {
                    Status::Running
                } else {
                    Status::Stopped
                }
            }
            Ok(_) | Err(_) => Status::Error,
        }
    }

    pub fn get_batch_statuses<'a>(
        service_names: impl IntoIterator<Item = &'a str>,
    ) -> HashMap<String, Status> {
        let mut statuses: HashMap<String, Status> = service_names
            .into_iter()
            .map(|name| {
                let status = if validate_service_name(name) {
                    Status::Stopped
                } else {
                    Status::Error
                };
                (name.to_owned(), status)
            })
            .collect();

        if statuses.is_empty() {
            return statuses;
        }

        let cmd = Command::new(PODMAN_COMMAND)
            .arg("ps")
            .arg("-a")
            .arg("--format")
            .arg(format_status_template())
            .output();

        match cmd {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    apply_batch_status_line(&mut statuses, line);
                }
            }
            Ok(_) | Err(_) => {
                for status in statuses.values_mut() {
                    *status = Status::Error;
                }
            }
        }

        statuses
    }
}

fn apply_batch_status_line(statuses: &mut HashMap<String, Status>, line: &str) {
    let (status_str, podman_project, legacy_project) = parse_status_line(line);
    let project_name = if !podman_project.is_empty() {
        podman_project
    } else {
        legacy_project
    };

    if status_str.starts_with("Up")
        && let Some(status) = statuses.get_mut(project_name)
        && *status != Status::Error
    {
        *status = Status::Running;
    }
}

fn format_status_template() -> String {
    format!(
        "{{{{.Status}}}}\t{{{{.Label \"{COMPOSE_PROJECT_LABEL}\"}}}}\t{{{{.Label \"{LEGACY_COMPOSE_PROJECT_LABEL}\"}}}}"
    )
}

fn parse_status_line(line: &str) -> (&str, &str, &str) {
    let mut parts = line.splitn(3, '\t');
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}

fn validate_service_name(name: &str) -> bool {
    name.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_status_parser_marks_project_running_when_any_container_is_up() {
        let mut statuses = HashMap::from([
            ("redis".to_string(), Status::Stopped),
            ("mailpit".to_string(), Status::Stopped),
        ]);
        apply_batch_status_line(&mut statuses, "Exited (0)\t\tredis");
        apply_batch_status_line(&mut statuses, "Up 3 seconds\tredis\t");

        assert_eq!(statuses.get("redis"), Some(&Status::Running));
    }

    #[test]
    fn batch_status_parser_ignores_unknown_projects() {
        let mut statuses = HashMap::from([("redis".to_string(), Status::Stopped)]);
        apply_batch_status_line(&mut statuses, "Up 3 seconds\tpostgres\t");

        assert_eq!(statuses.get("redis"), Some(&Status::Stopped));
    }

    #[test]
    fn batch_status_parser_accepts_legacy_compose_project_labels() {
        let mut statuses = HashMap::from([("redis".to_string(), Status::Stopped)]);

        apply_batch_status_line(&mut statuses, "Up 3 seconds\t\tredis");

        assert_eq!(statuses.get("redis"), Some(&Status::Running));
    }
}
