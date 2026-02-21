#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const primary = path.resolve(__dirname, '..', 'dist', 'src', 'cli.js');
const legacy = path.resolve(__dirname, '..', 'dist', 'cli', 'src', 'cli.js');
const entry = fs.existsSync(primary) ? primary : legacy;

if (!fs.existsSync(entry)) {
  console.error('GENIE CLI entrypoint not found. Run `npm --prefix geniecli run build` first.');
  process.exit(1);
}

require(entry);
