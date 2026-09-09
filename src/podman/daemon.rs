use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn podman_service_active() -> bool {
    systemctl_command()
        .arg("is-active")
        .arg("podman.socket")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

pub fn start() -> Result<(), String> {
    run_systemctl("start", &["podman.socket"])?;
    ensure_daemon_state(true, "start")
}

pub fn stop() -> Result<(), String> {
    run_systemctl("stop", &["podman.socket"])?;
    ensure_daemon_state(false, "stop")
}

pub fn restart() -> Result<(), String> {
    run_systemctl("restart", &["podman.socket"])?;
    ensure_daemon_state(true, "restart")
}

fn run_systemctl(action: &str, units: &[&str]) -> Result<(), String> {
    let output = systemctl_command()
        .arg(action)
        .args(units)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("Failed to {} Podman service: {}", action, error))?;

    if output.status.success() {
        return Ok(());
    }

    let error_msg = String::from_utf8_lossy(&output.stderr);
    if error_msg.trim().is_empty() {
        Err(format!("Failed to {} Podman service", action))
    } else {
        Err(format!(
            "Failed to {} Podman service: {}",
            action,
            error_msg.trim()
        ))
    }
}

fn ensure_daemon_state(expected_active: bool, action: &str) -> Result<(), String> {
    const MAX_RETRIES: usize = 20;
    const RETRY_DELAY_MS: u64 = 100;

    for _ in 0..MAX_RETRIES {
        if podman_service_active() == expected_active {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
    }

    let expectation = if expected_active {
        "running"
    } else {
        "stopped"
    };
    Err(format!(
        "Podman service did not become {} after {}",
        expectation, action
    ))
}

fn systemctl_command() -> Command {
    let mut command = Command::new("systemctl");
    if !running_as_root() {
        command.arg("--user");
    }
    command
}

fn running_as_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|output| output.status.success() && output.stdout == b"0\n")
        .unwrap_or(false)
}
