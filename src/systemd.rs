use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

const SYSTEMD_UNIT_DIR: &str = "/etc/systemd/system";
const UNIT_NAME: &str = "argv-docker-manager-autorestart.service";

pub fn install_auto_restart_unit(project_root: &Path) -> Result<String> {
    let project_root = project_root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", project_root.display()))?;
    let executable = std::env::current_exe()
        .context("failed to locate argv-docker-manager executable")?
        .canonicalize()
        .context("failed to resolve argv-docker-manager executable")?;
    let user = service_user()?;
    let unit_name = UNIT_NAME;
    let content = render_unit(&executable, &project_root, &user)?;
    let temporary_path = write_temporary_unit(unit_name, &content)?;
    let destination = Path::new(SYSTEMD_UNIT_DIR).join(unit_name);

    let install_result = run_privileged(
        "install",
        [
            OsStr::new("-m"),
            OsStr::new("0644"),
            temporary_path.as_os_str(),
            destination.as_os_str(),
        ],
    );
    let _ = fs::remove_file(&temporary_path);
    install_result?;

    run_privileged("systemctl", [OsStr::new("daemon-reload")])?;
    run_privileged("systemctl", [OsStr::new("enable"), OsStr::new(unit_name)])?;

    Ok(unit_name.to_string())
}

pub fn is_auto_restart_unit_installed() -> bool {
    Path::new(SYSTEMD_UNIT_DIR).join(UNIT_NAME).is_file()
}

pub fn remove_auto_restart_unit() -> Result<()> {
    let unit_path = Path::new(SYSTEMD_UNIT_DIR).join(UNIT_NAME);
    if !unit_path.is_file() {
        bail!("auto-restart unit is not installed");
    }

    run_privileged("systemctl", [OsStr::new("stop"), OsStr::new(UNIT_NAME)])?;
    run_privileged("systemctl", [OsStr::new("disable"), OsStr::new(UNIT_NAME)])?;
    run_privileged("rm", [unit_path.as_os_str()])?;
    run_privileged("systemctl", [OsStr::new("daemon-reload")])?;

    Ok(())
}

fn render_unit(executable: &Path, project_root: &Path, user: &str) -> Result<String> {
    let executable = escape_unit_value(executable)?;
    let project_root = escape_unit_value(project_root)?;
    validate_user(user)?;

    Ok(format!(
        "[Unit]\n\
         Description=Start ARGV Docker Manager selected projects\n\
         Requires=docker.service\n\
         After=docker.service network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         User={user}\n\
         WorkingDirectory={project_root}\n\
         ExecStart=\"{executable}\" --auto-restart \"{project_root}\"\n\
         RemainAfterExit=yes\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n"
    ))
}

fn escape_unit_value(path: &Path) -> Result<String> {
    let value = path
        .to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))?;
    if value.contains(['\n', '\r', '\0']) {
        bail!(
            "path contains characters unsupported by systemd: {}",
            path.display()
        );
    }

    Ok(value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%"))
}

fn service_user() -> Result<String> {
    let user = std::env::var("SUDO_USER")
        .ok()
        .filter(|user| user != "root")
        .or_else(|| std::env::var("USER").ok())
        .context("USER is not set")?;
    validate_user(&user)?;
    Ok(user)
}

fn validate_user(user: &str) -> Result<()> {
    if user.is_empty()
        || !user.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        bail!("invalid systemd service user: {user}");
    }
    Ok(())
}

fn write_temporary_unit(unit_name: &str, content: &str) -> Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{unit_name}.{}.{nonce}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    if let Err(error) = file.write_all(content.as_bytes()) {
        let _ = fs::remove_file(&path);
        return Err(error).with_context(|| format!("failed to write {}", path.display()));
    }
    Ok(path)
}

fn run_privileged<I, S>(program: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = if running_as_root() {
        Command::new(program)
    } else {
        let mut command = Command::new("sudo");
        command.arg(program);
        command
    };
    let status = command
        .args(args)
        .status()
        .with_context(|| format!("failed to run privileged {program}"))?;
    if !status.success() {
        bail!("privileged {program} exited with {status}");
    }
    Ok(())
}

fn running_as_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|output| output.status.success() && output.stdout == b"0\n")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_unit_uses_dynamic_absolute_paths() {
        let unit = render_unit(
            Path::new("/opt/my tools/argv-docker-manager"),
            Path::new("/srv/compose clone"),
            "alice",
        )
        .unwrap();

        assert_eq!(
            unit,
            "[Unit]\n\
             Description=Start ARGV Docker Manager selected projects\n\
             Requires=docker.service\n\
             After=docker.service network-online.target\n\
             Wants=network-online.target\n\
             \n\
             [Service]\n\
             Type=oneshot\n\
             User=alice\n\
             WorkingDirectory=/srv/compose clone\n\
             ExecStart=\"/opt/my tools/argv-docker-manager\" --auto-restart \"/srv/compose clone\"\n\
             RemainAfterExit=yes\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n"
        );
    }

    #[test]
    fn generated_unit_escapes_systemd_specifiers() {
        let unit = render_unit(
            Path::new("/opt/100%/argv-docker-manager"),
            Path::new("/srv/compose"),
            "alice",
        )
        .unwrap();

        assert!(unit.contains("ExecStart=\"/opt/100%%/argv-docker-manager\""));
    }
}
