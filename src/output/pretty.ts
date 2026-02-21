import chalk from 'chalk';
import { FileInfoOutput, ProjectSummaryOutput, SymbolRow } from './types';

export const prettyOutput = {
  searchResults(query: string, results: SymbolRow[]): void {
    if (results.length === 0) {
      console.log(chalk.yellow(`\nNo symbols found for "${query}"\n`));
      return;
    }

    console.log(chalk.bold(`\nFound ${results.length} symbol${results.length !== 1 ? 's' : ''}:\n`));

    const grouped = groupByFile(results);
    for (const [file, symbols] of Object.entries(grouped)) {
      console.log(chalk.cyan(`📄 ${file}`));
      for (const symbol of symbols) {
        console.log(`  ${symbol.name} ${chalk.gray(`(${symbol.kind})`)} - line ${symbol.line}`);
      }
      console.log();
    }
  },

  dependencies(file: string, deps: string[]): void {
    console.log(chalk.bold(`\nDependencies of ${file}:\n`));

    if (deps.length === 0) {
      console.log(chalk.gray('  No dependencies'));
    } else {
      for (const dep of deps) {
        console.log(`  ${chalk.cyan('→')} ${dep}`);
      }
    }

    console.log();
  },

  dependents(file: string, dependents: string[]): void {
    console.log(chalk.bold(`\nFiles that depend on ${file}:\n`));

    if (dependents.length === 0) {
      console.log(chalk.gray('  No dependents (file is not imported anywhere)'));
    } else {
      for (const dependent of dependents) {
        console.log(`  ${chalk.cyan('←')} ${dependent}`);
      }
    }

    console.log();
  },

  fileInfo(info: FileInfoOutput): void {
    console.log(chalk.bold(`\nFile: ${info.path}\n`));
    console.log(`Language:     ${chalk.green(info.language || 'Unknown')}`);
    if (typeof info.size === 'number') {
      console.log(`Size:         ${chalk.green(formatBytes(info.size))}`);
    }
    if (typeof info.lastIndexed === 'number') {
      console.log(`Last indexed: ${chalk.green(formatRelativeTime(info.lastIndexed))}`);
    }

    if (info.symbols && info.symbols.length > 0) {
      console.log(chalk.bold('\nSymbols:'));
      for (const symbol of info.symbols) {
        console.log(`  • ${symbol.name} ${chalk.gray(`(${symbol.kind})`)} - line ${symbol.line}`);
      }
    }

    if (info.imports && info.imports.length > 0) {
      console.log(chalk.bold('\nImports:'));
      for (const imported of info.imports) {
        console.log(`  ${chalk.cyan('→')} ${imported}`);
      }
    }

    if (info.exports && info.exports.length > 0) {
      console.log(chalk.bold('\nExports:'));
      for (const exported of info.exports) {
        console.log(`  • ${exported}`);
      }
    }

    if (info.dependents && info.dependents.length > 0) {
      console.log(chalk.bold('\nUsed by:'));
      for (const dependent of info.dependents) {
        console.log(`  ${chalk.cyan('←')} ${dependent}`);
      }
    }

    console.log();
  },

  summary(stats: ProjectSummaryOutput): void {
    console.log(chalk.bold('\n📊 Project Summary\n'));
    console.log(`Files:        ${chalk.green(stats.totalFiles.toLocaleString())}`);
    console.log(`Symbols:      ${chalk.green(stats.totalSymbols.toLocaleString())}`);
    console.log(`Dependencies: ${chalk.green(stats.totalDependencies.toLocaleString())}`);

    if (stats.languages && Object.keys(stats.languages).length > 0) {
      console.log(`Languages:    ${formatLanguages(stats.languages)}`);
    }

    if (typeof stats.lastIndexed === 'number') {
      console.log(`\nLast indexed: ${chalk.gray(formatRelativeTime(stats.lastIndexed))}`);
    }

    if (typeof stats.indexSize === 'number') {
      console.log(`Index size:   ${chalk.gray(formatBytes(stats.indexSize))}`);
    }

    console.log();
  },

  status(status: {
    running: boolean;
    totalFiles: number;
    completedFiles: number;
  }): void {
    if (status.running) {
      const percent = status.totalFiles > 0
        ? Math.round((status.completedFiles / status.totalFiles) * 100)
        : 0;
      console.log(chalk.bold('\n⏳ Indexing in progress...\n'));
      console.log(
        `Progress: ${chalk.green(status.completedFiles.toLocaleString())} / ${status.totalFiles.toLocaleString()} files (${percent}%)`
      );
    } else {
      console.log(chalk.bold.green('\n✓ Index is ready\n'));
    }
  },
};

function groupByFile(results: SymbolRow[]): Record<string, SymbolRow[]> {
  return results.reduce<Record<string, SymbolRow[]>>((acc, item) => {
    if (!acc[item.filePath]) {
      acc[item.filePath] = [];
    }
    acc[item.filePath].push(item);
    return acc;
  }, {});
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = Math.max(0, now - timestamp);
  const seconds = Math.floor(diff / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) {
    return `${days} day${days !== 1 ? 's' : ''} ago`;
  }
  if (hours > 0) {
    return `${hours} hour${hours !== 1 ? 's' : ''} ago`;
  }
  if (minutes > 0) {
    return `${minutes} minute${minutes !== 1 ? 's' : ''} ago`;
  }
  return 'just now';
}

function formatLanguages(languages: Record<string, number>): string {
  const total = Object.values(languages).reduce((sum, value) => sum + value, 0);
  const sorted = Object.entries(languages).sort((a, b) => b[1] - a[1]);

  return sorted
    .map(([language, count]) => {
      if (total <= 0) {
        return `${language} (${count})`;
      }
      return `${language} (${Math.round((count / total) * 100)}%)`;
    })
    .join(', ');
}
