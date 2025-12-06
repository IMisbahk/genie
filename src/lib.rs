#![allow(non_snake_case)]
use chrono::{DateTime, Utc};
use clap::{CommandFactory, Parser, Subcommand};
use regex::Regex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::time::{Duration, interval};
use tokio::{select, signal};
use warp::Filter;
use warp::http::Uri;

use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod ignore;
pub mod registry;

use crate::ignore::{buildIgnoreSet, listFilesRecursive};
use crate::registry::{addToRegistry, loadRegistry};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(alias = "project_name")]
    pub projectName: String,
    #[serde(alias = "created_at_unix")]
    pub createdAtUnix: u64,
    #[serde(alias = "genie_version")]
    pub genieVersion: String,
    #[serde(default, alias = "default_author")]
    pub defaultAuthor: Option<String>,
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
    Status {
        #[arg(long = "json")]
        jsonMode: bool,
        #[arg(long = "files")]
        includeFiles: bool,
        #[arg(long = "deep")]
        deepCompare: bool,
    },
    Commit {
        #[arg(short, long)]
        message: Option<String>,
    },
    Log {
        #[arg(long = "json")]
        jsonMode: bool,
        #[arg(long = "limit", default_value_t = 50)]
        limit: usize,
    },
    Ui {
        #[arg(long)]
        port: Option<u16>,
    },
    Watch {
        #[arg(long = "interval", default_value_t = 3)]
        intervalSeconds: u64,
        #[arg(long = "files")]
        includeFiles: bool,
        #[arg(long = "deep")]
        deepCompare: bool,
    },
    Guard {
        #[arg(long = "max-file-mb", default_value_t = 50)]
        maxFileMegabytes: u64,
        #[arg(long = "json")]
        jsonMode: bool,
        #[arg(long = "strict")]
        strictMode: bool,
    },
    Insights {
        #[arg(long = "json")]
        jsonMode: bool,
        #[arg(long = "top", default_value_t = 5)]
        topFiles: usize,
    },
    Projects {
        #[arg(long = "json")]
        jsonMode: bool,
        #[arg(long = "details")]
        includeDetails: bool,
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct StatusSnapshot {
    pub projectPath: String,
    pub commitCount: i64,
    pub lastCommitId: Option<i64>,
    pub trackedFiles: usize,
    pub trackedBytes: u64,
    pub modified: Vec<FileChange>,
    pub untracked: Vec<String>,
    pub missing: Vec<String>,
}

pub struct StatusOptions {
    pub jsonMode: bool,
    pub includeFiles: bool,
    pub deepCompare: bool,
}

pub struct LogOptions {
    pub jsonMode: bool,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct GuardFinding {
    pub severity: String,
    pub file: String,
    pub category: String,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Serialize)]
pub struct FileTouchStat {
    pub path: String,
    pub touches: i64,
}

#[derive(Debug, Serialize)]
pub struct FileSizeStat {
    pub path: String,
    pub bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct CommitSummary {
    pub id: i64,
    pub timestamp: i64,
    pub author: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileTypeStat {
    pub extension: String,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct InsightReport {
    pub projectPath: String,
    pub commitCount: i64,
    pub firstCommit: Option<i64>,
    pub lastCommit: Option<i64>,
    pub activeDays: u64,
    pub avgCommitsPerDay: f64,
    pub topFiles: Vec<FileTouchStat>,
    pub largestFiles: Vec<FileSizeStat>,
    pub recentCommits: Vec<CommitSummary>,
    pub fileTypes: Vec<FileTypeStat>,
    pub trackedFiles: usize,
    pub trackedBytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ProjectSummary {
    pub name: String,
    pub path: String,
    pub createdAt: u64,
    pub commitCount: Option<i64>,
    pub lastCommitTimestamp: Option<i64>,
}

#[derive(Clone)]
struct FileMeta {
    size: i64,
    modified: i64,
}

#[derive(Clone)]
struct ActualFile {
    absolutePath: PathBuf,
    meta: FileMeta,
}

pub fn showWelcome() {
    println!("\n🧞‍♂️ Welcome to Genie!");
    println!("Fast, simple personal version control\n");
    println!("Quickstart:");
    println!("  1) cd into a project and run: genie init");
    println!("  2) check changes: genie status");
    println!("  3) commit: genie commit -m \"Your message\"\n");
    println!("New superpowers:");
    println!("  • genie watch --files      # live status monitor");
    println!("  • genie guard              # secret + bloat scanner");
    println!("  • genie insights           # repository analytics\n");
    println!("UI Dashboard:");
    println!("  genie ui  # then open http://localhost:2718\n");
    println!("Next steps: 'genie docs' or 'genie --help'\n");
}

pub fn openDocs() {
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

pub fn initProject() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let projectName = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unnamed")
        .to_string();

    let genieDir: PathBuf = cwd.join(".genie");

    if genieDir.exists() {
        println!("⚠️  .genie already exists at: {}", genieDir.display());
        return Ok(());
    }

    fs::create_dir_all(&genieDir)?;
    fs::create_dir_all(genieDir.join("keys"))?;
    fs::create_dir_all(genieDir.join("refs"))?;
    fs::create_dir_all(genieDir.join("refs/heads"))?;

    let ignorePath = cwd.join(".genieignore");
    if !ignorePath.exists() {
        let defaultIgnore = "target/\n\nbuild/\n\ndist/\n\nnode_modules/\n\n.git/\n\n.DS_Store\n\n.genie/\n\n*.o\n\n*.so\n\n*.dll\n\n*.exe\n\n*.log\n\n.github/\n\n.gitignore\n";
        fs::write(ignorePath, defaultIgnore)?;
    }

    let sinceEpoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let cfg = Config {
        projectName: projectName.clone(),
        createdAtUnix: sinceEpoch,
        genieVersion: env!("CARGO_PKG_VERSION").to_string(),
        defaultAuthor: None,
    };

    let configPath = genieDir.join("config.json");
    let mut configFile = fs::File::create(&configPath)?;
    let configJson = serde_json::to_string_pretty(&cfg)?;
    configFile.write_all(configJson.as_bytes())?;
    configFile.flush()?;

    fs::File::create(genieDir.join("lock"))?;
    fs::write(genieDir.join("HEAD"), "ref: refs/heads/main\n")?;
    fs::write(genieDir.join("refs/heads/main"), b"")?;

    addToRegistry(&cfg.projectName, cwd.to_string_lossy().as_ref(), sinceEpoch);

    let conn = Connection::open(genieDir.join("history.db"))?;
    ensureHistorySchema(&conn)?;

    println!("🧞 Initialized Genie project at {}", genieDir.display());
    println!("✔ Wrote config -> {}", configPath.display());
    println!("✔ Created history.db");

    Ok(())
}

fn ensureHistorySchema(conn: &Connection) -> rusqlite::Result<()> {
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS commit_file_hashes (
            commit_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            PRIMARY KEY (commit_id, file_path)
        )",
        [],
    )?;

    Ok(())
}

fn loadProjectConfig() -> Option<Config> {
    let cwd = env::current_dir().ok()?;
    let configPath = cwd.join(".genie/config.json");
    let data = fs::read_to_string(configPath).ok()?;
    serde_json::from_str::<Config>(&data).ok()
}

fn openHistory() -> Option<Connection> {
    let cwd = env::current_dir().ok()?;
    let dbPath = cwd.join(".genie/history.db");
    let conn = Connection::open(dbPath).ok()?;
    let _ = ensureHistorySchema(&conn);
    Some(conn)
}

fn openHistoryForPath(path: &str) -> Option<Connection> {
    let dbPath = PathBuf::from(path).join(".genie/history.db");
    let conn = Connection::open(dbPath).ok()?;
    let _ = ensureHistorySchema(&conn);
    Some(conn)
}

fn readCurrentAuthor() -> String {
    if let Ok(author) = env::var("GENIE_AUTHOR") {
        if !author.trim().is_empty() {
            return author;
        }
    }
    if let Some(cfg) = loadProjectConfig() {
        if let Some(author) = cfg.defaultAuthor {
            if !author.trim().is_empty() {
                return author;
            }
        }
    }
    "local-user".to_string()
}

fn computeFileHash(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let readBytes = file.read(&mut buffer).ok()?;
        if readBytes == 0 {
            break;
        }
        hasher.update(&buffer[..readBytes]);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn resolveGenieDir() -> Option<PathBuf> {
    let cwd = env::current_dir().ok()?;
    let genieDir = cwd.join(".genie");
    if genieDir.exists() {
        Some(genieDir)
    } else {
        None
    }
}

fn updateHeadPointer(commitId: i64) {
    if let Some(genieDir) = resolveGenieDir() {
        let headPath = genieDir.join("HEAD");
        let headContents =
            fs::read_to_string(&headPath).unwrap_or_else(|_| "ref: refs/heads/main".to_string());
        let reference = headContents
            .split_whitespace()
            .nth(1)
            .unwrap_or("refs/heads/main");
        let branchPath = genieDir.join(reference);
        let _ = fs::write(branchPath, commitId.to_string());
    }
}

pub fn makeCommit(message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(mut conn) = openHistory() {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let author = readCurrentAuthor();
        conn.execute(
            "INSERT INTO commits (timestamp, message, author) VALUES (?1, ?2, ?3)",
            params![timestamp as i64, message, author],
        )?;
        let commitId: i64 = conn.last_insert_rowid();

        let cwd = env::current_dir()?;
        let ignoreSet = buildIgnoreSet(&cwd);
        let files = listFilesRecursive(&cwd, &cwd, &ignoreSet);

        let tx = conn.transaction()?;
        let mut totalBytes: u64 = 0;
        let mut totalFiles: usize = 0;

        for file in files {
            if file
                .components()
                .any(|component| component.as_os_str() == ".genie")
            {
                continue;
            }
            if let Ok(relative) = file.strip_prefix(&cwd) {
                if let Ok(metadata) = fs::metadata(&file) {
                    let fileSize = metadata.len() as i64;
                    let lastModified = metadata
                        .modified()
                        .ok()
                        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs() as i64)
                        .unwrap_or(0);
                    let filePath = relative.to_string_lossy().to_string();
                    tx.execute(
                        "INSERT INTO commit_files (commit_id, file_path, file_size, last_modified) VALUES (?1, ?2, ?3, ?4)",
                        params![commitId, filePath, fileSize, lastModified],
                    )?;

                    if let Some(fileHash) = computeFileHash(&file) {
                        tx.execute(
                            "INSERT OR REPLACE INTO commit_file_hashes (commit_id, file_path, file_hash) VALUES (?1, ?2, ?3)",
                            params![commitId, relative.to_string_lossy().to_string(), fileHash],
                        )?;
                    }

                    totalBytes += fileSize as u64;
                    totalFiles += 1;
                }
            }
        }

        tx.commit()?;
        updateHeadPointer(commitId);

        let complexityScore = (totalFiles as u64 * 7) + (totalBytes / 1024);
        println!(
            "✅ Commit {} — \"{}\" ({} files, {}, score {})",
            commitId,
            message,
            totalFiles,
            formatBytes(totalBytes),
            complexityScore
        );
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
    Ok(())
}

fn createStatusSnapshot(deepCompare: bool) -> Option<StatusSnapshot> {
    let cwd = env::current_dir().ok()?;
    let genieDir = cwd.join(".genie");
    if !genieDir.exists() {
        return None;
    }

    let projectPath = cwd.to_string_lossy().to_string();
    let conn = openHistory()?;

    let commitCount: i64 = conn
        .query_row("SELECT COUNT(*) FROM commits", [], |row| row.get(0))
        .unwrap_or(0);

    let lastCommitId: Option<i64> = conn
        .query_row(
            "SELECT id FROM commits ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let mut trackedFiles: HashMap<String, FileMeta> = HashMap::new();
    let mut trackedHashes: HashMap<String, String> = HashMap::new();
    let mut trackedBytes: u64 = 0;

    if let Some(commitId) = lastCommitId {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT file_path, file_size, last_modified FROM commit_files WHERE commit_id = ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![commitId], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            }) {
                for row in rows.flatten() {
                    trackedBytes += row.1 as u64;
                    trackedFiles.insert(
                        row.0,
                        FileMeta {
                            size: row.1,
                            modified: row.2,
                        },
                    );
                }
            }
        }

        if let Ok(mut stmt) =
            conn.prepare("SELECT file_path, file_hash FROM commit_file_hashes WHERE commit_id = ?1")
        {
            if let Ok(rows) = stmt.query_map(params![commitId], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    trackedHashes.insert(row.0, row.1);
                }
            }
        }
    }

    let ignoreSet = buildIgnoreSet(&cwd);
    let discoveredFiles = listFilesRecursive(&cwd, &cwd, &ignoreSet);
    let mut actualFiles: HashMap<String, ActualFile> = HashMap::new();

    for file in discoveredFiles {
        if file
            .components()
            .any(|component| component.as_os_str() == ".genie")
        {
            continue;
        }
        if let Ok(relative) = file.strip_prefix(&cwd) {
            if let Ok(metadata) = fs::metadata(&file) {
                let fileSize = metadata.len() as i64;
                let lastModified = metadata
                    .modified()
                    .ok()
                    .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs() as i64)
                    .unwrap_or(0);
                let relativePath = relative.to_string_lossy().to_string();
                actualFiles.insert(
                    relativePath.clone(),
                    ActualFile {
                        absolutePath: file.clone(),
                        meta: FileMeta {
                            size: fileSize,
                            modified: lastModified,
                        },
                    },
                );
            }
        }
    }

    let mut untracked = Vec::new();
    let mut missing = Vec::new();
    let mut modified = Vec::new();

    for (path, _) in &actualFiles {
        if !trackedFiles.contains_key(path) {
            untracked.push(path.clone());
        }
    }

    for (path, _) in &trackedFiles {
        if !actualFiles.contains_key(path) {
            missing.push(path.clone());
        }
    }

    for (path, actual) in &actualFiles {
        if let Some(previous) = trackedFiles.get(path) {
            let mut reasons: Vec<&str> = Vec::new();
            if previous.size != actual.meta.size {
                reasons.push("size changed");
            }
            if previous.modified != actual.meta.modified {
                reasons.push("timestamp changed");
            }

            if reasons.is_empty() && deepCompare {
                if let Some(expectedHash) = trackedHashes.get(path) {
                    if let Some(currentHash) = computeFileHash(&actual.absolutePath) {
                        if &currentHash != expectedHash {
                            reasons.push("content hash changed");
                        }
                    }
                }
            }

            if !reasons.is_empty() {
                let reasonText = reasons.join(", ");
                modified.push(FileChange {
                    path: path.clone(),
                    reason: reasonText,
                });
            }
        }
    }

    Some(StatusSnapshot {
        projectPath,
        commitCount,
        lastCommitId,
        trackedFiles: trackedFiles.len(),
        trackedBytes,
        modified,
        untracked,
        missing,
    })
}

fn formatBytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unitIndex = 0;
    while value >= 1024.0 && unitIndex < UNITS.len() - 1 {
        value /= 1024.0;
        unitIndex += 1;
    }
    if unitIndex == 0 {
        format!("{} {}", bytes, UNITS[unitIndex])
    } else {
        format!("{:.1} {}", value, UNITS[unitIndex])
    }
}

fn formatTimestamp(ts: i64) -> String {
    if let Some(dt) = DateTime::<Utc>::from_timestamp(ts, 0) {
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    } else {
        ts.to_string()
    }
}

impl StatusSnapshot {
    fn digestSignature(&self) -> String {
        let mut modifiedPaths: Vec<String> = self.modified.iter().map(|m| m.path.clone()).collect();
        modifiedPaths.sort();
        let mut untrackedPaths = self.untracked.clone();
        untrackedPaths.sort();
        let mut missingPaths = self.missing.clone();
        missingPaths.sort();
        format!(
            "{}|{}|{}|{}|{}|{}",
            self.commitCount,
            self.lastCommitId.unwrap_or(0),
            modifiedPaths.join(","),
            untrackedPaths.join(","),
            missingPaths.join(","),
            self.trackedFiles
        )
    }
}

pub fn showStatus(options: StatusOptions) {
    if let Some(snapshot) = createStatusSnapshot(options.deepCompare) {
        if options.jsonMode {
            if let Ok(serialized) = serde_json::to_string_pretty(&snapshot) {
                println!("{}", serialized);
            }
        } else {
            printStatusText(&snapshot, options.includeFiles);
        }
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
}

fn printStatusText(snapshot: &StatusSnapshot, includeFiles: bool) {
    println!("📂 Project: {}", snapshot.projectPath);
    println!(
        "✔ Commits: {} | Tracked files: {} | Footprint: {}",
        snapshot.commitCount,
        snapshot.trackedFiles,
        formatBytes(snapshot.trackedBytes)
    );

    if snapshot.modified.is_empty() && snapshot.untracked.is_empty() && snapshot.missing.is_empty()
    {
        println!("✨ Workspace is clean.");
        return;
    }

    if snapshot.modified.is_empty() {
        println!("Modified files: none");
    } else {
        println!("Modified files: {}", snapshot.modified.len());
        if includeFiles {
            for change in &snapshot.modified {
                println!("  ~ {} ({})", change.path, change.reason);
            }
        }
    }

    if snapshot.untracked.is_empty() {
        println!("Untracked files: none");
    } else {
        println!("Untracked files: {}", snapshot.untracked.len());
        if includeFiles {
            for file in &snapshot.untracked {
                println!("  + {}", file);
            }
        }
    }

    if snapshot.missing.is_empty() {
        println!("Missing files: none");
    } else {
        println!("Missing files: {}", snapshot.missing.len());
        if includeFiles {
            for file in &snapshot.missing {
                println!("  - {}", file);
            }
        }
    }
}

pub async fn watchStatus(intervalSeconds: u64, includeFiles: bool, deepCompare: bool) {
    let intervalSeconds = intervalSeconds.max(1);
    println!(
        "👀 Watching for changes every {}s (Ctrl+C to exit)...",
        intervalSeconds
    );
    let mut ticker = interval(Duration::from_secs(intervalSeconds));
    let mut lastDigest: Option<String> = None;

    loop {
        select! {
            _ = ticker.tick() => {
                if let Some(snapshot) = createStatusSnapshot(deepCompare) {
                    let digest = snapshot.digestSignature();
                    if lastDigest.as_ref() != Some(&digest) {
                        printStatusText(&snapshot, includeFiles);
                        lastDigest = Some(digest);
                    }
                } else {
                    println!("❌ No Genie repo found. Run `genie init` first.");
                    break;
                }
            }
            _ = signal::ctrl_c() => {
                println!("\n🛑 Watch mode stopped.");
                break;
            }
        }
    }
}

pub fn showLog(options: LogOptions) {
    if let Some(conn) = openHistory() {
        let mut stmt = conn
            .prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id DESC LIMIT ?1")
            .unwrap();
        let rows = stmt
            .query_map(params![options.limit as i64], |row| {
                Ok(CommitSummary {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    message: row.get(2)?,
                    author: row.get(3)?,
                })
            })
            .unwrap();
        let mut commits: Vec<CommitSummary> = rows.filter_map(Result::ok).collect();
        commits.reverse();

        if options.jsonMode {
            if let Ok(serialized) = serde_json::to_string_pretty(&commits) {
                println!("{}", serialized);
            }
        } else {
            if commits.is_empty() {
                println!("📝 No commits yet.");
            } else {
                println!("📝 Showing {} commit(s):", commits.len());
                for commit in commits {
                    println!(
                        "  #{} {} {} :: {}",
                        commit.id,
                        formatTimestamp(commit.timestamp),
                        commit.author,
                        commit.message
                    );
                }
            }
        }
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
}

pub fn generateCompletions(shell: &str) {
    use clap_complete::{generate, shells};
    let mut cmd = Cli::command();
    match shell {
        "bash" => generate(shells::Bash, &mut cmd, "genie", &mut std::io::stdout()),
        "zsh" => generate(shells::Zsh, &mut cmd, "genie", &mut std::io::stdout()),
        "fish" => generate(shells::Fish, &mut cmd, "genie", &mut std::io::stdout()),
        _ => eprintln!("Unsupported shell: {}", shell),
    }
}

pub fn printMan() {
    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Vec::new();
    if man.render(&mut buffer).is_ok() {
        let _ = std::io::stdout().write_all(&buffer);
    }
}

pub fn doSelfUpdate() {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("imisbahk")
        .repo_name("genie")
        .bin_name("genie")
        .show_download_progress(true)
        .no_confirm(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build();

    match status {
        Ok(updater) => match updater.update() {
            Ok(result) => println!("Updated to {}", result.version()),
            Err(err) => eprintln!("Self-update failed: {}", err),
        },
        Err(err) => eprintln!("Failed to configure updater: {}", err),
    }
}

pub async fn startUiServer(port: Option<u16>) {
    let port = port.unwrap_or(2718);
    let commitsRoute = warp::path!("api" / "commits").and(warp::get()).map(|| {
        if let Some(conn) = openHistory() {
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

    let filesRoute = warp::path!("api" / "files")
        .and(warp::get())
        .map(|| {
            if let Some(conn) = openHistory() {
                let lastCommitId: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM commits ORDER BY id DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(commitId) = lastCommitId {
                    let mut stmt = conn
                        .prepare("SELECT file_path, file_size, last_modified FROM commit_files WHERE commit_id = ?1")
                        .unwrap();
                    let rows = stmt
                        .query_map(params![commitId], |row| {
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

    let insightsRoute = warp::path!("api" / "insights").and(warp::get()).map(|| {
        if let Some(report) = buildInsightReport(5) {
            warp::reply::json(&report)
        } else {
            warp::reply::json(&json!({ "error": "no repo" }))
        }
    });

    let projectsRoute = warp::path!("api" / "projects").and(warp::get()).map(|| {
        let entries = loadRegistry();
        let out: Vec<_> = entries
            .into_iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "path": entry.path,
                    "created_at": entry.created_at_unix,
                })
            })
            .collect();
        warp::reply::json(&out)
    });

    let projectCommits = warp::path!("api" / "project" / String / "commits")
        .and(warp::get())
        .map(|name: String| {
            let entries = loadRegistry();
            if let Some(entry) = entries.into_iter().find(|entry| entry.name == name) {
                if let Some(conn) = openHistoryForPath(&entry.path) {
                    let mut stmt = conn
                        .prepare(
                            "SELECT id, timestamp, message, author FROM commits ORDER BY id ASC",
                        )
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

    let projectFiles = warp::path!("api" / "project" / String / "files")
        .and(warp::get())
        .map(|name: String| {
            let entries = loadRegistry();
            if let Some(entry) = entries.into_iter().find(|entry| entry.name == name) {
                if let Some(conn) = openHistoryForPath(&entry.path) {
                    let lastCommitId: Option<i64> = conn
                        .query_row(
                            "SELECT id FROM commits ORDER BY id DESC LIMIT 1",
                            [],
                            |row| row.get(0),
                        )
                        .ok();
                    if let Some(commitId) = lastCommitId {
                        let mut stmt = conn
                            .prepare("SELECT file_path, file_size, last_modified FROM commit_files WHERE commit_id = ?1")
                            .unwrap();
                        let rows = stmt
                            .query_map(params![commitId], |row| {
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

    let indexRedirect =
        warp::path::end().map(|| warp::redirect::temporary(Uri::from_static("/main.html")));
    let projectRedirect = warp::path!("project" / String)
        .and(warp::path::end())
        .map(|_name: String| warp::redirect::temporary(Uri::from_static("/main.html")));

    let staticFiles = warp::fs::dir("ui");
    let routes = commitsRoute
        .or(filesRoute)
        .or(insightsRoute)
        .or(projectsRoute)
        .or(projectCommits)
        .or(projectFiles)
        .or(indexRedirect)
        .or(projectRedirect)
        .or(staticFiles);
    println!("🚀 Starting Genie UI server at http://localhost:{}", port);
    warp::serve(routes).run(([127, 0, 0, 1], port)).await;
}

fn buildInsightReport(topFiles: usize) -> Option<InsightReport> {
    let snapshot = createStatusSnapshot(false)?;
    let mut conn = openHistory()?;
    let commitCount = snapshot.commitCount;

    let mut firstCommit: Option<i64> = None;
    let mut lastCommit: Option<i64> = None;
    if commitCount > 0 {
        firstCommit = conn
            .query_row(
                "SELECT timestamp FROM commits ORDER BY id ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
        lastCommit = conn
            .query_row(
                "SELECT timestamp FROM commits ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();
    }

    let activeDays = match (firstCommit, lastCommit) {
        (Some(first), Some(last)) if last >= first => {
            let duration = (last - first) as u64;
            duration / 86_400 + 1
        }
        _ => 0,
    };

    let avgCommitsPerDay = if activeDays == 0 {
        0.0
    } else {
        commitCount as f64 / activeDays as f64
    };

    let mut topTouched: Vec<FileTouchStat> = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT file_path, COUNT(*) AS touches FROM commit_files GROUP BY file_path ORDER BY touches DESC LIMIT ?1",
    ) {
        if let Ok(rows) = stmt.query_map(params![topFiles as i64], |row| {
            Ok(FileTouchStat {
                path: row.get(0)?,
                touches: row.get(1)?,
            })
        }) {
            topTouched = rows.filter_map(Result::ok).collect();
        }
    }

    let mut largestFiles: Vec<FileSizeStat> = Vec::new();
    if let Some(commitId) = snapshot.lastCommitId {
        if let Ok(mut stmt) = conn.prepare(
            "SELECT file_path, file_size FROM commit_files WHERE commit_id = ?1 ORDER BY file_size DESC LIMIT ?2",
        ) {
            if let Ok(rows) = stmt.query_map(params![commitId, topFiles as i64], |row| {
                Ok(FileSizeStat {
                    path: row.get(0)?,
                    bytes: row.get(1)?,
                })
            }) {
                largestFiles = rows.filter_map(Result::ok).collect();
            }
        }
    }

    let recentCommits = gatherRecentCommits(&mut conn, 5);
    let fileTypes = gatherFileTypeStats(&mut conn, snapshot.lastCommitId);

    Some(InsightReport {
        projectPath: snapshot.projectPath,
        commitCount,
        firstCommit,
        lastCommit,
        activeDays,
        avgCommitsPerDay,
        topFiles: topTouched,
        largestFiles,
        recentCommits,
        fileTypes,
        trackedFiles: snapshot.trackedFiles,
        trackedBytes: snapshot.trackedBytes,
    })
}

fn gatherRecentCommits(conn: &mut Connection, limit: usize) -> Vec<CommitSummary> {
    if let Ok(mut stmt) =
        conn.prepare("SELECT id, timestamp, message, author FROM commits ORDER BY id DESC LIMIT ?1")
    {
        if let Ok(rows) = stmt.query_map(params![limit as i64], |row| {
            Ok(CommitSummary {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                message: row.get(2)?,
                author: row.get(3)?,
            })
        }) {
            return rows.filter_map(Result::ok).collect();
        }
    }
    Vec::new()
}

fn gatherFileTypeStats(conn: &mut Connection, commitId: Option<i64>) -> Vec<FileTypeStat> {
    if let Some(commitId) = commitId {
        if let Ok(mut stmt) =
            conn.prepare("SELECT file_path, file_size FROM commit_files WHERE commit_id = ?1")
        {
            if let Ok(rows) = stmt.query_map(params![commitId], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            }) {
                let mut buckets: BTreeMap<String, (usize, u64)> = BTreeMap::new();
                for row in rows.flatten() {
                    let extension = Path::new(&row.0)
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase())
                        .unwrap_or_else(|| "<none>".to_string());
                    let entry = buckets.entry(extension).or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 += row.1;
                }
                return buckets
                    .into_iter()
                    .map(|(extension, (files, bytes))| FileTypeStat {
                        extension,
                        files,
                        bytes,
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

pub fn showInsights(jsonMode: bool, topFiles: usize) {
    if let Some(report) = buildInsightReport(topFiles) {
        if jsonMode {
            if let Ok(serialized) = serde_json::to_string_pretty(&report) {
                println!("{}", serialized);
            }
            return;
        }

        println!("📊 Repository insights for {}", report.projectPath);
        println!(
            "Commits: {} | Active days: {} | Avg commits/day: {:.2}",
            report.commitCount, report.activeDays, report.avgCommitsPerDay
        );
        if let Some(first) = report.firstCommit {
            println!("First commit: {}", formatTimestamp(first));
        }
        if let Some(last) = report.lastCommit {
            println!("Latest commit: {}", formatTimestamp(last));
        }
        println!(
            "Tracked footprint: {} files ({})",
            report.trackedFiles,
            formatBytes(report.trackedBytes)
        );

        if !report.topFiles.is_empty() {
            println!("Top touched files:");
            for stat in &report.topFiles {
                println!("  {} ({} touches)", stat.path, stat.touches);
            }
        }

        if !report.largestFiles.is_empty() {
            println!("Largest files:");
            for stat in &report.largestFiles {
                println!("  {} ({})", stat.path, formatBytes(stat.bytes as u64));
            }
        }

        if !report.fileTypes.is_empty() {
            println!("File type mix:");
            for stat in &report.fileTypes {
                println!(
                    "  .{} -> {} files ({})",
                    stat.extension,
                    stat.files,
                    formatBytes(stat.bytes)
                );
            }
        }

        if !report.recentCommits.is_empty() {
            println!("Recent commits:");
            for commit in &report.recentCommits {
                println!(
                    "  #{} {} {} :: {}",
                    commit.id,
                    formatTimestamp(commit.timestamp),
                    commit.author,
                    commit.message
                );
            }
        }
    } else {
        println!("❌ No Genie repo found. Run `genie init` first.");
    }
}

pub fn runGuard(maxFileMb: u64, jsonMode: bool, strictMode: bool) {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let thresholdBytes = maxFileMb.max(1) * 1024 * 1024;
    let ignoreSet = buildIgnoreSet(&cwd);
    let files = listFilesRecursive(&cwd, &cwd, &ignoreSet);
    let mut findings: Vec<GuardFinding> = Vec::new();

    let secretPatterns = vec![
        ("GitHub token", Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap()),
        (
            "AWS secret",
            Regex::new(r"(?i)aws_secret_access_key\s*=\s*[A-Za-z0-9/+]{40}").unwrap(),
        ),
        (
            "Slack token",
            Regex::new(r"xox[baprs]-[A-Za-z0-9-]{10,48}").unwrap(),
        ),
        (
            "Private key",
            Regex::new(r"-----BEGIN (RSA|OPENSSH) PRIVATE KEY-----").unwrap(),
        ),
        (
            "Generic secret",
            Regex::new(r#"(?i)(api_key|secret|token)\s*=\s*['"]?[A-Za-z0-9_\-]{20,}"#).unwrap(),
        ),
    ];

    for file in files {
        if file
            .components()
            .any(|component| component.as_os_str() == ".genie")
        {
            continue;
        }
        if let Ok(metadata) = fs::metadata(&file) {
            let relative = file.strip_prefix(&cwd).unwrap_or(&file);
            let pathString = relative.to_string_lossy().to_string();
            let fileSize = metadata.len();

            if fileSize > thresholdBytes {
                findings.push(GuardFinding {
                    severity: if fileSize > (thresholdBytes * 2) {
                        "critical".to_string()
                    } else {
                        "warning".to_string()
                    },
                    file: pathString.clone(),
                    category: "large-file".to_string(),
                    message: format!(
                        "File is {} which exceeds {} MB",
                        formatBytes(fileSize),
                        maxFileMb
                    ),
                    hint: "Consider ignoring or splitting this file".to_string(),
                });
            }

            if strictMode {
                if let Some(name) = file.file_name().and_then(|name| name.to_str()) {
                    if name.starts_with(".env") || name.ends_with(".pem") {
                        findings.push(GuardFinding {
                            severity: "warning".to_string(),
                            file: pathString.clone(),
                            category: "sensitive-config".to_string(),
                            message: "Sensitive configuration file detected".to_string(),
                            hint: "Ensure this file is encrypted or ignored".to_string(),
                        });
                    }
                }
            }

            if metadata.is_file() && fileSize <= 2 * 1024 * 1024 {
                if let Ok(mut handle) = fs::File::open(&file) {
                    let mut buffer = Vec::new();
                    if handle.read_to_end(&mut buffer).is_ok() {
                        if isLikelyText(&buffer) {
                            if let Ok(content) = String::from_utf8(buffer) {
                                for (label, pattern) in &secretPatterns {
                                    if pattern.is_match(&content) {
                                        findings.push(GuardFinding {
                                            severity: "critical".to_string(),
                                            file: pathString.clone(),
                                            category: "secret-leak".to_string(),
                                            message: format!("{} detected", label),
                                            hint:
                                                "Rotate the credential and remove it from history"
                                                    .to_string(),
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if jsonMode {
        if let Ok(serialized) = serde_json::to_string_pretty(&findings) {
            println!("{}", serialized);
        }
    } else if findings.is_empty() {
        println!("🛡️  Guard check passed. No suspicious files detected.");
    } else {
        println!("🛡️  Guard raised {} issue(s):", findings.len());
        for finding in &findings {
            println!(
                "  [{}] {} - {} ({})",
                finding.severity, finding.file, finding.message, finding.hint
            );
        }
    }
}

fn isLikelyText(bytes: &[u8]) -> bool {
    if bytes.iter().take(1024).any(|b| *b == 0) {
        return false;
    }
    let printable = bytes
        .iter()
        .filter(|b| {
            b.is_ascii_graphic() || **b == b'\n' || **b == b'\r' || **b == b'\t' || **b == b' '
        })
        .count();
    printable * 100 / bytes.len().max(1) > 80
}

pub fn showProjects(jsonMode: bool, includeDetails: bool) {
    let entries = loadRegistry();
    if entries.is_empty() {
        println!("📚 Registry is empty. Run 'genie init' inside projects to register them.");
        return;
    }

    let mut summaries: Vec<ProjectSummary> = Vec::new();

    for entry in entries {
        let mut summary = ProjectSummary {
            name: entry.name.clone(),
            path: entry.path.clone(),
            createdAt: entry.created_at_unix,
            commitCount: None,
            lastCommitTimestamp: None,
        };

        if includeDetails {
            if let Some(conn) = openHistoryForPath(&entry.path) {
                summary.commitCount = conn
                    .query_row("SELECT COUNT(*) FROM commits", [], |row| row.get(0))
                    .ok();
                summary.lastCommitTimestamp = conn
                    .query_row(
                        "SELECT timestamp FROM commits ORDER BY id DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .ok();
            }
        }

        summaries.push(summary);
    }

    if jsonMode {
        if let Ok(serialized) = serde_json::to_string_pretty(&summaries) {
            println!("{}", serialized);
        }
        return;
    }

    println!("📚 Registered projects:");
    for summary in &summaries {
        let commitInfo = summary
            .commitCount
            .map(|count| format!("{} commits", count))
            .unwrap_or_else(|| "unknown commits".to_string());
        let lastInfo = summary
            .lastCommitTimestamp
            .map(|ts| formatTimestamp(ts))
            .unwrap_or_else(|| "n/a".to_string());
        println!(
            "  {} - {} ({}, last {})",
            summary.name, summary.path, commitInfo, lastInfo
        );
    }
}
