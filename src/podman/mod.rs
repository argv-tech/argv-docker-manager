pub const PODMAN_COMMAND: &str = "podman";
pub const PODMAN_COMPOSE_COMMAND: &str = "podman-compose";
pub const COMPOSE_PROJECT_LABEL: &str = "io.podman.compose.project";
pub const LEGACY_COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";

pub mod client;
pub mod compose;
pub mod daemon;
pub mod events;
pub mod inspect;
pub mod process;
