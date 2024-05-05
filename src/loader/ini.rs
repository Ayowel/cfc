use std::collections::HashMap;

use anyhow::{Error, Result};
use ini_core as ini;
use regex::Regex;
use tracing::{debug, trace, warn};

pub fn parse_ini(payload: &String) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
    let mut current_section = "".to_string();
    let mut current_data = HashMap::new();
    let mut parser = ini::Parser::new(payload.as_str());
    while let Some(i) = parser.next() {
        match i {
            ini::Item::Error(e) => {
                return Err(Error::msg(e.to_string()));
            },
            ini::Item::Section(s) => {
                current_section = s.trim().to_string();
                debug!["Found ini config section {}", s];
                let re = Regex::new("^(?<kind>[^\\s]+)\\s*\"(?<name>[^\"]+)\"$").unwrap();
                let (section_kind, section_name): (String, String);
                match re.captures(&current_section) {
                    Some(c) => {
                        section_kind = c.name("kind").unwrap().as_str().to_string();
                        section_name = c.name("name").unwrap().as_str().to_string();
                        current_section = format!("{} \"{}\"", section_kind, section_name);
                    },
                    None => {
                        if current_section == "global" {
                            current_data.insert(current_section.clone(), HashMap::new());
                            continue;
                        } else {
                            return Err(Error::msg(format!["Found unsupported ini header {}", s]));
                        }
                    }
                }
                if current_data.contains_key(&current_section) {
                    warn![
                        "The section key '{}' is present more than once in the configuration. {} {} {}",
                        current_section,
                        "Both sections will be merged.",
                        "This is not supported and the actual behavior may change in the future.",
                        "Update your configuration files to only declare jobs once.",
                        ];
                } else {
                    current_data.insert(current_section.clone(), HashMap::from([
                        ("kind".to_string(), vec![section_kind]),
                        ("name".to_string(), vec![section_name])
                    ]));
                }
            },
            ini::Item::SectionEnd => current_section = "".to_string(),
            ini::Item::Property(k, v) => {
                let k = k.trim();
                let v = v.map(|v| v.trim());
                trace!["Found entry '{}' with value '{:?}'", k, v];
                if current_section.is_empty() {
                    return Err(Error::msg(format!("Found property {} without a section", k)));
                }
                if v == None {
                    warn!["Found property '{}' without a value, it will be ignored.", k];
                    continue;
                }
                let section_info = current_data.get_mut(&current_section).unwrap();
                if !section_info.contains_key(k) {
                    section_info.insert(k.trim().to_string(), vec![]);
                }
                section_info.get_mut(k).unwrap().push(v.unwrap().to_string());
            },
            ini::Item::Comment(_) => {},
            ini::Item::Blank => {},
        }
    }
    Ok(current_data)
}
