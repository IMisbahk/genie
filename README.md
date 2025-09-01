# Genie 🧞

Genie is a personal version control system that is lightweight, fast, and simple.
It does not hash files. Instead, it uses file metadata such as size and modification time to track changes.
This makes commit and status operations near-instant, even on large projects.

## Features
	•	Instant commits and status using file metadata
	•	Commit history stored in a lightweight SQLite database
	•	Support for .genieignore to skip files and folders
	•	Web-based UI dashboard available at http://localhost:2718
	•	Simple commands for initialization, committing, status, and logs

## Installation

Clone the repository and build with Cargo

```bash
git clone https://github.com/imisbahk/genie.git
cd genie
cargo build --release
```

The binary will be available at target/release/genie


## Usage

Initialize a new Genie project

```bash
genie init
```

Make a commit

```bash
genie commit -m "Initial commit"
```

Check status

```bash
genie status
```

View commit log

```bash
genie log
```

Launch the UI dashboard

```bash
genie ui
```

The UI will be available at http://localhost:2718

## Roadmap
	•	Commit complexity scoring (files changed, bytes changed)
	•	Hotspot file detection
	•	Space-efficient snapshots
	•	Experimental time-based branching

## License

MIT License © 2025 Misbah