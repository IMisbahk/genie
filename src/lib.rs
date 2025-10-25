use clap::{Parser, Subcommand};
use clap::CommandFactory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use rusqlite::{Connection, params};
use warp::Filter;
use warp::http::Uri;
use serde_json::json;
pub mod registry;
use crate::registry::{load_registry, add_to_registry};
pub mod ignore;
use crate::ignore::{build_ignore_set, list_files_recursive};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub project_name: String,
    pub created_at_unix: u64,
    pub genie_version: &'static str,
}

pub fn show_welcome() {
    println!("\n🧞‍♂️ Welcome to Genie!");
    println!("Fast, simple personal version control\n");
    println!("Quickstart:");
    println!("  1) cd into a project and run: genie init");
    println!("  2) check changes: genie status");
    println!("  3) commit: genie commit -m \"Your message\"\n");
    println!("UI Dashboard:");
    println!("  genie ui  # then open http://localhost:2718\n");
    println!("Next steps: 'genie docs' or 'genie --help'\n");
}

pub fn open_docs() {
    let url = "https://github.com/imisbahk/genie#readme";
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).status();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).status();
    }
    println!("Documentation: {}", url);
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
    Ui {
        #[arg(long)]
        port: Option<u16>,
    },
    Completions {
        #[arg(value_parser = ["bash", "zsh", "fish"].into_iter().collect::<Vec<_>>())]
        shell: String,
    },
    Man,
    SelfUpdate,
    Welcome,
    Docs,
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
    fs::create_dir_all(genie_dir.join("refs"))?;
    fs::create_dir_all(genie_dir.join("refs/heads"))?;

    let ignore_path = cwd.join(".genieignore");
    if !ignore_path.exists() {
        let default_ignore = "\
target/\n
build/\n
dist/\n
node_modules/\n
.git/\n
.DS_Store\n
.genie/\n
*.o\n
*.so\n
*.dll\n
*.exe\n
*.log\n
.github/\n
.gitignore\n
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
    fs::write(genie_dir.join("HEAD"), "ref: refs/heads/main\n")?;
    fs::write(genie_dir.join("refs/heads/main"), b"")?;

    add_to_registry(&cfg.project_name, cwd.to_string_lossy().as_ref(), since_epoch);

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

fn open_history_for_path(path: &str) -> Option<Connection> {
    let db_path = PathBuf::from(path).join(".genie/history.db");
    Connection::open(db_path).ok()
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
        let ignore_set = build_ignore_set(&cwd);
        let files = list_files_recursive(&cwd, &cwd, &ignore_set);

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

        let ignore_patterns = build_ignore_set(&cwd);
        let files = list_files_recursive(&cwd, &cwd, &ignore_patterns);

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

pub fn generate_completions(shell: &str) {
    use clap_complete::{generate, shells};
    let mut cmd = Cli::command();
    match shell {
        "bash" => generate(shells::Bash, &mut cmd, "genie", &mut std::io::stdout()),
        "zsh" => generate(shells::Zsh, &mut cmd, "genie", &mut std::io::stdout()),
        "fish" => generate(shells::Fish, &mut cmd, "genie", &mut std::io::stdout()),
        _ => eprintln!("Unsupported shell: {}", shell),
    }
}

pub fn print_man() {
    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buf: Vec<u8> = Vec::new();
    man.render(&mut buf).ok();
    let _ = std::io::stdout().write_all(&buf);
}

pub fn do_self_update() {
    // Adjust owner/repo if different
    let status = self_update::backends::github::Update::configure()
        .repo_owner("imisbahk")
        .repo_name("genie")
        .bin_name("genie")
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build();

    match status {
        Ok(upd) => {
            match upd.update() {
                Ok(u) => println!("Updated to {}", u.version()),
                Err(e) => eprintln!("Self-update failed: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to configure updater: {}", e),
    }
}

pub async fn start_ui_server(port: Option<u16>) {
    let port = port.unwrap_or(2718);
   let commits_route = warp::path!("api" / "commits")
        .and(warp::get())
        .map(|| {
            if let Some(conn) = open_history() {
                let mut stmt = conn
                    .prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id ASC")
                    .unwrap();
                let rows = stmt
                    .query_map([], |row| {
                        Ok(json!({
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
                            Ok(json!({
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

    let projects_route = warp::path!("api" / "projects")
        .and(warp::get())
        .map(|| {
            let entries = load_registry();
            let out: Vec<_> = entries.into_iter().map(|e| json!({
                "name": e.name,
                "path": e.path,
                "created_at": e.created_at_unix,
            })).collect();
            warp::reply::json(&out)
        });

    let project_commits = warp::path!("api" / "project" / String / "commits")
        .and(warp::get())
        .map(|name: String| {
            let entries = load_registry();
            if let Some(e) = entries.into_iter().find(|e| e.name == name) {
                if let Some(conn) = open_history_for_path(&e.path) {
                    let mut stmt = conn
                        .prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id ASC")
                        .unwrap();
                    let rows = stmt
                        .query_map([], |row| {
                            Ok(json!({
                                "id": row.get::<_, i64>(0)?,
                                "timestamp": row.get::<_, i64>(1)?,
                                "message": row.get::<_, String>(2)?,
                                "author": row.get::<_, String>(3)?,
                            }))
                        })
                        .unwrap();
                    let commits: Vec<_> = rows.filter_map(Result::ok).collect();
                    return warp::reply::json(&commits);
                }
            }
            warp::reply::json(&Vec::<serde_json::Value>::new())
        });

    let project_files = warp::path!("api" / "project" / String / "files")
        .and(warp::get())
        .map(|name: String| {
            let entries = load_registry();
            if let Some(e) = entries.into_iter().find(|e| e.name == name) {
                if let Some(conn) = open_history_for_path(&e.path) {
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
                                Ok(json!({
                                    "file_path": row.get::<_, String>(0)?,
                                    "file_size": row.get::<_, i64>(1)?,
                                    "last_modified": row.get::<_, i64>(2)?,
                                }))
                            })
                            .unwrap();
                        let files: Vec<_> = rows.filter_map(Result::ok).collect();
                        return warp::reply::json(&files);
                    }
                }
            }
            warp::reply::json(&Vec::<serde_json::Value>::new())
        });

    let index_redirect = warp::path::end().map(|| warp::redirect::temporary(Uri::from_static("/main.html")));
    let project_redirect = warp::path!("project" / String)
        .and(warp::path::end())
        .map(|_name: String| warp::redirect::temporary(Uri::from_static("/main.html")));

    let static_files = warp::fs::dir("ui");
    let routes = commits_route
        .or(files_route)
        .or(projects_route)
        .or(project_commits)
        .or(project_files)
        .or(index_redirect)
        .or(project_redirect)
        .or(static_files);
    println!("🚀 Starting Genie UI server at http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;
}