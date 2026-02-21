# @imisbahk/genie

Standalone GENIE CLI for codebase indexing, symbol search, dependency lookup, and MCP server integration.

## Install

```bash
npm i @imisbahk/genie
```

```bash
bun install @imisbahk/genie
```

```bash
pnpm install @imisbahk/genie
```

Or run locally from this folder:

```bash
npm install
npm run build
node ./bin/genie.js --help
```

## Quick Start

Index a project:

```bash
genie index
```

Search for symbols:

```bash
genie search handleSubmit
```

Show file dependencies:

```bash
genie deps src/api/posts.ts
```

Start MCP server over stdio:

```bash
genie serve --stdio --project /path/to/project --auto-index
```

## Commands

- `index [path]`
- `search <query>`
- `deps <file>`
- `dependents <file>`
- `info <file>`
- `summary`
- `status`
- `clear`
- `serve`

Run `genie --help` for all options.

## Project Layout

- `src/`: CLI command handlers and output formatting
- `common/`: shared service/types used by the CLI runtime
- `node/`: Node service and native binding loader
- `mcp/`: MCP server implementation
- `bin/`: executable entrypoint
- `scripts/`: packaging helpers

## Notes

- This package is fully standalone and can be moved independently of the parent repo.
- Native binary in this workspace is currently `darwin-arm64`; rebuild for other platforms if needed.
