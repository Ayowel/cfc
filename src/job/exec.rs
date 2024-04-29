use std::fmt::{Debug, Display, Formatter};

use anyhow::Error;
use croner::Cron;

/// Execute an arbitrary command on a container.
/// This is normally instanciated as the value of the enum obtained by calling
/// [JobInfo::try_from][`crate::job::JobInfo::try_from`] with a `kind` key set
/// to [`ExecJobInfo::LABEL`].
/// 
/// The container must be started when the command is executed
/// or it will fail.
/// 
/// ## Examples
/// 
/// ```rust,no_run
/// use cfc::job::ExecJobInfo;
/// 
/// #[tokio::main(flavor = "current_thread")]
/// async fn main() {
///     let mut job = ExecJobInfo::default();
///     // The job's name, command, and container should be 
///     job.name = "Demo job".to_string();
///     job.command = "echo 3".to_string();
///     job.container = "democontainer".to_string();
/// 
///     if let Ok(Some(_)) = job.exec().await {
///         println!("Success!");
///     }
/// }
/// ```
#[derive(Clone)]
pub struct ExecJobInfo {
    /// The display name of the job
    pub name: String,
    /// The cron schedule for the job's execution
    pub schedule: Cron,
    /// The command that will be executed
    pub command: String,
    /// The target container's ID
    pub container: String,
    /// The user used to execute the command
    pub user: Option<String>,
    /// Whether a tty should be provisionned for the command's execution
    pub tty: bool,
    /// The additional environment variables to set when executing the command
    pub environment: Vec<String>,
}

impl ExecJobInfo {
    pub const LABEL: &'static str = "job-exec";
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

impl Default for ExecJobInfo {
    fn default() -> Self {
        Self {
            name: Default::default(),
            schedule: Cron::new("@hourly").parse().unwrap(),
            command: Default::default(),
            container: Default::default(),
            user: None,
            tty: false,
            environment: Default::default(),
        }
    }
}

impl Display for ExecJobInfo {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            fmt,
            "{}.{}.{}",
            Self::LABEL,
            self.name,
            self.container,
        )
    }
}

impl Debug for ExecJobInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecJobInfo")
            .field("name", &self.name)
            .field("schedule", &self.schedule.pattern.to_string())
            .field("command", &self.command)
            .field("container", &self.container)
            .field("user", &self.user)
            .field("tty", &self.tty)
            .field("environment", &self.environment)
            .finish()
    }
}
