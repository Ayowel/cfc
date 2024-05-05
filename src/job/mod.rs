//! Job representation
use anyhow::Error;
use bollard::Docker;
use croner::Cron;
use tokio::{task::JoinSet, time};
use tracing::{debug, error, info};
use std::{collections::HashMap, fmt::Debug, time::Duration};

mod common;
mod exec;
mod run;
mod local;
mod servicerun;

pub use common::ExecutionReport;
pub use exec::ExecJobInfo;
pub use run::RunJobInfo;
pub use local::LocalJobInfo;
pub use servicerun::ServiceRunJobInfo;

use crate::job::common::ExecutionSchedule;

pub use self::common::ExecInfo;

/// Sleep until the next occurence of the provided cron
async fn cron_sleep(cron: &Cron) -> Result<ExecInfo, Error> {
    let current_time = chrono::Local::now();
    let next_occurence = cron.find_next_occurrence(&current_time, false).unwrap();
    let sleep = (next_occurence - current_time).num_milliseconds();
    assert!(sleep >= 0);
    tokio::time::sleep(Duration::from_millis(sleep as u64)).await;
    Ok(ExecInfo::Schedule(ExecutionSchedule{}))
}

/// A job's information container that allows to start the corresponding cron.
/// 
/// When manipulating this enum, prefer using the provided proxy functions or use the
/// macro [match_all_jobs!][crate::job::match_all_jobs] to write similar processing steps.
/// 
/// ## Examples
/// 
/// ```rust
/// # use std::collections::HashMap;
/// # use cfc::job::JobInfo;
/// let job = JobInfo::try_from(HashMap::from([
///    ("kind".to_string(), vec!["job-local".to_string()]),
///    ("name".to_string(), vec!["example_job".to_string()]),
///    ("command".to_string(), vec!["echo 3".to_string()]),
///    ("schedule".to_string(), vec!["@hourly".to_string()]),
/// ])).unwrap();
/// match job {
///     JobInfo::LocalJob(l) => assert_eq!(l.name, "example_job"),
///     _ => panic!("The generated job does not have the expected type"),
/// }
/// ```
#[derive(Debug)]
pub enum JobInfo {
    ExecJob(Box<ExecJobInfo>),
    RunJob(Box<RunJobInfo>),
    LocalJob(Box<LocalJobInfo>),
    ServiceRunJob(Box<ServiceRunJobInfo>),
}

/// Perform a match on all JobInfo enum members and apply the same processing to all of them
/// 
/// ## Examples
/// 
/// ```rust
/// # use std::collections::HashMap;
/// # use cfc::job::{JobInfo, match_all_jobs};
/// let job = JobInfo::try_from(HashMap::from([
///     // ...
/// #    ("kind".to_string(), vec!["job-local".to_string()]),
/// #    ("name".to_string(), vec!["example_job".to_string()]),
/// #    ("command".to_string(), vec!["echo 3".to_string()]),
/// #    ("schedule".to_string(), vec!["@hourly".to_string()]),
/// ])).unwrap();
/// let name = match_all_jobs!(&job, e, &e.name);
/// # assert_eq!(name, "example_job");
/// ```
#[macro_export]
macro_rules! match_all_jobs {
    ($target: expr, $varname: ident, $processing: expr) => {
        match $target {
            JobInfo::ExecJob($varname) => $processing,
            JobInfo::RunJob($varname) => $processing,
            JobInfo::LocalJob($varname) => $processing,
            JobInfo::ServiceRunJob($varname) => $processing,
        }
    };
}

pub use match_all_jobs;

impl TryFrom<HashMap<String, Vec<String>>> for JobInfo {
    type Error = Error;

    fn try_from(mut parameters: HashMap<String, Vec<String>>) -> Result<Self, Self::Error> {
        let kind = parameters.remove("kind");
        if kind == None {
            return Err(Error::msg(format!["The job has no job kind"]));
        }
        if kind.as_ref().unwrap().len() != 1 {
            debug!["The job has several kinds set, using the last configured one"];
        }
        let kind = kind.unwrap().pop().unwrap();
        let job_info: JobInfo;
        match kind.as_str() {
            ExecJobInfo::LABEL => {
                let job = ExecJobInfo::try_from(parameters)?;
                job_info = JobInfo::ExecJob(Box::new(job));
            },
            RunJobInfo::LABEL => {
                let job = RunJobInfo::try_from(parameters)?;
                job_info = JobInfo::RunJob(Box::new(job));
            },
            LocalJobInfo::LABEL => {
                let job = LocalJobInfo::try_from(parameters)?;
                job_info = JobInfo::LocalJob(Box::new(job));
            },
            ServiceRunJobInfo::LABEL => {
                let job = ServiceRunJobInfo::try_from(parameters)?;
                job_info = JobInfo::ServiceRunJob(Box::new(job));
            }
            _ => return Err(Error::msg(format!["Unsupported job type {}", kind])),
        }
        Ok(job_info)
    }
}

impl JobInfo {
    /// Start scheduling the execution of the job.
    /// This future should never return unless a fatal configuration error occured
    pub async fn start(self, handle: Docker) -> Result<Option<bool>, Error> {
        let mut set = JoinSet::new();

        let cron;
        let may_run_parallel;
        match_all_jobs!(&self, e, {cron = e.get_schedule(); may_run_parallel = e.may_run_parallel();});
        let initial_cron = cron.clone();
        set.spawn(async move {cron_sleep(&initial_cron).await});
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(ExecInfo::Schedule(_))) => {
                    // Return from timer
                    if may_run_parallel || set.is_empty() {
                        let handle_copy = handle.clone();
                        match_all_jobs!(&self, e, {
                            let exec_job = e.as_ref().clone();
                            set.spawn(async move {
                                let start_time = time::Instant::now();
                                let name = exec_job.name.clone();
                                let e = exec_job.exec(&handle_copy).await;
                                let duration = time::Instant::now() - start_time;
                                info!("Job {} ended in {}.{:04} seconds", name, duration.as_secs(), duration.as_millis()%1000);
                                e
                            });
                        });
                    }
                    let cron = cron.clone();
                    set.spawn(async move {cron_sleep(&cron).await});
                },
                Ok(Ok(ExecInfo::Report(r))) => {
                    info!("Job ended successfully: {} - {:?}", self.name(), r);
                },
                Ok(Err(e)) => {
                    error!("An error occured while running job {}: {}", self.name(), e);
                    // break;
                },
                Err(e) => {
                    error!("A join error occured while running job {}: {}", self.name(), e);
                    return Err(Error::new(e));
                }
            }
        }
        Err(Error::msg(format!("The job {} unexpectedly exhausted all its runners", self.name())))
    }

    /// Get the name of the job
    pub fn name(&self) -> &String {
        match_all_jobs!(self, e, &e.name)
    }

    /// Get the command executed when the job is triggered
    pub fn command(&self) -> &String {
        match_all_jobs!(self, e, &e.command)
    }

    /// Get the schedule on which the job is executed
    #[deprecated]
    pub fn schedule(&self) -> &Cron {
        match_all_jobs!(self, e, &e.schedule)
    }

    /// Get the job's type as a str
    pub fn kind(&self) -> &str {
        match self {
            JobInfo::ExecJob(_) => ExecJobInfo::LABEL,
            JobInfo::RunJob(_) => RunJobInfo::LABEL,
            JobInfo::LocalJob(_) => LocalJobInfo::LABEL,
            JobInfo::ServiceRunJob(_) => ServiceRunJobInfo::LABEL,
        }
    }
}
