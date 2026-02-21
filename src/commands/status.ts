import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import { createGenieService, ensureIndexExists } from '../utils/service';

interface StatusOptions {
  json?: boolean;
}

export async function statusCommand(options: StatusOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const status = genie.getWarmupStatus();

    if (options.json) {
      jsonOutput(status);
    } else {
      prettyOutput.status(status);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Failed to get status: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
