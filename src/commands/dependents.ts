import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import { normalizeFilePathInput } from '../utils/paths';
import { createGenieService, ensureIndexExists } from '../utils/service';

interface DependentsOptions {
  json?: boolean;
}

export async function dependentsCommand(file: string, options: DependentsOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const filePath = normalizeFilePathInput(file, projectPath);
    const dependents = genie.getDependents(filePath);

    if (options.json) {
      jsonOutput(dependents);
    } else {
      prettyOutput.dependents(filePath, dependents);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Failed to get dependents: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
