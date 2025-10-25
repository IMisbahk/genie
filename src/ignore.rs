use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;

pub fn build_ignore_set(dir: &PathBuf) -> GlobSet {
    let ignore_path = dir.join(".genieignore");
    let mut builder = GlobSetBuilder::new();

    if ignore_path.exists() {
        if let Ok(file) = fs::File::open(ignore_path) {
            for line in io::BufReader::new(file).lines().flatten() {
                let pat = line.trim();
                if pat.is_empty() || pat.starts_with('#') { continue; }

                let mut variants: Vec<String> = Vec::new();

                if pat.ends_with('/') {
                    let dir = pat.trim_end_matches('/');
                    variants.push(format!("{}/**", dir));
                    variants.push(format!("**/{}/**", dir));
                    variants.push(dir.to_string());
                    variants.push(format!("**/{}", dir));
                } else {
                    variants.push(pat.to_string());
                    variants.push(format!("**/{}", pat));
                }

                for v in variants {
                    if let Ok(glob) = Glob::new(&v) {
                        builder.add(glob);
                    }
                }
            }
        }
    }

    if let Ok(glob) = Glob::new(".genie/**") { builder.add(glob); }

    builder.build().unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap())
}

pub fn list_files_recursive(root: &PathBuf, dir: &PathBuf, ignore_set: &GlobSet) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(rel_path) = path.strip_prefix(root) {
                let rel_str = rel_path.to_string_lossy();
                if ignore_set.is_match(rel_str.as_ref()) { continue; }
            }

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == ".genie" { continue; }
                }
                files.extend(list_files_recursive(root, &path, ignore_set));
            } else {
                files.push(path);
            }
        }
    }
    files
}
