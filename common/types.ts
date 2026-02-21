export interface FileInfo {
  path: string;
  language: string;
  size: number;
  mtimeMs: number;
  hash: string;
}

export interface SymbolInfo {
  filePath: string;
  name: string;
  kind: string;
  line: number;
  column: number;
}

export interface ProjectSummary {
  totalFiles: number;
  totalSymbols: number;
  totalDependencies: number;
}

export interface IndexResult {
  indexedFiles: number;
  changedFiles: number;
  deletedFiles: number;
  symbolCount: number;
  dependencyCount: number;
  durationMs: number;
}

export interface WarmupStatus {
  running: boolean;
  totalFiles: number;
  completedFiles: number;
  remainingFiles: number;
}

export interface GenieConfig {
  dbPath?: string;
  ignorePatterns?: string[];
  maxFileSize?: number;
  indexInterval?: number;
  enableWarmup?: boolean;
  maxConcurrency?: number;
  autoIndexOnInit?: boolean;
  enableWatcher?: boolean;
}

export interface IndexingUnavailable {
  available: false;
  reason: string;
}
