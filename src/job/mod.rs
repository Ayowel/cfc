//! Job representation
use anyhow::Error;
use bollard::Docker;
use croner::Cron;
use regex::Regex;
use tokio::task::JoinSet;
use tracing::{debug, warn};
use std::{collections::HashMap, fmt::Debug, time::Duration};

mod common;
mod exec;
mod run;
mod local;
mod servicerun;

pub use exec::ExecJobInfo;
pub use run::RunJobInfo;
pub use local::LocalJobInfo;
pub use servicerun::ServiceRunJobInfo;

/// Parse a user-provided string to generate the corresponding cronjob
fn schedule_to_cron(sched: &str) -> Result<Cron, Error> {
    // TODO: support multi-keys '@every' (e.g.: 1h30m)
    let mut sched = sched.trim().to_string();
    let re = Regex::new("^@every\\s+(?<interval>[0-9]+)(?<unit>s|m|h)$").unwrap();
    match re.captures(sched.as_str()) {
        Some(c) => {
            let interval: i32 = c.name("interval").unwrap().as_str().parse().unwrap();
            let unit = c.name("unit").unwrap().as_str();
            match unit {
                // TODO: add randomization of 0 values
                "s" => sched = format!("*/{} * * * * *", interval).to_string(),
                "m" => sched = format!("0 */{} * * * *", interval).to_string(),
                "h" => sched = format!("0 0 */{} * * *", interval).to_string(),
                _ => unreachable!("Encountered an unhandled time unit while parsing a schedule"),
            }
        },
        None => {},
    }
    Cron::new(&sched).with_seconds_optional().parse().map_err(|e| Error::new(e))
}

/// Sleep until the next occurence of the provided cron
async fn cron_sleep(cron: &Cron) -> Result<Option<bool>, Error> {
    let current_time = chrono::Local::now();
    let next_occurence = cron.find_next_occurrence(&current_time, false).unwrap();
    let sleep = (next_occurence - current_time).num_milliseconds();
    assert!(sleep >= 0);
    tokio::time::sleep(Duration::from_millis(sleep as u64)).await;
    Ok(Some(true))
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
        let command = parameters.remove("command");
        let schedule = parameters.remove("schedule");
        if kind == None {
            return Err(Error::msg(format!["The job has no job type"]));
        }
        if kind.as_ref().unwrap().len() != 1 {
            debug!["The job has several kinds set, using the last configured one"];
        }
        if command == None {
            return Err(Error::msg(format!["The job has no command"]));
        }
        if schedule == None {
            return Err(Error::msg(format!["The job has no schedule"]));
        }
        let schedule = schedule.unwrap().pop().unwrap();
        let kind = kind.unwrap().pop().unwrap();
        let job_info: JobInfo;
        match kind.as_str() {
            ExecJobInfo::LABEL => {
                let job = ExecJobInfo {
                    name: parameters.remove("name").map_or_else(|| "".to_string(), |mut n| n.pop().unwrap()),
                    schedule: schedule_to_cron(&schedule.as_str()).unwrap(),
                    command: command.unwrap().pop().unwrap(),
                    container: parameters.remove("container").map(|mut c| c.pop().unwrap()).unwrap(),
                    user: parameters.remove("user").map(|mut u| u.pop().unwrap()),
                    tty: parameters.remove("tty").map_or(false, |mut t| t.pop().unwrap().parse().unwrap()),
                    environment: parameters.remove("environment").unwrap_or(Default::default()),
                };
                job_info = JobInfo::ExecJob(Box::new(job));
            },
            RunJobInfo::LABEL => {
                let job = RunJobInfo {
                    name: parameters.remove("name").map_or_else(|| "".to_string(), |mut n| n.pop().unwrap()),
                    schedule: schedule_to_cron(&schedule.as_str()).unwrap(),
                    command: command.unwrap().pop().unwrap(),
                    image: parameters.remove("image").map(|mut c| c.pop().unwrap()),
                    user: parameters.remove("user").map_or(None, |mut u| Some(u.pop().unwrap())),
                    network: parameters.remove("network"),
                    hostname: parameters.remove("hostname").map_or(None, |mut u| Some(u.pop().unwrap())),
                    delete: parameters.remove("delete").map_or(true, |mut t| t.pop().unwrap().parse().unwrap()),
                    container: parameters.remove("container").map(|mut c| c.pop().unwrap()),
                    tty: parameters.remove("tty").map_or(false, |mut t| t.pop().unwrap().parse().unwrap()),
                    volume: parameters.remove("volume").unwrap_or_else(|| Default::default()),
                    environment: parameters.remove("environment").unwrap_or(Default::default()),
                };
                if job.image == None && job.container == None {
                    return Err(Error::msg(format!["The job {} has neither an image nor a container parameter. At least one of thse must be set.", job.name]));
                }
                job_info = JobInfo::RunJob(Box::new(job));
            },
            LocalJobInfo::LABEL => {
                let job = LocalJobInfo {
                    name: parameters.remove("name").map_or_else(|| "".to_string(), |mut n| n.pop().unwrap()),
                    schedule: schedule_to_cron(&schedule.as_str()).unwrap(),
                    command: command.unwrap().pop().unwrap(),
                    dir: parameters.remove("dir").map(|mut d| d.pop().unwrap()),
                    environment: parameters.remove("environment").unwrap_or(Default::default()),
                };
                job_info = JobInfo::LocalJob(Box::new(job));
            },
            ServiceRunJobInfo::LABEL => {
                let job = ServiceRunJobInfo {
                    name: parameters.remove("name").map_or_else(|| "".to_string(), |mut n| n.pop().unwrap()),
                    schedule: schedule_to_cron(&schedule.as_str()).unwrap(),
                    command: command.unwrap().pop().unwrap(),
                    image: parameters.remove("image").map(|mut c| c.pop().unwrap()),
                    user: parameters.remove("user").map_or(None, |mut u| Some(u.pop().unwrap())),
                    network: parameters.remove("network"),
                    delete: parameters.remove("delete").map_or(true, |mut t| t.pop().unwrap().parse().unwrap()),
                    container: parameters.remove("container").map(|mut c| c.pop().unwrap()),
                    tty: parameters.remove("tty").map_or(false, |mut t| t.pop().unwrap().parse().unwrap()),
                };
                job_info = JobInfo::ServiceRunJob(Box::new(job));
            }
            _ => return Err(Error::msg(format!["Unsupported job type {}", kind])),
        }
        if !parameters.is_empty() {
            let k: Vec<&String> = parameters.keys().collect();
            warn!["There are unused keys in the job. Unaffected values: {:?}", k];
        }
        Ok(job_info)
    }
}

impl JobInfo {
    /// Start scheduling the execution of the job.
    /// This future should never return unless a fatal configuration error occured
    pub async fn start(self, _handle: Docker) -> Result<Option<bool>, Error> {
        let mut set = JoinSet::new();

        let cron;
        let may_run_parallel;
        match_all_jobs!(&self, e, {cron = e.get_schedule(); may_run_parallel = e.may_run_parallel();});
        let initial_cron = cron.clone();
        set.spawn(async move {cron_sleep(&initial_cron).await});
        while let Some(res) = set.join_next().await {
            if let Ok(Ok(Some(_))) = res {
                if may_run_parallel || set.is_empty() {
                    match_all_jobs!(&self, e, {
                        let exec_job = e.as_ref().clone();
                        set.spawn(async {exec_job.exec().await});
                    });
                }
                let cron = cron.clone();
                set.spawn(async move {cron_sleep(&cron).await});
            }
        }
        warn!["A job terminated, this is probably not desired: {:?}", self];
        Err(Error::msg("Aborting because a job unexpectedly stopped"))
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
