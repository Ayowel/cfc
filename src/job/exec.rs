use std::fmt::{Debug, Display, Formatter};

use anyhow::Error;
use bollard::{exec::{CreateExecOptions, StartExecOptions, StartExecResults}, secret::ExecInspectResponse, Docker};
use croner::Cron;
use tracing::debug;

use crate::job::common::{ExecInfo, ExecutionReport};

impl ExecutionReport {
    pub fn ingest_exec_inspect(&mut self, result: &ExecInspectResponse) -> Result<(), Error> {
        if result.running.unwrap() {
            return Err(Error::msg("Called Exec Inspect ingest before the command's termination"));
        }
        self.retval = result.exit_code.unwrap();
        Ok(())
    }
}

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
    pub async fn exec(self, handle: &Docker) -> Result<ExecInfo, Error> {
        debug!("Executing job '{}' on container {} ({})", self.name, self.container, self.command);
        let opts = CreateExecOptions {
            tty: Some(self.tty),
            attach_stdin: Some(true),
            attach_stderr: Some(true),
            env: Some(self.environment),
            cmd: Some(shell_words::split(self.command.as_ref()).unwrap()),
            user: self.user,
            ..Default::default()
        };
        let create_result;
        match handle.create_exec(&self.container, opts).await {
            Ok(c) => create_result = c,
            Err(e) => return Err(e.into())
        }
        let opts = StartExecOptions {
            detach: false,
            tty: self.tty,
            output_capacity: None,
        };
        let ostream;
        match handle.start_exec(&create_result.id, Some(opts)).await {
            Ok(r) => match r {
                StartExecResults::Attached { output, input: _ } => {
                    ostream = output;
                },
                StartExecResults::Detached => panic!("Spawned a detached exec process, this should never happen."),
            },
            Err(e) => { return Err(e.into()); },
        };
        let mut report = ExecutionReport::default();
        if let Err(e) = report.exhaust_stream(ostream).await {
            return Err(e.into());
        }
        match handle.inspect_exec(&create_result.id).await {
            Ok(i) => {
                report.ingest_exec_inspect(&i)?;
                debug!("Exec finished with result {:?}", i);
            },
            Err(e) => return Err(e.into()),
        }
        Ok(ExecInfo::Report(report))
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
