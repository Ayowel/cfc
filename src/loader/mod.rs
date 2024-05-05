use std::collections::HashMap;

use anyhow::{Error, Result};
use tokio::fs;
use tracing::{debug, trace};

use crate::{context::ApplicationContext, job::JobInfo};

#[cfg(feature = "labels")]
pub mod docker;
#[cfg(feature = "ini")]
pub mod ini;
#[cfg(feature = "yaml")]
pub mod yaml;

/// Maps a normalized map to a JobInfo list. All keys set in the sub-HashMaps MUST be non-empty Vec.
fn map_to_job(map: HashMap<String, HashMap<String, Vec<String>>>) -> Result<Vec<JobInfo>> {
    let mut retval = vec![];
    for (name, mut parameters) in map{
        debug!["Create new job '{}'", name];
        trace!["Create new job '{}' from {:?}", name, parameters];
        if !parameters.contains_key("name") {
            parameters.insert("name".to_string(), vec![name.clone()]);
        }
        match JobInfo::try_from(parameters) {
            Ok(job) => {
                trace!["Created new job {:?}", job];
                retval.push(job);
            }
            Err(e) => return Err(e),
        }
    }
    return Ok(retval);
}

fn load_file_content(content: &String, ext: &String) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let r = Err(Error::msg("No compiled feature supports parsing files, try to use the --docker option to get configuration from labels"));
    let is_ini = ext == "ini";
    let is_yaml = ["yaml", "yml"].contains(&ext.as_str());
    #[cfg(feature="ini")]
    let r = if is_ini || !is_yaml {
        r.or_else(|_| ini::parse_ini(content))
    } else {r};
    #[cfg(feature="yaml")]
    let r = if is_yaml || !is_ini {
        r.or_else(|_| yaml::parse_yaml(content))
    } else {r};
    r
}

pub async fn load_file(path: &String, mut _ctx: &ApplicationContext) -> Result<Vec<JobInfo>> {
    fs::read(&path).await
        .map_err(|e| Error::new(e))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|e| Error::new(e)))
        .and_then(|c| load_file_content(&c, &path.split(".").last().unwrap().to_lowercase()))
        .and_then(|mut map| {
            // TODO: load global configs into ctx
            map.remove("global");
            Ok(map)
        }).and_then(|map| map_to_job(map))
}

pub async fn load_labels(_ctx: &ApplicationContext) -> Result<Vec<JobInfo>> {
    #[cfg(feature = "labels")]
    let jobs = docker::get_tagged_targets(&_ctx.get_handle()?, &_ctx.label_prefixes, _ctx.unsafe_labels).await
        .and_then(|map| map_to_job(map));
    #[cfg(not(feature = "labels"))]
    let jobs = Err(Error::msg("No compiled feature supports parsing labels, try to use file parsing"));
    jobs
}
