# Genie 🧞

Genie is a lightweight, fast personal version control system.
It tracks file changes using metadata (size + mtime), making commits and status near‑instant even on large projects.
Comes with a simple web dashboard and a global registry of your projects.

## Features
	•	Instant status/commits via file metadata (no hashing)
	•	SQLite-backed commit history per project in .genie/
	•	Glob-based .genieignore (powered by globset)
	•	Web dashboard at http://localhost:2718 (or choose a port)
	•	Global registry of projects at ~/.genie/registry.json
	•	New commands for welcome, docs, completions, man page, and self-update

## Installation

Pick one:

1) One-line installer (from GitHub releases)

```bash
curl -fsSL https://raw.githubusercontent.com/imisbahk/genie/main/install.sh | bash
```

2) Cargo install (build from source)

```bash
cargo install --path .
```

3) Manual copy/symlink from local build

```bash
cargo build --release
sudo cp target/release/genie /usr/local/bin/genie
# or
sudo ln -s "$(pwd)/target/release/genie" /usr/local/bin/genie
```

## Usage

Initialize a new Genie project

```bash
genie init
```

Check status

```bash
genie status
```

Make a commit

```bash
genie commit -m "Initial commit"
```

View commit log

```bash
genie log
```

Launch the UI dashboard

```bash
genie ui               # default http://localhost:2718
# or choose a port
genie ui --port 3000
```

The UI will be available at http://localhost:<port>

Extras

```bash
# friendly welcome and quickstart
genie welcome

# open docs in your browser (also prints the URL)
genie docs

# print shell completions (bash|zsh|fish)
genie completions zsh > ~/.zsh/completions/_genie

# print the man page to stdout
genie man | man -l -

# update to the latest release (when published)
genie self-update
```

## Roadmap
	•	Commit complexity scoring (files changed, bytes changed)
	•	Hotspot file detection
	•	Space-efficient snapshots
	•	Experimental time-based branching

## License

MIT License 2025 Misbah