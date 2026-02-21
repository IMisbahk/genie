import path from 'node:path';

export function normalizeProjectPath(projectPath: string): string {
  return path.resolve(projectPath);
}

export function normalizeFilePathInput(filePath: string, projectPath: string): string {
  const resolvedProject = normalizeProjectPath(projectPath);
  const resolvedFile = path.isAbsolute(filePath)
    ? path.resolve(filePath)
    : path.resolve(resolvedProject, filePath);
  const relative = path.relative(resolvedProject, resolvedFile);
  return relative.replace(/\\/g, '/').replace(/^\.\//, '');
}

export function indexDirPath(projectPath: string): string {
  return path.join(normalizeProjectPath(projectPath), '.genie');
}

export function indexDbPath(projectPath: string): string {
  return path.join(indexDirPath(projectPath), 'index.db');
}
