use anyhow::{Error, Result};
use bollard::{Docker, API_DEFAULT_VERSION};
use tracing::error;

pub struct ApplicationContext {
    pub label_prefixes: Vec<String>,
    pub socket: Option<String>,
    pub unsafe_labels: bool,
    pub config_path: String,
}

impl Default for ApplicationContext {
    fn default() -> Self {
        ApplicationContext {
            label_prefixes: vec![],
            socket: None,
            unsafe_labels: false,
            config_path: "/etc/cfc.conf".to_string(),
        }
    }
}
impl ApplicationContext {
    pub fn get_handle(self: &Self) -> Result<Docker> {
        match self.socket.as_ref() {
            Some(path) => Docker::connect_with_socket(path, 120, API_DEFAULT_VERSION),
            None => Docker::connect_with_defaults(),
        }.map_err(|e| {
            error!("Failed to connect to Docker: {}", e);
            Error::new(e)
        })
    }
}
