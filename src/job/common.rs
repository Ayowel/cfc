use std::pin::Pin;

use anyhow::Error;
use bollard::container::LogOutput;
use futures_util::{Stream, TryStreamExt};

pub(crate) const UNKNOWN_CONTAINER_LABEL: &'static str = "UNKNOWN";

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
