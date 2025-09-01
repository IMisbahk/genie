use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::{Connection, params};
use warp::Filter;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub project_name: String,
    pub created_at_unix: u64,
    pub genie_version: &'static str,
}

#[derive(Parser)]
#[command(name = "genie")]
#[command(about = "A personal distributed VCS", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Status,
    Commit {
        #[arg(short, long)]
        message: Option<String>,
    },
    Log,
    Ui,
}

pub fn init_project() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let project_name = cwd
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    let genie_dir: PathBuf = cwd.join(".genie");

    if genie_dir.exists() {
        println!("⚠️  .genie already exists at: {}", genie_dir.display());
        return Ok(());
    }

    fs::create_dir_all(&genie_dir)?;
    fs::create_dir_all(genie_dir.join("keys"))?;

    let ignore_path = cwd.join(".genieignore");
    if !ignore_path.exists() {
        let default_ignore = "\
target/
build/
dist/
*.o
*.so
*.dll
*.exe
node_modules/
.DS_Store
.genie/
.genieignore
*.log
cargo.lock
cargo.toml
.github/
.gitignore
";
        fs::write(ignore_path, default_ignore)?;
    }

    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let cfg = Config {
        project_name,
        created_at_unix: since_epoch,
        genie_version: "0.0.1",
    };

    let cfg_path = genie_dir.join("config.json");
    let mut f = fs::File::create(&cfg_path)?;
    let config_json = serde_json::to_string_pretty(&cfg)?;
    f.write_all(config_json.as_bytes())?;
    f.flush()?;

    fs::File::create(genie_dir.join("lock"))?;

    let conn = Connection::open(genie_dir.join("history.db"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS commits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            message TEXT NOT NULL,
            author TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS commit_files (
            commit_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            last_modified INTEGER NOT NULL,
            PRIMARY KEY (commit_id, file_path)
        )",
        [],
    )?;

    println!("🧞 Initialized Genie project at {}", genie_dir.display());
    println!("✔ Wrote config -> {}", cfg_path.display());
    println!("✔ Created history.db");

    Ok(())
}

fn open_history() -> Option<Connection> {
    let cwd = env::current_dir().ok()?;
    let db_path = cwd.join(".genie/history.db");
    Connection::open(db_path).ok()
}

fn load_ignore_patterns(dir: &PathBuf) -> Vec<String> {
    let ignore_path = dir.join(".genieignore");
    if !ignore_path.exists() {
        return Vec::new();
    }

    let file = fs::File::open(ignore_path).unwrap();
    io::BufReader::new(file)
        .lines()
        .flatten()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect()
}

fn list_files_recursive(dir: &PathBuf, ignore_patterns: &Vec<String>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(rel_path) = path.strip_prefix(dir) {
                let rel_str = rel_path.to_string_lossy().to_string();

                if ignore_patterns.iter().any(|p| rel_str.contains(p) || path.ends_with(p)) {
                    continue;
                }
            }

            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name == ".genie" {
                        continue;
                    }
                }
                files.extend(list_files_recursive(&path, ignore_patterns));
            } else {
                files.push(path);
            }
        }
    }
    files
}

pub fn make_commit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(mut conn) = open_history() {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        conn.execute(
            "INSERT INTO commits (timestamp, message, author) VALUES (?1, ?2, ?3)",
            params![ts, message, "local-user"],
        )?;
        let id: i64 = conn.last_insert_rowid();

        let cwd = env::current_dir()?;
        let ignore_patterns = load_ignore_patterns(&cwd);
        let files = list_files_recursive(&cwd, &ignore_patterns);

        let tx = conn.transaction()?;

        for file in files {
            if file.components().any(|c| c.as_os_str() == ".genie") {
                continue;
            }
            if let Ok(rel_path) = file.strip_prefix(&cwd) {
                if let Ok(metadata) = fs::metadata(&file) {
                    let file_size = metadata.len() as i64;
                    let last_modified = metadata.modified()
                        .ok()
                        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    let file_path = rel_path.to_string_lossy().to_string();

                    tx.execute(
                        "INSERT INTO commit_files (commit_id, file_path, file_size, last_modified) VALUES (?1, ?2, ?3, ?4)",
                        params![id, file_path, file_size, last_modified],
                    )?;
                }
            }
        }

        tx.commit()?;
        println!("✅ Commit {} — \"{}\"", id, message);
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
    Ok(())
}

pub fn show_status() {
    let cwd = env::current_dir().unwrap_or_default();
    let genie_dir = cwd.join(".genie");
    if !genie_dir.exists() {
        println!("No Genie repo found in the current directory. Run `genie init`.");
        return;
    }

    println!("📂 Project: {}", cwd.display());
    if let Some(conn) = open_history() {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM commits").unwrap();
        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        println!("✔ {} commits recorded", count);

        let last_commit_id: Option<i64> = conn.query_row(
            "SELECT id FROM commits ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ).ok();

        let mut tracked_files: HashMap<String, (i64, i64)> = HashMap::new();
        if let Some(cid) = last_commit_id {
            let mut stmt = conn.prepare(
                "SELECT file_path, file_size, last_modified FROM commit_files WHERE commit_id = ?1"
            ).unwrap();
            let rows = stmt
                .query_map(params![cid], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .unwrap();
            for row in rows.flatten() {
                tracked_files.insert(row.0, (row.1, row.2));
            }
        }

        let ignore_patterns = load_ignore_patterns(&cwd);
        let files = list_files_recursive(&cwd, &ignore_patterns);

        let mut actual_files: HashMap<String, (i64, i64)> = HashMap::new();
        for file in files {
            if file.components().any(|c| c.as_os_str() == ".genie") {
                continue;
            }
            if let Ok(rel_path) = file.strip_prefix(&cwd) {
                if let Ok(metadata) = fs::metadata(&file) {
                    let file_size = metadata.len() as i64;
                    let last_modified = metadata.modified()
                        .ok()
                        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);

                    let file_path = rel_path.to_string_lossy().to_string();
                    actual_files.insert(file_path, (file_size, last_modified));
                }
            }
        }

        let mut untracked = Vec::new();
        for file in actual_files.keys() {
            if !tracked_files.contains_key(file) {
                untracked.push(file.clone());
            }
        }

        let mut modified = Vec::new();
        for (file, (size, modified_time)) in actual_files.iter() {
            if let Some((prev_size, prev_modified)) = tracked_files.get(file) {
                if prev_size != size || prev_modified != modified_time {
                    modified.push(file.clone());
                }
            }
        }

        if untracked.is_empty() && modified.is_empty() {
            println!("No changes since last commit.");
        } else {
            if !untracked.is_empty() {
                println!("Untracked files:");
                for file in untracked {
                    println!("  + {}", file);
                }
            }
            if !modified.is_empty() {
                println!("Modified files:");
                for file in modified {
                    println!("  ~ {}", file);
                }
            }
        }
    }
}

pub fn show_log() {
    if let Some(conn) = open_history() {
        let mut stmt = conn
            .prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id ASC")
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap();

        println!("📝 Commit history:");
        for row in rows {
            let (id, ts, msg, author) = row.unwrap();
            println!("  {} | {} | {} | {}", id, ts, author, msg);
        }
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
}

pub async fn start_ui_server() {
   let commits_route = warp::path!("api" / "commits")
        .and(warp::get())
        .map(|| {
            if let Some(conn) = open_history() {
                let mut stmt = conn
                    .prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id ASC")
                    .unwrap();
                let rows = stmt
                    .query_map([], |row| {
                        Ok(serde_json::json!({
                            "id": row.get::<_, i64>(0)?,
                            "timestamp": row.get::<_, i64>(1)?,
                            "message": row.get::<_, String>(2)?,
                            "author": row.get::<_, String>(3)?,
                        }))
                    })
                    .unwrap();

                let commits: Vec<_> = rows.filter_map(Result::ok).collect();
                warp::reply::json(&commits)
            } else {
                warp::reply::json(&Vec::<serde_json::Value>::new())
            }
        });

    let files_route = warp::path!("api" / "files")
        .and(warp::get())
        .map(|| {
            if let Some(conn) = open_history() {
                let last_commit_id: Option<i64> = conn.query_row(
                    "SELECT id FROM commits ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                ).ok();

                if let Some(cid) = last_commit_id {
                    let mut stmt = conn.prepare(
                        "SELECT file_path, file_size, last_modified FROM commit_files WHERE commit_id = ?1"
                    ).unwrap();
                    let rows = stmt
                        .query_map(params![cid], |row| {
                            Ok(serde_json::json!({
                                "file_path": row.get::<_, String>(0)?,
                                "file_size": row.get::<_, i64>(1)?,
                                "last_modified": row.get::<_, i64>(2)?,
                            }))
                        })
                        .unwrap();

                    let files: Vec<_> = rows.filter_map(Result::ok).collect();
                    warp::reply::json(&files)
                } else {
                    warp::reply::json(&Vec::<serde_json::Value>::new())
                }
            } else {
                warp::reply::json(&Vec::<serde_json::Value>::new())
            }
        });

    let static_files = warp::fs::dir("ui");
    let routes = commits_route.or(files_route).or(static_files);
    println!("🚀 Starting Genie UI server at http://localhost:2718");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}