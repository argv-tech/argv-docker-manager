use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use crate::config::Runtime;
use crate::docker::process::run_capture;

#[derive(Clone)]
pub struct ComposeProject {
    pub dir: PathBuf,
    pub runtime: Runtime,
}

impl ComposeProject {
    pub fn new(name: impl Into<String>, runtime: Runtime) -> Self {
        let name = name.into();
        let dir = PathBuf::from("containers").join(name);
        Self { dir, runtime }
    }

    pub fn at(project_root: &Path, name: &str, runtime: Runtime) -> Self {
        let dir = project_root.join("containers").join(name);
        Self { dir, runtime }
    }

    pub fn command(&self) -> Command {
        let mut cmd = self.runtime.command();
        cmd.arg("compose").current_dir(&self.dir);
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
        let mut cmd = self.command();
        cmd.arg("logs")
            .arg("-f")
            .arg("--tail=100")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        cmd.spawn()
    }
}
