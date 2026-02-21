import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import { createGenieService, ensureIndexExists } from '../utils/service';

interface SearchOptions {
  json?: boolean;
}

export async function searchCommand(query: string, options: SearchOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const results = genie.searchSymbols(query);

    if (options.json) {
      jsonOutput(results);
    } else {
      prettyOutput.searchResults(query, results);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Search failed: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
