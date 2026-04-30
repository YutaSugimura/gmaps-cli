use crate::config::{config_dir, ensure_private_dir, write_private_file};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Places {
    #[serde(default)]
    pub places: Vec<Place>,
}

pub fn places_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("places.yaml"))
}

pub fn load() -> Result<Places> {
    let path = places_path()?;
    if !path.exists() {
        return Ok(Places::default());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(Places::default());
    }
    let p: Places = serde_yaml_ng::from_str(&text)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(p)
}

pub fn save(places: &Places) -> Result<()> {
    let dir = config_dir()?;
    ensure_private_dir(&dir)?;
    let path = places_path()?;
    let yaml = serde_yaml_ng::to_string(places).context("Failed to serialize places to YAML")?;
    write_private_file(&path, yaml.as_bytes())?;
    Ok(())
}

impl Places {
    pub fn find(&self, name: &str) -> Option<&Place> {
        self.places.iter().find(|p| p.name == name)
    }

    /// Replace if the same name exists, otherwise append. Returns true when an existing entry was updated.
    pub fn upsert(&mut self, place: Place) -> bool {
        if let Some(existing) = self.places.iter_mut().find(|p| p.name == place.name) {
            *existing = place;
            true
        } else {
            self.places.push(place);
            false
        }
    }

    /// Returns true if a place with the given name was removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.places.len();
        self.places.retain(|p| p.name != name);
        self.places.len() < len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_and_find() {
        let mut places = Places::default();
        let added = places.upsert(Place {
            name: "home".into(),
            lat: 35.0,
            lng: 139.0,
            note: None,
        });
        assert!(!added);
        assert!(places.find("home").is_some());
        let updated = places.upsert(Place {
            name: "home".into(),
            lat: 36.0,
            lng: 140.0,
            note: None,
        });
        assert!(updated);
        assert_eq!(places.find("home").unwrap().lat, 36.0);
    }

    #[test]
    fn remove_works() {
        let mut places = Places::default();
        places.upsert(Place {
            name: "home".into(),
            lat: 35.0,
            lng: 139.0,
            note: None,
        });
        assert!(places.remove("home"));
        assert!(!places.remove("home"));
    }
}
