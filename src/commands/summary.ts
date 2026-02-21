import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import {
  createGenieService,
  ensureIndexExists,
  readIndexMetadata,
  readLanguagesFromWorkspaceOverview,
} from '../utils/service';

interface SummaryOptions {
  json?: boolean;
}

export async function summaryCommand(options: SummaryOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const summary = genie.getProjectSummary();
    const output = {
      ...summary,
      ...readIndexMetadata(projectPath),
      languages: readLanguagesFromWorkspaceOverview(projectPath),
    };

    if (options.json) {
      jsonOutput(output);
    } else {
      prettyOutput.summary(output);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Failed to get summary: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
