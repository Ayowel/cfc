use std::pin::Pin;

use anyhow::Error;
use bollard::container::LogOutput;
use croner::Cron;
use futures_util::{Stream, TryStreamExt};
use regex::Regex;

pub(crate) const UNKNOWN_CONTAINER_LABEL: &'static str = "UNKNOWN";


/// Extract a single value from a HashMap<String, Vec<String>>.
/// If the key is defined, the vec is expected to be of size 1
#[macro_export]
macro_rules! take_one {
    ($map: ident, $key: expr) => {
        $map.remove($key).map_or_else(|| Ok(None), |mut v| {
            if v.len() != 1 {
                Err(anyhow::Error::msg(format!("The job key {} has too may values ({:?})", $key, v)))
            } else {
                Ok(v.pop())
            }
        })
    };
}

/// Extract a single value from a HashMap<String, Vec<String>>.
/// The key has to be defined and the vec has to be of size 1
#[macro_export]
macro_rules! require_one {
    ($map: ident, $key: expr) => {
        $map.remove($key).map_or_else(|| {
            Err(anyhow::Error::msg(format!("The job key {} is required but not set", $key)))
        }, |mut v| {
            if v.len() != 1 {
                Err(anyhow::Error::msg(format!("The job key {} has too many values ({:?})", $key, v)))
            } else {
                Ok(v.pop().unwrap())
            }
        })
    };
}

/// Parse a user-provided string to generate the corresponding cronjob
pub(crate) fn schedule_to_cron(sched: &str) -> Result<Cron, Error> {
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

/// Returned by the schedule watch when a job's execution should occur.
#[derive(Clone, Debug, Default)]
pub struct ExecutionSchedule {}

/// Returned by a job to report on its execution if no error occured
#[derive(Clone, Debug, Default)]
pub struct ExecutionReport {
    pub retval: i64,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug)]
pub enum ExecInfo {
    Report(ExecutionReport),
    Schedule(ExecutionSchedule),
}

impl ExecutionReport {
    pub async fn exhaust_stream(&mut self, stream: Pin<Box<dyn Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>>) -> Result<(), Error> {
        if self.stdout.is_some() || self.stderr.is_some() {
            return Err(Error::msg("The report already contains a stream's data."))
        }
        let l: Vec<_> = stream.try_collect().await.map_err(|e| Error::new(e))?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        for stream in l {
            match stream {
                bollard::container::LogOutput::StdErr { message } => stderr += &String::from_utf8(message.into()).map_err(|e| Error::new(e))?,
                bollard::container::LogOutput::StdOut { message } => stdout += &String::from_utf8(message.into()).map_err(|e| Error::new(e))?,
                bollard::container::LogOutput::StdIn { message: _ } => {},
                bollard::container::LogOutput::Console { message } => stdout += &String::from_utf8(message.into()).map_err(|e| Error::new(e))?,
            }
        }
        if !stdout.is_empty() {
            self.stdout = Some(stdout);
        }
        if !stderr.is_empty() {
            self.stderr = Some(stderr);
        }
        Ok(())
    }
}
