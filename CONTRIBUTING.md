# Contributing

## Prerequisites

- Node.js 20+
- npm 10+
- Rust toolchain (only needed when rebuilding native components)

## Local Setup

```bash
npm install
npm run build
node ./bin/genie.js --help
```

## Development Workflow

1. Create a branch for your change.
2. Make targeted edits in `src/`, `common/`, `node/`, or `mcp/`.
3. Build and run basic checks locally.
4. Open a PR with a clear summary and test notes.

## Build and Smoke Test

```bash
npm run build
node ./bin/genie.js --version
node ./bin/genie.js --help
```

## Code Guidelines

- Keep CLI output stable and script-friendly (`--json` behavior should remain deterministic).
- Avoid breaking command names or argument semantics.
- Keep runtime dependencies minimal.
- Prefer small, focused changes.

## Sync Policy

This package contains local copies of shared modules (`common/`, `node/`, `mcp/`) to stay standalone.  
If you change shared behavior here, keep the parent integration package in sync as needed.

## Pull Request Checklist

- [ ] Build succeeds (`npm run build`)
- [ ] CLI boots (`node ./bin/genie.js --help`)
- [ ] Docs updated when behavior changes
- [ ] No unrelated file churn
