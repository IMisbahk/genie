use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;

pub fn buildIgnoreSet(dir: &PathBuf) -> GlobSet {
    let ignorePath = dir.join(".genieignore");
    let mut builder = GlobSetBuilder::new();

    if ignorePath.exists() {
        if let Ok(file) = fs::File::open(ignorePath) {
            for line in io::BufReader::new(file).lines().flatten() {
                let pattern = line.trim();
                if pattern.is_empty() || pattern.starts_with('#') {
                    continue;
                }

                let mut variants: Vec<String> = Vec::new();

                if pattern.ends_with('/') {
                    let directory = pattern.trim_end_matches('/');
                    variants.push(format!("{}/**", directory));
                    variants.push(format!("**/{}/**", directory));
                    variants.push(directory.to_string());
                    variants.push(format!("**/{}", directory));
                } else {
                    variants.push(pattern.to_string());
                    variants.push(format!("**/{}", pattern));
                }

                for variant in variants {
                    if let Ok(glob) = Glob::new(&variant) {
                        builder.add(glob);
                    }
                }
            }
        }
    }

    if let Ok(glob) = Glob::new(".genie/**") {
        builder.add(glob);
    }

    builder
        .build()
        .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap())
}

pub fn listFilesRecursive(root: &PathBuf, dir: &PathBuf, ignoreSet: &GlobSet) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(relativePath) = path.strip_prefix(root) {
                let relativeString = relativePath.to_string_lossy();
                if ignoreSet.is_match(relativeString.as_ref()) {
                    continue;
                }
            }

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == ".genie" {
                        continue;
                    }
                }
                files.extend(listFilesRecursive(root, &path, ignoreSet));
            } else {
                files.push(path);
            }
        }
    }
    files
}
