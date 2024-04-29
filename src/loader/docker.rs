use std::collections::{HashMap, HashSet};

use anyhow::{Error, Result};
use bollard::{container::ListContainersOptions, Docker};
use json::{self, JsonValue};
use tracing::{event, Level};

use crate::job::LocalJobInfo;

pub async fn get_tagged_targets(handle: &Docker, label_prefixes: &Vec<String>, allow_unsafe_jobs: bool) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let mut container_idx: HashSet<String> = HashSet::new();
    let mut job_map = HashMap::new();
    for prefix in label_prefixes {
        let label_filter = format!("{prefix}.enabled=true");
        event![Level::DEBUG, "Looking for containers with label {label_filter}"];
        let options = ListContainersOptions::<String> {
            all: true, // Do we really want to index 'all' ?
            filters: HashMap::from([("label".into(), vec![label_filter])]),
            ..Default::default()
        };
        let container_list;
        match handle.list_containers(Some(options)).await {
            Ok(l) => container_list = l,
            Err(e) => {
                event!(Level::ERROR, "Failed to get container list: {}", e);
                return Err(Error::msg("Failed to get container list"));
            }
        }
        event!(Level::DEBUG, "Found {} candidate containers", container_list.len());
        for container in container_list {
            let container_id = container.id.as_ref();
            if container_idx.contains(container_id.unwrap()) {
                event![Level::DEBUG, "Skipping {} as it was already encountered", container_id.unwrap()];
                continue;
            }
            container_idx.insert(container_id.unwrap().to_string());
            event!(Level::DEBUG, "On container {:?}", container);
            if !container.labels.as_ref().is_some_and(|c| !c.is_empty()) {
                continue;
            }
            for (key, value) in container.labels.as_ref().unwrap() {
                let mut key_parts = key.split(".");
                if key_parts.next().map_or(true, |p| !label_prefixes.contains(&p.to_string())) {
                    event![Level::TRACE, "Skipping label {} as it does not start with one of the expected prefix", key];
                    continue;
                }
                let job_kind = key_parts.next().and_then(|k| Some(k.to_string()));
                let job_name = key_parts.next().and_then(|n| Some(n.to_string()));
                let job_parameter = key_parts.next().and_then(|p| Some(p.to_string()));
                if job_kind.is_none() || job_name.is_none() || job_parameter.is_none() || key_parts.next().is_some() {
                    event![Level::TRACE, "Skipping label {} as its key does not contain the 4 expected parts", key];
                    continue;
                }
                let job_kind = job_kind.unwrap();
                let job_name = job_name.unwrap();
                let job_parameter = job_parameter.unwrap();
                if !allow_unsafe_jobs {
                    match job_kind.as_str() {
                        LocalJobInfo::LABEL => {
                            event![Level::ERROR, "Found local job declared in tags, however this is not allowed. Skipping label {}.", key];
                            continue;
                        },
                        _ => {},
                    }
                }
                // Start including the key
                let job_key = format!["{}_{}_{}", container_id.unwrap(), job_kind, job_name];
                if !job_map.contains_key(&job_key) {
                    job_map.insert(job_key.clone(), HashMap::from([
                        ("kind".to_string(), vec![job_kind.clone()]),
                        ("name".to_string(), vec![job_name.clone()]),
                    ]));
                }
                let evt_info = job_map.get_mut(&job_key).unwrap();
                if !evt_info.get("kind").unwrap().contains(&job_kind) {
                    event![Level::ERROR, "Found conflicting cron types for job {} (had '{}' but found '{}' in {})", job_name, evt_info.get("kind").unwrap().first().unwrap(), job_kind, key];
                    return Err(Error::msg("Conflicting cron types on label"));
                }
                // FIXME: this is only required due to the fact that we allow the use of multiple prefix keys
                if let Some(param_value) = evt_info.get(&job_parameter) {
                    event![Level::WARN, "Parameter is set more than once with different label prefixes (found on {})", key];
                    if !param_value.contains(value) {
                        return Err(Error::msg("Parameter set mote than once has different values in its occurences"));
                    }
                    continue;
                }
                match job_parameter.as_str() {
                    "volume"|"network"|"environment" => {
                        evt_info.insert(job_parameter, json::parse(value)
                            .map_or_else(|e| Err(Error::new(e)), |j| {
                                if let JsonValue::Array(v) = j {
                                    let mut values = vec![];
                                    for i in v {
                                        if let JsonValue::String(s) = i {
                                            values.push(s);
                                        } else {
                                            return Err(Error::msg(""));
                                        }
                                    }
                                    return Ok(values);
                                } else {
                                    return Err(Error::msg(""));
                                }
                            })
                            .unwrap_or_else(|_| vec![value.to_owned()])
                        );
                    },
                    _ => {evt_info.insert(job_parameter, vec![value.to_owned()]);},
                }
            }
        }
    }
    Ok(job_map)
}
