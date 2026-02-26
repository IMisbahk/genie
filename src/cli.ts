import { Command } from 'commander';
import { clearCommand } from './commands/clear';
import { dependentsCommand } from './commands/dependents';
import { depsCommand } from './commands/deps';
import { indexCommand } from './commands/index';
import { infoCommand } from './commands/info';
import { searchCommand } from './commands/search';
import { serveCommand } from './commands/serve';
import { statusCommand } from './commands/status';
import { summaryCommand } from './commands/summary';
import pkg from '../package.json' assert { type: 'json' };

const program = new Command();

program
  .name('genie')
  .description('GENIE - Universal Codebase Intelligence')
  .version(pkg.version);

program
  .command('index [path]')
  .description('Index a codebase')
  .option('--watch', 'Watch for changes')
  .option('--quiet', 'Suppress output')
  .action(indexCommand);

program
  .command('search <query>')
  .description('Search for symbols')
  .option('--json', 'Output as JSON')
  .action(searchCommand);

program
  .command('deps <file>')
  .description('Show dependencies')
  .option('--json', 'Output as JSON')
  .action(depsCommand);

program
  .command('dependents <file>')
  .description('Show dependents')
  .option('--json', 'Output as JSON')
  .action(dependentsCommand);

program
  .command('info <file>')
  .description('Get file information')
  .option('--json', 'Output as JSON')
  .action(infoCommand);

program
  .command('summary')
  .description('Project statistics')
  .option('--json', 'Output as JSON')
  .action(summaryCommand);

program
  .command('status')
  .description('Indexing status')
  .option('--json', 'Output as JSON')
  .action(statusCommand);

program
  .command('clear')
  .description('Clear index')
  .option('--force', 'Skip confirmation')
  .action(clearCommand);

program
  .command('serve')
  .description('Start MCP server')
  .option('--stdio', 'Use stdio transport (default)')
  .option('--http', 'Use HTTP transport')
  .option('--port <port>', 'HTTP port', '3000')
  .option('--project <path>', 'Project path', process.cwd())
  .option('--auto-index', 'Index before starting server')
  .option('--watch', 'Watch files and re-index on changes')
  .action(serveCommand);

program.addHelpText(
  'after',
  `
Examples:
  geniecli index               Index current directory
  geniecli search "User"       Find User symbol
  geniecli deps app.ts         Show what app.ts imports
  geniecli serve --stdio       Start MCP server for Claude Desktop

Learn more: https://rocket.com/docs/genie
`
);

void program.parseAsync(process.argv);
