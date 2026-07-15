use std::collections::HashMap;
use std::process::Command;

use crate::status::Status;

pub struct DockerClient;

impl DockerClient {
    pub fn docker_info_ok() -> bool {
        Command::new("docker")
            .arg("info")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn docker_cli_ok() -> bool {
        Command::new("docker")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn compose_cli_ok() -> bool {
        Command::new("docker")
            .arg("compose")
            .arg("version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn image_exists(image: &str) -> bool {
        Command::new("docker")
            .arg("image")
            .arg("inspect")
            .arg(image)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn get_status(project: &str) -> Status {
        match Command::new("docker")
            .arg("ps")
            .arg("--filter")
            .arg(format!("label=com.docker.compose.project={}", project))
            .arg("--format")
            .arg("{{.Names}}\t{{.Status}}")
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.trim().is_empty() {
                    Status::Stopped
                } else {
                    let has_running = stdout.lines().any(|line| {
                        line.split('\t')
                            .nth(1)
                            .map(|status| status.starts_with("Up"))
                            .unwrap_or(false)
                    });
                    if has_running {
                        Status::Running
                    } else {
                        Status::Stopped
                    }
                }
            }
            Err(_) => Status::Error,
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

        let cmd = Command::new("docker")
            .arg("ps")
            .arg("-a")
            .arg("--format")
            .arg("{{.Status}}\t{{.Label \"com.docker.compose.project\"}}")
            .output();

        match cmd {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                for line in stdout.lines() {
                    apply_batch_status_line(&mut statuses, line);
                }
            }
            Err(_) => {
                for status in statuses.values_mut() {
                    *status = Status::Error;
                }
            }
        }

        statuses
    }
}

fn apply_batch_status_line(statuses: &mut HashMap<String, Status>, line: &str) {
    let mut parts = line.splitn(2, '\t');
    let status_str = parts.next().unwrap_or_default();
    let project_name = parts.next().unwrap_or_default();

    if status_str.starts_with("Up")
        && let Some(status) = statuses.get_mut(project_name)
        && *status != Status::Error
    {
        *status = Status::Running;
    }
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
        apply_batch_status_line(&mut statuses, "Exited (0)\tredis");
        apply_batch_status_line(&mut statuses, "Up 3 seconds\tredis");

        assert_eq!(statuses.get("redis"), Some(&Status::Running));
    }

    #[test]
    fn batch_status_parser_ignores_unknown_projects() {
        let mut statuses = HashMap::from([("redis".to_string(), Status::Stopped)]);
        apply_batch_status_line(&mut statuses, "Up 3 seconds\tpostgres");

        assert_eq!(statuses.get("redis"), Some(&Status::Stopped));
    }
}
