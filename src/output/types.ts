export interface SymbolRow {
  filePath: string;
  name: string;
  kind: string;
  line: number;
  column: number;
}

export interface FileInfoOutput {
  path: string;
  language?: string;
  size?: number;
  lastIndexed?: number;
  symbols?: Array<{
    name: string;
    kind: string;
    line: number;
    column?: number;
  }>;
  imports?: string[];
  exports?: string[];
  dependents?: string[];
}

export interface ProjectSummaryOutput {
  totalFiles: number;
  totalSymbols: number;
  totalDependencies: number;
  languages?: Record<string, number>;
  lastIndexed?: number;
  indexSize?: number;
}
