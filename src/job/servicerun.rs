use std::{collections::HashMap, fmt::{Debug, Display, Formatter}};

use anyhow::Error;
use bollard::Docker;
use croner::Cron;
use tracing::warn;

use crate::{job::common::UNKNOWN_CONTAINER_LABEL, require_one, take_one};

use super::common::{schedule_to_cron, ExecInfo};

#[derive(Clone)]
pub struct ServiceRunJobInfo {
    pub name: String,
    pub schedule: Cron,
    pub command: String,
    pub image: Option<String>,
    pub user: Option<String>,
    pub network: Option<Vec<String>>,
    pub delete: bool,
    pub container: Option<String>,
    pub tty: bool,
}

impl ServiceRunJobInfo {
    pub const LABEL: &'static str = "job-service-run";
    pub async fn exec(self, _handle: &Docker) -> Result<ExecInfo, Error> {
        Err(Error::msg("message")) // TODO
    }
    pub fn get_schedule(&self) -> Cron {
        self.schedule.clone()
    }
    pub fn may_run_parallel(&self) -> bool {
        true
    }
}

impl TryFrom<HashMap<String, Vec<String>>> for ServiceRunJobInfo {
    type Error = Error;

    fn try_from(mut value: HashMap<String, Vec<String>>) -> Result<Self, Self::Error> {
        let job = ServiceRunJobInfo {
            name: require_one!(value, "name").unwrap_or_else(|_| "".to_string()),
            schedule: schedule_to_cron(&require_one!(value, "schedule")?.as_str())?,
            command: require_one!(value, "command")?,
            image: take_one!(value, "image")?,
            user: take_one!(value, "user")?,
            network: value.remove("network"),
            delete: take_one!(value, "delete")?.map_or(Ok(true), |t| t.parse().map_err(|e| Error::new(e)))?,
            container: take_one!(value, "container")?,
            tty: take_one!(value, "tty")?.map_or(Ok(false), |t| t.parse().map_err(|e| Error::new(e)))?,
        };
        if !value.is_empty() {
            warn!("The job key map has excess attributes that will not be used: {:?}", value.keys());
        }
        Ok(job)
    }
}

impl Display for ServiceRunJobInfo {
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

impl Debug for ServiceRunJobInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRunJobInfo")
            .field("name", &self.name)
            .field("schedule", &self.schedule.pattern.to_string())
            .field("command", &self.command)
            .field("image", &self.image)
            .field("user", &self.user)
            .field("network", &self.network)
            .field("delete", &self.delete)
            .field("container", &self.container)
            .field("tty", &self.tty).finish()
    }
}
