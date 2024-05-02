use std::fmt::{Debug, Display, Formatter};

use anyhow::Error;
use bollard::Docker;
use croner::Cron;
use tracing::{event, Level};

use super::common::{ExecInfo, ExecutionReport};

#[derive(Clone)]
pub struct LocalJobInfo {
    pub name: String,
    pub schedule: Cron,
    pub command: String,
    pub dir: Option<String>,
    pub environment: Vec<String>,
}

impl LocalJobInfo {
    pub const LABEL: &'static str = "job-local";
    pub async fn exec(self, _: &Docker) -> Result<ExecInfo, Error> {
        let mut command = tokio::process::Command::new(self.command);
        for e in self.environment {
            let mut env_info = e.split("=");
            if let Some(key) = env_info.next() {
                let value = env_info.collect::<Vec<_>>().join(".");
                command.env(key, value);
            } else {
                return Err(Error::msg(format!("Failed to parse environment variable '{}'", e)));
            }
        }
        if let Some(dir) = self.dir {
            command.current_dir(dir);
        }
        command.output().await
            .and_then(|o| {
                // TODO: move this to the caller and return an object enum to handle the distinction between timer and job
                if o.status.code().and_then(|c| Some(c != 0)).unwrap_or(true) {
                    event!(
                        Level::ERROR,
                        "Unexpected error code {} in local job '{}'. [{}] [{}]",
                        o.status.code().unwrap_or(10000),
                        self.name,
                        String::from_utf8(o.stdout).unwrap_or_else(|_| "FAILED_TO_PARSE_OUTPUT".to_string()),
                        String::from_utf8(o.stderr).unwrap_or_else(|_| "FAILED_TO_PARSE_OUTPUT".to_string()),
                    );
                } else {
                    event!(
                        Level::INFO,
                        "Local job '{}' ended successfully.",
                        self.name,
                    );
                    event!(
                        Level::DEBUG,
                        "Local job '{}' ended successfully ({}). [{}] [{}]",
                        self.name,
                        o.status.code().unwrap_or(10000),
                        String::from_utf8(o.stdout).unwrap_or_else(|_| "FAILED_TO_PARSE_OUTPUT".to_string()),
                        String::from_utf8(o.stderr).unwrap_or_else(|_| "FAILED_TO_PARSE_OUTPUT".to_string()),
                    );
                }
                let mut report = ExecutionReport::default();
                report.retval = o.status.code().unwrap().into();
                Ok(ExecInfo::Report(report))
            })
            .map_err(|e| Error::new(e))
    }
    pub fn get_schedule(&self) -> Cron {
        self.schedule.clone()
    }
    pub fn may_run_parallel(&self) -> bool {
        true
    }
}

impl Display for LocalJobInfo {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "{}.{}.{}",
            Self::LABEL,
            self.name,
            "CFC_HOST",
        )
    }
}

impl Debug for LocalJobInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalJobInfo")
            .field("name", &self.name)
            .field("schedule", &self.schedule.pattern.to_string())
            .field("command", &self.command)
            .field("dir", &self.dir)
            .field("environment", &self.environment)
            .finish()
    }
}
