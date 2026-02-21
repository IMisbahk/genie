import fs from 'node:fs';
import path from 'node:path';
import { GenieNodeService } from '../../node/genieNodeService';
import { indexDbPath, normalizeProjectPath } from './paths';

export interface ServiceOptions {
  autoIndexOnInit?: boolean;
  enableWatcher?: boolean;
}

export async function createGenieService(
  projectPath: string,
  options: ServiceOptions = {}
): Promise<GenieNodeService> {
  const resolvedProject = normalizeProjectPath(projectPath);
  const service = new GenieNodeService();

  await service.initialize(resolvedProject, {
    dbPath: indexDbPath(resolvedProject),
    autoIndexOnInit: options.autoIndexOnInit ?? false,
    enableWatcher: options.enableWatcher ?? false,
  });

  if (!service.isAvailable()) {
    throw new Error(service.getLastError() || 'GENIE service is unavailable');
  }

  return service;
}

export function ensureIndexExists(projectPath: string): void {
  if (!fs.existsSync(indexDbPath(projectPath))) {
    throw new Error('No index found. Run `genie index` first.');
  }
}

export function readIndexMetadata(projectPath: string): {
  lastIndexed?: number;
  indexSize?: number;
} {
  try {
    const stat = fs.statSync(indexDbPath(projectPath));
    return {
      lastIndexed: stat.mtimeMs,
      indexSize: stat.size,
    };
  } catch {
    return {};
  }
}

export function readLanguagesFromWorkspaceOverview(projectPath: string): Record<string, number> | undefined {
  const readmePath = path.join(normalizeProjectPath(projectPath), '.genie', 'README.md');

  try {
    const content = fs.readFileSync(readmePath, 'utf8');
    const line = content
      .split(/\r?\n/)
      .find((entry) => entry.toLowerCase().startsWith('primary languages:'));

    if (!line) {
      return undefined;
    }

    const tail = line.split(':').slice(1).join(':').trim().replace(/\.$/, '');
    if (!tail || tail === 'unknown') {
      return undefined;
    }

    const output: Record<string, number> = {};
    for (const segment of tail.split(',')) {
      const part = segment.trim();
      const match = part.match(/^(.*?)\s*\((\d+)\)$/);
      if (!match) {
        continue;
      }
      const language = match[1].trim();
      const count = Number.parseInt(match[2], 10);
      if (language && Number.isFinite(count)) {
        output[language] = count;
      }
    }

    return Object.keys(output).length > 0 ? output : undefined;
  } catch {
    return undefined;
  }
}
