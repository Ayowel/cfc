use std::collections::HashMap;

use anyhow::{Error, Result};
use saphyr_parser::{Event, Parser};
use tracing::warn;

pub fn parse_yaml(payload: &String) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let mut parser = Parser::new_from_str(payload.as_str());
    let mut data = HashMap::new();
    let mut current_depth = -1;
    let mut is_vec_context = false;
    let mut current_job_name = "".to_string();
    let mut current_job_key = "".to_string();
    while let Some(token) = parser.next() {
        if token.is_err() {
            return Err(Error::new(token.unwrap_err()));
        }
        let (event, marker) = token.unwrap();
        match event {
            Event::DocumentStart | Event::DocumentEnd | Event::Nothing | Event::StreamStart => {},
            Event::Alias(_) => {
                warn!("Found an alias in the YAML file. Their use is not supported at the moment (as line {} column {})", marker.line(), marker.col());
            },
            Event::Scalar(value, _, _, _) => {
                match current_depth {
                    0 => {
                        if !current_job_name.is_empty() {
                            return Err(Error::msg(format!("Unexpected scalar in dict, a dict was was expected (at line {} col {})", marker.line(), marker.col())));
                        }
                        if data.contains_key(&value) {
                            warn!("The key '{}' appears several times in a single dict, this may produce unexpected results and is not supported. Please fix your YAML configuration (ar line {} col {})", value, marker.line(), marker.col());
                        } else {
                            data.insert(value, HashMap::new());
                        }
                    },
                    1 => {
                        let current_subdict = data.get_mut(&current_job_name).unwrap();
                        if current_job_key.is_empty() {
                            if current_subdict.contains_key(&value) {
                                warn!("The key '{}' appears several times in a single dict, this may produce unexpected results and is not supported. Please fix your YAML configuration (at line {} col {})", value, marker.line(), marker.col());
                            } else {
                                current_subdict.insert(value, vec![]);
                            }
                        } else {
                            current_subdict.get_mut(&current_job_key).unwrap().push(value);
                        }
                    },
                    _ => return Err(Error::msg(format!("Unhandled error while parsing yaml file (at line {} column {}): Unexpected scalar", marker.line(), marker.col()))),
                }
            },
            Event::SequenceStart(_, _) => {
                if current_depth != 1 || is_vec_context {
                    return Err(Error::msg(format!("Arrays may only be used at depth 2 in YAML configuration (at line {} column {})", marker.line(), marker.col())))
                }
                is_vec_context = true;
            },
            Event::SequenceEnd => is_vec_context = false,
            Event::MappingStart(_, _) => {
                current_depth += 1;
                match current_depth {
                    0 => {},
                    1 => assert!(!current_job_name.is_empty()),
                    _ => return Err(Error::msg(format!["Yaml dict is too deeply nested at line {}, column {} in file", marker.line(), marker.col()])),
                }
            },
            Event::MappingEnd => {
                current_depth -= 1;
                match current_depth {
                    0 => current_job_key = "".to_string(),
                    1 => current_job_name = "".to_string(),
                    _ => {},
                }
            },
            Event::StreamEnd => {
                return Ok(data);
            },
        }
    }
    return Err(Error::msg("The YAML parser ended unexpectedly"))
}
