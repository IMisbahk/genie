import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import { normalizeFilePathInput } from '../utils/paths';
import { createGenieService, ensureIndexExists } from '../utils/service';

interface DepsOptions {
  json?: boolean;
}

export async function depsCommand(file: string, options: DepsOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const filePath = normalizeFilePathInput(file, projectPath);
    const deps = genie.getDependencies(filePath);

    if (options.json) {
      jsonOutput(deps);
    } else {
      prettyOutput.dependencies(filePath, deps);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Failed to get dependencies: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
