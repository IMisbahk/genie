import { FileInfo, GenieConfig, IndexResult, ProjectSummary, SymbolInfo, WarmupStatus } from './types';

export interface IGenieService {
  initialize(workspacePath: string, config?: GenieConfig): Promise<void>;

  getFileInfo(path: string): FileInfo | null;
  searchSymbols(query: string, limit?: number): SymbolInfo[];
  getDependencies(filePath: string): string[];
  getDependents(filePath: string): string[];
  getProjectSummary(): ProjectSummary;

  indexWorkspace(path?: string): Promise<IndexResult | null>;
  isWarmupComplete(): boolean;
  getWarmupStatus(): WarmupStatus;
  dispose?(): void;
}
