/* eslint-disable @typescript-eslint/no-var-requires */
import fs from 'node:fs';
import path from 'node:path';

export interface NativeIndexResult {
  indexedFiles?: number;
  indexed_files?: number;
  changedFiles?: number;
  changed_files?: number;
  deletedFiles?: number;
  deleted_files?: number;
  symbolCount?: number;
  symbol_count?: number;
  dependencyCount?: number;
  dependency_count?: number;
  durationMs?: number;
  duration_ms?: number;
}

export interface NativeWarmupStatus {
  running: boolean;
  totalFiles?: number;
  total_files?: number;
  completedFiles?: number;
  completed_files?: number;
  remainingFiles?: number;
  remaining_files?: number;
}

export interface NativeIndexer {
  indexFull?(): NativeIndexResult;
  index_full?(): NativeIndexResult;
  queryFile?(path: string): any;
  query_file?(path: string): any;
  querySymbols?(query: string, limit?: number): any[];
  query_symbols?(query: string, limit?: number): any[];
  queryDependencies?(path: string): string[];
  query_dependencies?(path: string): string[];
  queryDependents?(path: string): string[];
  query_dependents?(path: string): string[];
  summary(): any;
  symbolWarmupStatus?(): NativeWarmupStatus;
  symbol_warmup_status?(): NativeWarmupStatus;
}

export interface NativeBindings {
  GenieIndexer: new (workspacePath: string, dbPath?: string) => NativeIndexer;
}

export function loadGenieNativeBindings(): NativeBindings {
  const localNode = path.resolve(__dirname, 'genie-core.node');
  if (fs.existsSync(localNode)) {
    return require(localNode) as NativeBindings;
  }

  const stagedNode = path.resolve(__dirname, '..', '..', '..', 'genie', 'node', 'genie-core.node');
  if (fs.existsSync(stagedNode)) {
    return require(stagedNode) as NativeBindings;
  }

  const rootLoader = path.resolve(__dirname, '..', '..', 'index.js');
  if (fs.existsSync(rootLoader)) {
    return require(rootLoader) as NativeBindings;
  }

  throw new Error('GENIE native binary not found. Expected genie/node/genie-core.node');
}
