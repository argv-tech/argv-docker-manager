use std::process::Command;

use super::{COMPOSE_PROJECT_LABEL, LEGACY_COMPOSE_PROJECT_LABEL, PODMAN_COMMAND};

const PROJECT_TEMPLATE: &str = concat!(
    "{{index .Config.Labels \"io.podman.compose.project\"}}\t",
    "{{index .Config.Labels \"com.docker.compose.project\"}}"
);
const RUNTIME_TEMPLATE: &str = concat!(
    "{{range $k, $v := .NetworkSettings.Networks}}",
    "{{$k}}={{$v.IPAddress}} {{end}}\t",
    "{{range $p, $v := .NetworkSettings.Ports}}",
    "{{$p}}={{if $v}}{{(index $v 0).HostIp}}:",
    "{{(index $v 0).HostPort}}{{else}}internal{{end}} {{end}}"
);

pub struct ContainerRuntimeDetails {
    pub ips: String,
    pub ports: String,
}

impl Default for ContainerRuntimeDetails {
    fn default() -> Self {
        Self {
            ips: "pending".to_string(),
            ports: "none".to_string(),
        }
    }
}

pub fn project_name(container_name: &str) -> Option<String> {
    inspect_value(container_name, PROJECT_TEMPLATE).and_then(|value| {
        value
            .split('\t')
            .map(str::trim)
            .find(|value| !value.is_empty() && *value != "<no value>")
            .map(ToOwned::to_owned)
    })
}

pub fn runtime_details(container_name: &str) -> ContainerRuntimeDetails {
    inspect_value(container_name, RUNTIME_TEMPLATE)
        .map(|value| parse_runtime_details(&value))
        .unwrap_or_default()
}

pub fn project_containers(project: &str) -> Vec<String> {
    let output = Command::new(PODMAN_COMMAND)
        .arg("ps")
        .arg("--filter")
        .arg(format!("label={COMPOSE_PROJECT_LABEL}={project}"))
        .arg("--format")
        .arg("{{.Names}}")
        .output();

    let podman_containers = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    };

    if !podman_containers.is_empty() {
        return podman_containers;
    }

    Command::new(PODMAN_COMMAND)
        .arg("ps")
        .arg("--filter")
        .arg(format!("label={LEGACY_COMPOSE_PROJECT_LABEL}={project}"))
        .arg("--format")
        .arg("{{.Names}}")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn inspect_value(container_name: &str, template: &str) -> Option<String> {
    let output = Command::new(PODMAN_COMMAND)
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

fn parse_runtime_details(value: &str) -> ContainerRuntimeDetails {
    let (ips, ports) = value.split_once('\t').unwrap_or((value, ""));
    ContainerRuntimeDetails {
        ips: normalize_runtime_value(ips, "pending"),
        ports: normalize_runtime_value(ports, "none"),
    }
}

fn normalize_runtime_value(value: &str, fallback: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for part in value.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push_str(", ");
        }
        normalized.push_str(part);
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_parser_normalizes_ips_and_ports_from_one_inspect_result() {
        let details = parse_runtime_details(
            "default=172.18.0.2 backend=172.19.0.2 \t80/tcp=0.0.0.0:8080 443/tcp=internal ",
        );

        assert_eq!(
            (details.ips.as_str(), details.ports.as_str()),
            (
                "default=172.18.0.2, backend=172.19.0.2",
                "80/tcp=0.0.0.0:8080, 443/tcp=internal"
            )
        );
    }

    #[test]
    fn runtime_parser_uses_fallbacks_for_missing_values() {
        let details = parse_runtime_details("unknown\tnone");

        assert_eq!(
            (details.ips.as_str(), details.ports.as_str()),
            ("pending", "none")
        );
    }
}
