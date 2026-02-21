import chokidar from 'chokidar';
import { GenieNodeService } from '../../node/genieNodeService';
import { logger } from '../utils/logger';
import { spinner } from '../utils/spinner';
import { createGenieService } from '../utils/service';
import { normalizeProjectPath } from '../utils/paths';

interface IndexOptions {
  watch?: boolean;
  quiet?: boolean;
}

export async function indexCommand(targetPath = process.cwd(), options: IndexOptions = {}): Promise<void> {
  const projectPath = normalizeProjectPath(targetPath);
  const quiet = Boolean(options.quiet);
  let genie: GenieNodeService | undefined;

  try {
    genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    if (!quiet) {
      spinner.start('Indexing project...');
    }

    const started = Date.now();
    const indexResult = await genie.indexWorkspace();
    const durationMs = indexResult?.durationMs ?? Date.now() - started;
    const summary = genie.getProjectSummary();

    if (!quiet) {
      spinner.succeed(
        `Indexed ${summary.totalFiles.toLocaleString()} files in ${(durationMs / 1000).toFixed(1)}s`
      );
      logger.info(
        `📊 Found ${summary.totalSymbols.toLocaleString()} symbols across ${summary.totalDependencies.toLocaleString()} dependencies`
      );
    }

    if (options.watch) {
      await startWatchMode(projectPath, genie, quiet);
      return;
    }

    genie.dispose();
  } catch (error) {
    if (spinner.isSpinning()) {
      spinner.fail('Indexing failed');
    }
    logger.error(`Failed to index: ${errorMessage(error)}`);
    genie?.dispose();
    process.exitCode = 1;
  }
}

async function startWatchMode(projectPath: string, genie: GenieNodeService, quiet: boolean): Promise<void> {
  if (!quiet) {
    logger.info('👀 Watching for file changes...');
  }

  const watcher = chokidar.watch(projectPath, {
    ignored: ['**/node_modules/**', '**/.git/**', '**/.genie/**', '**/.rocket/**'],
    persistent: true,
    ignoreInitial: true,
  });

  let debounceTimer: NodeJS.Timeout | undefined;
  let running = false;
  let pending = false;

  const schedule = (): void => {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }

    debounceTimer = setTimeout(() => {
      void reindex();
    }, 700);
  };

  const reindex = async (): Promise<void> => {
    if (running) {
      pending = true;
      return;
    }

    running = true;
    try {
      const started = Date.now();
      const result = await genie.indexWorkspace();
      const durationMs = result?.durationMs ?? Date.now() - started;
      if (!quiet) {
        logger.success(`Re-indexed in ${(durationMs / 1000).toFixed(1)}s`);
      }
    } catch (error) {
      logger.error(`Re-index failed: ${errorMessage(error)}`);
    } finally {
      running = false;
      if (pending) {
        pending = false;
        void reindex();
      }
    }
  };

  watcher.on('add', schedule);
  watcher.on('change', schedule);
  watcher.on('unlink', schedule);

  process.on('SIGINT', async () => {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    await watcher.close();
    genie.dispose();
    if (!quiet) {
      logger.info('Stopped watch mode');
    }
    process.exit(0);
  });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
