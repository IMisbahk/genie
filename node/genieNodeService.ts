import { IGenieService } from '../common/genieService';
import {
  FileInfo,
  GenieConfig,
  IndexResult,
  ProjectSummary,
  SymbolInfo,
  WarmupStatus,
} from '../common/types';
import { loadGenieNativeBindings, NativeIndexer } from './nativeBindings';
import fs from 'node:fs';
import path from 'node:path';

export class GenieNodeService implements IGenieService {
  private workspacePath = '';
  private dbPath: string | undefined;
  private native: NativeIndexer | null = null;
  private available = false;
  private lastError = '';
  private currentIndexTask: Promise<IndexResult | null> | null = null;
  private watcher: fs.FSWatcher | null = null;
  private reindexTimer: NodeJS.Timeout | null = null;
  private pollTimer: NodeJS.Timeout | null = null;
  private readonly reindexDebounceMs = 250;
  private pollIntervalMs = 5000;
  private readonly ignoredDirNames = new Set([
    '.git',
    '.next',
    'build',
    'dist',
    'node_modules',
    'target',
    '.rocket',
    '.genie',
  ]);

  async initialize(workspacePath: string, config?: GenieConfig): Promise<void> {
    this.disposeWatcher();
    this.workspacePath = workspacePath;
    this.dbPath = config?.dbPath;
    this.pollIntervalMs = Math.max(1000, config?.indexInterval ?? 5000);
    const enableWatcher = config?.enableWatcher ?? true;
    const autoIndexOnInit = config?.autoIndexOnInit ?? true;

    try {
      const bindings = loadGenieNativeBindings();
      this.native = new bindings.GenieIndexer(this.workspacePath, this.dbPath);
      this.available = true;
      this.lastError = '';
      if (enableWatcher) {
        this.setupFileWatcher();
      }
      if (autoIndexOnInit) {
        await this.indexWorkspace();
      }
    } catch (error) {
      this.available = false;
      this.lastError = this.formatError(error);
      this.native = null;
      this.debug(`initialize failed: ${this.lastError}`);
    }
  }

  async indexWorkspace(pathOverride?: string): Promise<IndexResult | null> {
    if (!this.available || !this.native) {
      return null;
    }

    if (pathOverride && pathOverride !== this.workspacePath) {
      await this.initialize(pathOverride, {
        dbPath: this.dbPath,
        autoIndexOnInit: false,
      });
      if (!this.available || !this.native) {
        return null;
      }
    }

    if (this.currentIndexTask) {
      return this.currentIndexTask;
    }

    this.currentIndexTask = Promise.resolve().then(() => {
      try {
        const row: any = this.call(this.native, 'indexFull', 'index_full', []);
        return {
          indexedFiles: row?.indexedFiles ?? row?.indexed_files ?? 0,
          changedFiles: row?.changedFiles ?? row?.changed_files ?? 0,
          deletedFiles: row?.deletedFiles ?? row?.deleted_files ?? 0,
          symbolCount: row?.symbolCount ?? row?.symbol_count ?? 0,
          dependencyCount: row?.dependencyCount ?? row?.dependency_count ?? 0,
          durationMs: row?.durationMs ?? row?.duration_ms ?? 0,
        } as IndexResult;
      } catch (error) {
        this.lastError = this.formatError(error);
        this.debug(`indexWorkspace failed: ${this.lastError}`);
        return null;
      }
    }).finally(() => {
      this.currentIndexTask = null;
    });
    return this.currentIndexTask;
  }

  getFileInfo(path: string): FileInfo | null {
    if (!this.available || !this.native) {
      return null;
    }

    try {
      const row: any = this.call(this.native, 'queryFile', 'query_file', [path]);
      if (!row) {
        return null;
      }
      return {
        path: row.path,
        language: row.language,
        size: row.size,
        mtimeMs: row.mtimeMs ?? row.mtime_ms ?? 0,
        hash: row.hash,
      };
    } catch {
      return null;
    }
  }

  searchSymbols(query: string, limit = 1000): SymbolInfo[] {
    if (!this.available || !this.native) {
      return [];
    }

    try {
      const rows: any[] = this.call(this.native, 'querySymbols', 'query_symbols', [query, limit]) ?? [];
      return rows.map((row: any) => ({
        filePath: row.filePath ?? row.file_path,
        name: row.name,
        kind: row.kind,
        line: row.line,
        column: row.column,
      }));
    } catch {
      return [];
    }
  }

  getDependencies(filePath: string): string[] {
    if (!this.available || !this.native) {
      return [];
    }

    try {
      return this.call(this.native, 'queryDependencies', 'query_dependencies', [filePath]) ?? [];
    } catch {
      return [];
    }
  }

  getDependents(filePath: string): string[] {
    if (!this.available || !this.native) {
      return [];
    }

    try {
      return this.call(this.native, 'queryDependents', 'query_dependents', [filePath]) ?? [];
    } catch {
      return [];
    }
  }

  getProjectSummary(): ProjectSummary {
    if (!this.available || !this.native) {
      return { totalFiles: 0, totalSymbols: 0, totalDependencies: 0 };
    }

    try {
      const row: any = this.native.summary();
      return {
        totalFiles: row.totalFiles ?? row.total_files ?? 0,
        totalSymbols: row.totalSymbols ?? row.total_symbols ?? 0,
        totalDependencies: row.totalDependencies ?? row.total_dependencies ?? 0,
      };
    } catch {
      return { totalFiles: 0, totalSymbols: 0, totalDependencies: 0 };
    }
  }

  isWarmupComplete(): boolean {
    const status = this.getWarmupStatus();
    return !status.running && status.remainingFiles === 0;
  }

  getWarmupStatus(): WarmupStatus {
    if (!this.available || !this.native) {
      return { running: false, totalFiles: 0, completedFiles: 0, remainingFiles: 0 };
    }

    try {
      const row: any = this.call(this.native, 'symbolWarmupStatus', 'symbol_warmup_status', []);
      if (!row) {
        return { running: false, totalFiles: 0, completedFiles: 0, remainingFiles: 0 };
      }

      return {
        running: Boolean(row.running),
        totalFiles: row.totalFiles ?? row.total_files ?? 0,
        completedFiles: row.completedFiles ?? row.completed_files ?? 0,
        remainingFiles: row.remainingFiles ?? row.remaining_files ?? 0,
      };
    } catch {
      return { running: false, totalFiles: 0, completedFiles: 0, remainingFiles: 0 };
    }
  }

  isAvailable(): boolean {
    return this.available;
  }

  getLastError(): string {
    return this.lastError;
  }

  dispose(): void {
    this.disposeWatcher();
  }

  private call(target: any, camel: string, snake: string, args: any[]): any {
    if (typeof target[camel] === 'function') {
      return target[camel](...args);
    }
    if (typeof target[snake] === 'function') {
      return target[snake](...args);
    }
    throw new Error(`Missing native method: ${camel}/${snake}`);
  }

  private formatError(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }

  private setupFileWatcher(): void {
    if (!this.workspacePath) {
      return;
    }

    try {
      this.watcher = fs.watch(
        this.workspacePath,
        { recursive: true },
        (_eventType, fileName) => {
          if (!fileName) {
            return;
          }
          const rel = fileName.toString().replace(/\\/g, '/');
          if (this.shouldIgnoreChange(rel)) {
            return;
          }
          this.scheduleReindex();
        }
      );
      this.watcher.on('error', (error) => {
        this.debug(`watcher error: ${this.formatError(error)}`);
        this.disposeWatcher();
        this.setupPollingFallback();
      });
    } catch (error) {
      this.debug(`watcher unavailable: ${this.formatError(error)}`);
      this.setupPollingFallback();
    }
  }

  private scheduleReindex(): void {
    if (this.reindexTimer) {
      clearTimeout(this.reindexTimer);
    }
    this.reindexTimer = setTimeout(() => {
      this.reindexTimer = null;
      void this.indexWorkspace();
    }, this.reindexDebounceMs);
  }

  private shouldIgnoreChange(relPath: string): boolean {
    const base = path.basename(relPath);
    if (
      base.endsWith('.test.ts') ||
      base.endsWith('.spec.ts') ||
      base.endsWith('.d.ts')
    ) {
      return true;
    }

    const parts = relPath.split('/');
    for (const part of parts) {
      if (this.ignoredDirNames.has(part)) {
        return true;
      }
    }
    return false;
  }

  private disposeWatcher(): void {
    if (this.reindexTimer) {
      clearTimeout(this.reindexTimer);
      this.reindexTimer = null;
    }
    if (this.watcher) {
      this.watcher.close();
      this.watcher = null;
    }
    if (this.pollTimer) {
      clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
  }

  private debug(message: string): void {
    // Headless production behavior: debug-only logging.
    console.debug(`[GENIE] ${message}`);
  }

  private setupPollingFallback(): void {
    if (this.pollTimer) {
      return;
    }
    this.pollTimer = setInterval(() => {
      void this.indexWorkspace();
    }, this.pollIntervalMs);
  }
}
