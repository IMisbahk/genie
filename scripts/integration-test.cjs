const { execSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

function run(cmd, opts = {}) {
  return execSync(cmd, { stdio: 'pipe', encoding: 'utf8', ...opts });
}

function main() {
  const repoRoot = path.resolve(__dirname, '..');
  const cliEntry = path.join(repoRoot, 'bin', 'genie.js');
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'genie-test-'));
  const projectDir = path.join(tmpDir, 'project');
  fs.mkdirSync(projectDir);

  const filePath = path.join(projectDir, 'index.ts');
  fs.writeFileSync(
    filePath,
    'export function GenieIntegrationSymbol() { return 42; }\n',
    'utf8',
  );

  // Build must have been run already (prepublishOnly or explicit build)
  console.log('Running genie index in', projectDir);
  run(`node ${JSON.stringify(cliEntry)} index . --quiet`, { cwd: projectDir });

  console.log('Running genie search');
  const output = run(
    `node ${JSON.stringify(cliEntry)} search GenieIntegrationSymbol --json`,
    { cwd: projectDir },
  );

  try {
    const results = JSON.parse(output);
    if (!Array.isArray(results) || results.length === 0) {
      throw new Error('Expected at least one search result');
    }
  } catch (err) {
    console.error('Integration test failed to parse or validate search results');
    console.error('Output:', output);
    throw err;
  }

  console.log('Integration test passed');
}

main();
