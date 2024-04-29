use std::fmt::{Debug, Display, Formatter};

use anyhow::Error;
use croner::Cron;

use crate::job::common::UNKNOWN_CONTAINER_LABEL;

#[derive(Clone)]
pub struct RunJobInfo {
    pub name: String,
    pub schedule: Cron,
    pub command: String,
    pub image: Option<String>,
    pub user: Option<String>,
    pub network: Option<Vec<String>>,
    pub hostname: Option<String>,
    pub delete: bool,
    pub container: Option<String>,
    pub tty: bool,
    pub volume: Vec<String>,
    pub environment: Vec<String>,
}

impl RunJobInfo {
    pub const LABEL: &'static str = "job-run";
    pub async fn exec(self) -> Result<Option<bool>, Error> {
        Err(Error::msg("message")) // TODO
    }
    pub fn get_schedule(&self) -> Cron {
        self.schedule.clone()
    }
    pub fn may_run_parallel(&self) -> bool {
        true
    }
}

impl Display for RunJobInfo {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "{}.{}.{}",
            Self::LABEL,
            self.name,
            self.container.as_ref().or(self.image.as_ref()).map_or(UNKNOWN_CONTAINER_LABEL, |s| s.as_str())
        )
    }
}

impl Debug for RunJobInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunJobInfo")
            .field("name", &self.name)
            .field("schedule", &self.schedule.pattern.to_string())
            .field("command", &self.command)
            .field("image", &self.image)
            .field("user", &self.user)
            .field("network", &self.network)
            .field("hostname", &self.hostname)
            .field("delete", &self.delete)
            .field("container", &self.container)
            .field("tty", &self.tty)
            .field("volume", &self.volume)
            .field("environment", &self.environment)
            .finish()
    }
}
