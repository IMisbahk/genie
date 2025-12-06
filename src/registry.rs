use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct RegistryEntry {
    pub name: String,
    pub path: String,
    pub created_at_unix: u64,
}

fn registryPath() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        let base = PathBuf::from(home).join(".genie");
        let _ = fs::create_dir_all(&base);
        Some(base.join("registry.json"))
    } else {
        None
    }
}

pub fn loadRegistry() -> Vec<RegistryEntry> {
    if let Some(path) = registryPath() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(entries) = serde_json::from_str::<Vec<RegistryEntry>>(&data) {
                return entries;
            }
        }
    }
    Vec::new()
}

fn saveRegistry(entries: &[RegistryEntry]) {
    if let Some(path) = registryPath() {
        if let Ok(serialized) = serde_json::to_string_pretty(entries) {
            let _ = fs::write(path, serialized);
        }
    }
}

pub fn addToRegistry(name: &str, path: &str, createdAtUnix: u64) {
    let mut entries = loadRegistry();
    if !entries.iter().any(|entry| entry.path == path) {
        entries.push(RegistryEntry {
            name: name.to_string(),
            path: path.to_string(),
            created_at_unix: createdAtUnix,
        });
        saveRegistry(&entries);
    }
}
