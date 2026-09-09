use std::fs;

use crate::podman::client::PodmanClient;
use crate::podman::compose::ComposeProject;

pub fn all_images_cached(service_name: &str) -> bool {
    let compose_path = ComposeProject::new(service_name).compose_file();
    let Ok(content) = fs::read_to_string(compose_path) else {
        return false;
    };
    let Ok(compose) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return false;
    };
    let Some(services) = compose
        .get("services")
        .and_then(|services| services.as_mapping())
    else {
        return false;
    };

    services.values().all(|service_def| {
        service_def
            .get("image")
            .and_then(|image| image.as_str())
            .map(PodmanClient::image_exists)
            .unwrap_or(true)
    })
}
