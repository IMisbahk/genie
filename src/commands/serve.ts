import chokidar, { FSWatcher } from 'chokidar';
import { GenieNodeService } from '../../node/genieNodeService';
import { GenieMCPServer } from '../../mcp/mcpServer';
import { logger } from '../utils/logger';
import { spinner } from '../utils/spinner';
import { createGenieService } from '../utils/service';
import { normalizeProjectPath } from '../utils/paths';

interface ServeOptions {
  stdio?: boolean;
  http?: boolean;
  port?: string;
  project?: string;
  autoIndex?: boolean;
  watch?: boolean;
}

export async function serveCommand(options: ServeOptions = {}): Promise<void> {
  const projectPath = normalizeProjectPath(options.project || process.cwd());
  const useHttp = Boolean(options.http);

  let watcher: FSWatcher | undefined;
  let genie: GenieNodeService | undefined;
  let mcpServer: GenieMCPServer | undefined;
  let shuttingDown = false;

  const serverLog = (message: string): void => {
    if (useHttp) {
      logger.info(message);
      return;
    }
    // stdio transport must keep stdout clean for protocol messages.
    console.error(message);
  };

  try {
    serverLog(`Starting GENIE MCP server for: ${projectPath}`);

    genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    if (options.autoIndex) {
      spinner.start('Indexing project...');
      const result = await genie.indexWorkspace();
      const durationMs = result?.durationMs ?? 0;
      spinner.succeed(`Indexed ${result?.indexedFiles?.toLocaleString() ?? '0'} files in ${(durationMs / 1000).toFixed(1)}s`);
    }

    mcpServer = new GenieMCPServer(genie);

    if (useHttp) {
      const parsedPort = Number.parseInt(options.port || '3000', 10);
      if (!Number.isFinite(parsedPort) || parsedPort <= 0) {
        throw new Error(`Invalid --port value: ${options.port}`);
      }

      const handle = await mcpServer.startSSEHttpServer({ port: parsedPort });
      logger.success(`MCP server ready on http://${handle.host}:${handle.port}${handle.ssePath}`);
    } else {
      await mcpServer.startStdio();
      serverLog('MCP server ready on stdio');
      serverLog('Waiting for connections...');
    }

    if (options.watch) {
      watcher = startWatchMode(projectPath, genie, serverLog);
    }

    const shutdown = async (): Promise<void> => {
      if (shuttingDown) {
        return;
      }
      shuttingDown = true;

      try {
        if (watcher) {
          await watcher.close();
        }
        if (mcpServer) {
          await mcpServer.close();
        }
      } finally {
        genie?.dispose();
      }
    };

    process.on('SIGINT', () => {
      void shutdown().finally(() => process.exit(0));
    });

    process.on('SIGTERM', () => {
      void shutdown().finally(() => process.exit(0));
    });
  } catch (error) {
    if (spinner.isSpinning()) {
      spinner.fail('Failed to start MCP server');
    }
    logger.error(`Failed to start server: ${errorMessage(error)}`);

    if (watcher) {
      await watcher.close();
    }
    if (mcpServer) {
      await mcpServer.close();
    }
    genie?.dispose();
    process.exitCode = 1;
  }
}

function startWatchMode(
  projectPath: string,
  genieService: GenieNodeService,
  log: (message: string) => void
): FSWatcher {
  log('Watching for file changes...');

  const watcher = chokidar.watch(projectPath, {
    ignored: ['**/node_modules/**', '**/.git/**', '**/.genie/**', '**/.rocket/**'],
    persistent: true,
    ignoreInitial: true,
  });

  let debounceTimer: NodeJS.Timeout | undefined;
  let running = false;
  let pending = false;

  const scheduleReindex = (): void => {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    debounceTimer = setTimeout(() => {
      void reindex();
    }, 800);
  };

  const reindex = async (): Promise<void> => {
    if (running) {
      pending = true;
      return;
    }

    running = true;
    try {
      const started = Date.now();
      const result = await genieService.indexWorkspace();
      const durationMs = result?.durationMs ?? Date.now() - started;
      log(`Re-indexed in ${(durationMs / 1000).toFixed(1)}s`);
    } catch (error) {
      log(`Re-index failed: ${errorMessage(error)}`);
    } finally {
      running = false;
      if (pending) {
        pending = false;
        void reindex();
      }
    }
  };

  watcher.on('add', scheduleReindex);
  watcher.on('change', scheduleReindex);
  watcher.on('unlink', scheduleReindex);

  return watcher;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
