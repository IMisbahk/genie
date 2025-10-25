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

fn registry_path() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        let base = PathBuf::from(home).join(".genie");
        let _ = fs::create_dir_all(&base);
        Some(base.join("registry.json"))
    } else {
        None
    }
}

pub fn load_registry() -> Vec<RegistryEntry> {
    if let Some(p) = registry_path() {
        if let Ok(data) = fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<Vec<RegistryEntry>>(&data) {
                return v;
            }
        }
    }
    Vec::new()
}

fn save_registry(entries: &[RegistryEntry]) {
    if let Some(p) = registry_path() {
        if let Ok(s) = serde_json::to_string_pretty(entries) {
            let _ = fs::write(p, s);
        }
    }
}

pub fn add_to_registry(name: &str, path: &str, created_at_unix: u64) {
    let mut entries = load_registry();
    if !entries.iter().any(|e| e.path == path) {
        entries.push(RegistryEntry { name: name.to_string(), path: path.to_string(), created_at_unix });
        save_registry(&entries);
    }
}
