use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::{Connection, params};
use sha2::{Sha256, Digest};
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
            file_hash TEXT NOT NULL,
            PRIMARY KEY (commit_id, file_path)
        )",
        [],
    )?;

    println!("🧞 Initialized Genie project at {}", genie_dir.display());
    println!("✔ Wrote config -> {}", cfg_path.display());
    println!("✔ Created history.db with commits and commit_files tables");

    Ok(())
}

fn open_history() -> Option<Connection> {
    let cwd = env::current_dir().ok()?;
    let db_path = cwd.join(".genie/history.db");
    Connection::open(db_path).ok()
}

fn hash_file(path: &PathBuf) -> Option<String> {
    let data = fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    Some(format!("{:x}", result))
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
                if let Some(hash) = hash_file(&file) {
                    tx.execute(
                        "INSERT INTO commit_files (commit_id, file_path, file_hash) VALUES (?1, ?2, ?3)",
                        params![id, rel_path.to_string_lossy(), hash],
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

        let mut tracked_files: HashMap<String, String> = HashMap::new();
        if let Some(cid) = last_commit_id {
            let mut stmt = conn.prepare(
                "SELECT file_path, file_hash FROM commit_files WHERE commit_id = ?1"
            ).unwrap();
            let rows = stmt
                .query_map(params![cid], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap();
            for row in rows.flatten() {
                tracked_files.insert(row.0, row.1);
            }
        }

        let ignore_patterns = load_ignore_patterns(&cwd);
        let files = list_files_recursive(&cwd, &ignore_patterns);
        let mut actual_files: HashMap<String, String> = HashMap::new();
        for file in files {
            if file.components().any(|c| c.as_os_str() == ".genie") {
                continue;
            }
            if let Ok(rel_path) = file.strip_prefix(&cwd) {
                let rel_str = rel_path.to_string_lossy().to_string();
                if let Some(hash) = hash_file(&file) {
                    actual_files.insert(rel_str, hash);
                }
            }
        }

        let mut untracked = Vec::new();
        for (file, _) in actual_files.iter() {
            if !tracked_files.contains_key(file) {
                untracked.push(file.clone());
            }
        }

        let mut modified = Vec::new();
        for (file, hash) in actual_files.iter() {
            if let Some(prev_hash) = tracked_files.get(file) {
                if prev_hash != hash {
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
    // Endpoint to get commits
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

    // Endpoint to get files for last commit
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
                        "SELECT file_path, file_hash FROM commit_files WHERE commit_id = ?1"
                    ).unwrap();
                    let rows = stmt
                        .query_map(params![cid], |row| {
                            Ok(serde_json::json!({
                                "file_path": row.get::<_, String>(0)?,
                                "file_hash": row.get::<_, String>(1)?,
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

    let routes = commits_route.or(files_route);

    println!("🚀 Starting Genie UI server at http://localhost:3030");
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}