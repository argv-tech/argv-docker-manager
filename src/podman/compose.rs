use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use super::PODMAN_COMPOSE_COMMAND;
use crate::podman::process::run_capture;

pub const COMPOSE_FILE_NAME: &str = "compose.yml";
const COMPOSE_FILE_NAMES: [&str; 4] = [
    "compose.yml",
    "compose.yaml",
    "docker-compose.yml",
    "docker-compose.yaml",
];

#[derive(Clone)]
pub struct ComposeProject {
    pub dir: PathBuf,
}

impl ComposeProject {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let dir = PathBuf::from("containers").join(name);
        Self { dir }
    }

    pub fn at(project_root: &Path, name: &str) -> Self {
        let dir = project_root.join("containers").join(name);
        Self { dir }
    }

    pub fn compose_file(&self) -> PathBuf {
        COMPOSE_FILE_NAMES
            .iter()
            .map(|name| self.dir.join(name))
            .find(|path| path.is_file())
            .unwrap_or_else(|| self.dir.join(COMPOSE_FILE_NAME))
    }

    pub fn command(&self) -> Command {
        let mut cmd = Command::new(PODMAN_COMPOSE_COMMAND);
        cmd.current_dir(&self.dir);
        cmd
    }

    pub fn pull_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("pull");
        cmd
    }

    pub fn up_detached_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("up").arg("-d");
        cmd
    }

    pub fn down_cmd(&self) -> Command {
        let mut cmd = self.command();
        cmd.arg("down");
        cmd
    }

    pub fn ps_output(&self) -> std::io::Result<Output> {
        let mut cmd = self.command();
        cmd.arg("ps");
        run_capture(cmd)
    }

    pub fn logs_follow(&self) -> std::io::Result<Child> {
        self.logs_command().spawn()
    }

    fn logs_command(&self) -> Command {
        let mut cmd = self.command();
        // podman-compose formats logs with Python print calls; piped stdout is otherwise buffered.
        cmd.env("PYTHONUNBUFFERED", "1");
        cmd.arg("logs")
            .arg("-f")
            .arg("--tail=100")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn command_uses_the_direct_podman_compose_provider() {
        let project = ComposeProject::new("mysql");

        assert_eq!(
            project.command().get_program(),
            OsStr::new("podman-compose")
        );
    }

    #[test]
    fn live_logs_disable_python_output_buffering() {
        let project = ComposeProject::new("mysql");
        let command = project.logs_command();

        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == OsStr::new("PYTHONUNBUFFERED"))
                .and_then(|(_, value)| value),
            Some(OsStr::new("1"))
        );
    }
}
