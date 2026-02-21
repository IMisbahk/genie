const fs = require('node:fs');
const path = require('node:path');

const source = path.resolve(__dirname, '..', 'node', 'genie-core.node');
const destinationDir = path.resolve(__dirname, '..', 'dist', 'node');
const destination = path.join(destinationDir, 'genie-core.node');

if (!fs.existsSync(source)) {
  console.error(`Missing native binary: ${source}`);
  process.exit(1);
}

fs.mkdirSync(destinationDir, { recursive: true });
fs.copyFileSync(source, destination);
console.log(`Copied native binary to ${destination}`);
