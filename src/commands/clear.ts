import { rmSync, existsSync } from 'node:fs';
import readline from 'node:readline/promises';
import { stdin as input, stdout as output } from 'node:process';
import { logger } from '../utils/logger';
import { indexDirPath } from '../utils/paths';

interface ClearOptions {
  force?: boolean;
}

export async function clearCommand(options: ClearOptions = {}): Promise<void> {
  const indexDir = indexDirPath(process.cwd());

  if (!existsSync(indexDir)) {
    logger.info('No index found');
    return;
  }

  if (!options.force) {
    const confirmed = await confirm('⚠️  This will delete the index. Continue?');
    if (!confirmed) {
      logger.info('Cancelled');
      return;
    }
  }

  try {
    rmSync(indexDir, { recursive: true, force: true });
    logger.success('Index cleared');
  } catch (error) {
    logger.error(`Failed to clear index: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

async function confirm(message: string): Promise<boolean> {
  const rl = readline.createInterface({ input, output });
  try {
    const answer = await rl.question(`${message} (y/N) `);
    return answer.trim().toLowerCase() === 'y';
  } finally {
    rl.close();
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
