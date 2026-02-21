import { prettyOutput } from '../output/pretty';
import { jsonOutput } from '../output/json';
import { logger } from '../utils/logger';
import { normalizeFilePathInput } from '../utils/paths';
import {
  createGenieService,
  ensureIndexExists,
  readIndexMetadata,
} from '../utils/service';

interface InfoOptions {
  json?: boolean;
}

export async function infoCommand(file: string, options: InfoOptions = {}): Promise<void> {
  const projectPath = process.cwd();

  try {
    ensureIndexExists(projectPath);
    const genie = await createGenieService(projectPath, {
      autoIndexOnInit: false,
      enableWatcher: false,
    });

    const filePath = normalizeFilePathInput(file, projectPath);
    const fileInfo = genie.getFileInfo(filePath);

    if (!fileInfo) {
      throw new Error(`File not found in index: ${filePath}`);
    }

    const symbols = genie
      .searchSymbols('', 100_000)
      .filter((symbol) => symbol.filePath === filePath);

    const output = {
      path: fileInfo.path,
      language: fileInfo.language,
      size: fileInfo.size,
      ...readIndexMetadata(projectPath),
      symbols,
      imports: genie.getDependencies(filePath),
      exports: [] as string[],
      dependents: genie.getDependents(filePath),
    };

    if (options.json) {
      jsonOutput(output);
    } else {
      prettyOutput.fileInfo(output);
    }

    genie.dispose();
  } catch (error) {
    logger.error(`Failed to get file info: ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
