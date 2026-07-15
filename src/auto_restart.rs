use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::docker::compose::ComposeProject;

const CONFIG_FILE: &str = ".argv-docker-manager-autorestart.toml";
const LEGACY_CONFIG_FILE: &str = ".docker-manager-autorestart.toml";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AutoRestartConfig {
    #[serde(default)]
    services: BTreeSet<String>,
}

impl AutoRestartConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = config_path(project_root);
        let (path, content) = match fs::read_to_string(&path) {
            Ok(content) => (path, content),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let legacy_path = legacy_config_path(project_root);
                match fs::read_to_string(&legacy_path) {
                    Ok(content) => (legacy_path, content),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(Self::default());
                    }
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("failed to read {}", legacy_path.display()));
                    }
                }
            }
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", path.display()));
            }
        };

        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = config_path(project_root);
        let temporary_path = temporary_config_path(project_root);
        let content =
            toml::to_string_pretty(self).context("failed to serialize auto-restart config")?;

        fs::write(&temporary_path, content)
            .with_context(|| format!("failed to write {}", temporary_path.display()))?;
        fs::rename(&temporary_path, &path)
            .with_context(|| format!("failed to replace {}", path.display()))
    }

    pub fn toggle(&mut self, service_name: &str) -> bool {
        if self.services.remove(service_name) {
            false
        } else {
            self.services.insert(service_name.to_owned());
            true
        }
    }

    pub fn contains(&self, service_name: &str) -> bool {
        self.services.contains(service_name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn services(&self) -> impl Iterator<Item = &str> {
        self.services.iter().map(String::as_str)
    }
}

pub fn start_configured_services(project_root: &Path) -> Result<()> {
    let project_root = project_root
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", project_root.display()))?;
    let config = AutoRestartConfig::load(&project_root)?;

    if config.is_empty() {
        eprintln!("no services configured for auto-restart");
        return Ok(());
    }

    eprintln!("waiting for Docker daemon...");
    wait_for_docker()?;

    let mut failures = Vec::new();

    for service_name in config.services() {
        eprintln!("starting {service_name}...");
        if let Err(error) = start_service(&project_root, service_name) {
            eprintln!("failed to start {service_name}: {error}");
            failures.push(error.to_string());
        }
    }

    if !failures.is_empty() {
        bail!("{}", failures.join("; "));
    }

    Ok(())
}

fn wait_for_docker() -> Result<()> {
    const MAX_ATTEMPTS: u32 = 30;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    for attempt in 1..=MAX_ATTEMPTS {
        let status = Command::new("docker")
            .args(["info"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() => {
                eprintln!("Docker daemon is ready");
                return Ok(());
            }
            _ => {
                if attempt < MAX_ATTEMPTS {
                    eprintln!(
                        "Docker not ready, retrying in {RETRY_DELAY:?} ({attempt}/{MAX_ATTEMPTS})"
                    );
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }

    bail!("Docker daemon did not become ready after {MAX_ATTEMPTS} attempts");
}

pub fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(CONFIG_FILE)
}

fn legacy_config_path(project_root: &Path) -> PathBuf {
    project_root.join(LEGACY_CONFIG_FILE)
}

fn temporary_config_path(project_root: &Path) -> PathBuf {
    project_root.join(format!("{CONFIG_FILE}.tmp"))
}

fn validate_service(project_root: &Path, service_name: &str) -> Result<()> {
    if service_name.is_empty()
        || !service_name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("invalid auto-restart service name: {service_name}");
    }

    let compose_path = project_root
        .join("containers")
        .join(service_name)
        .join("docker-compose.yml");
    if !compose_path.is_file() {
        bail!("compose file not found: {}", compose_path.display());
    }

    Ok(())
}

fn start_service(project_root: &Path, service_name: &str) -> Result<()> {
    validate_service(project_root, service_name)?;
    let child = ComposeProject::at(project_root, service_name)
        .up_detached_cmd()
        .spawn()
        .with_context(|| format!("failed to start compose project {service_name}"))?;

    eprintln!("spawned compose for {service_name} (pid {})", child.id());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_project(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("argv-docker-manager-{name}-{}", std::process::id()))
    }

    #[test]
    fn saved_selection_loads_in_sorted_order() {
        let project_root = temporary_project("config-round-trip");
        fs::create_dir_all(&project_root).unwrap();
        let mut config = AutoRestartConfig::default();
        config.toggle("redis");
        config.toggle("mysql");

        config.save(&project_root).unwrap();
        let loaded = AutoRestartConfig::load(&project_root).unwrap();
        let services: Vec<_> = loaded.services().collect();
        let _ = fs::remove_dir_all(&project_root);

        assert_eq!(services, ["mysql", "redis"]);
    }

    #[test]
    fn toggling_selected_service_twice_disables_it() {
        let mut config = AutoRestartConfig::default();
        config.toggle("redis");

        let enabled = config.toggle("redis");

        assert!(!enabled);
    }

    #[test]
    fn legacy_selection_is_loaded_when_new_config_is_missing() {
        let project_root = temporary_project("legacy-config");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(
            legacy_config_path(&project_root),
            "services = [\"redis\"]\n",
        )
        .unwrap();

        let loaded = AutoRestartConfig::load(&project_root).unwrap();
        let _ = fs::remove_dir_all(&project_root);

        assert!(loaded.contains("redis"));
    }

    #[test]
    fn validation_rejects_service_name_path_traversal() {
        let project_root = temporary_project("path-traversal");

        let error = validate_service(&project_root, "../redis").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("invalid auto-restart service name")
        );
    }
}
