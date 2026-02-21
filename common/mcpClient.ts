import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { FileInfo, ProjectSummary, SymbolInfo } from './types';

export interface GenieMCPClientOptions {
  command: string;
  args?: string[];
  cwd?: string;
  env?: Record<string, string>;
  name?: string;
  version?: string;
}

export interface ContextData {
  query: string;
  summary: ProjectSummary & { languages: string[] };
  symbols: SymbolInfo[];
  files: Array<{
    info: FileInfo;
    imports: string[];
    dependents: string[];
  }>;
}

type MCPStructuredResponse = {
  structuredContent?: Record<string, unknown>;
  content?: Array<{ type: string; text?: string }>;
  isError?: boolean;
};

export class GenieMCPClient {
  private readonly client: Client;
  private readonly transport: StdioClientTransport;
  private connected = false;

  constructor(options: GenieMCPClientOptions) {
    this.client = new Client(
      {
        name: options.name ?? 'genie-mcp-client',
        version: options.version ?? '1.0.0',
      },
      {
        capabilities: {},
      }
    );

    this.transport = new StdioClientTransport({
      command: options.command,
      args: options.args,
      cwd: options.cwd,
      env: options.env,
      stderr: 'pipe',
    });
  }

  async connect(): Promise<void> {
    if (this.connected) {
      return;
    }
    await this.client.connect(this.transport);
    this.connected = true;
  }

  async close(): Promise<void> {
    if (!this.connected) {
      return;
    }
    this.connected = false;
    await this.client.close();
  }

  async queryContext(prompt: string): Promise<ContextData> {
    await this.connect();

    const searchToken = this.extractSearchToken(prompt);
    const [summaryResponse, symbolResponse] = await Promise.all([
      this.callTool('genie_get_summary', {}),
      this.callTool('genie_search_symbols', { query: searchToken, limit: 5 }),
    ]);

    const summary = this.toSummary(summaryResponse);
    const symbols = this.toSymbols(symbolResponse);

    const uniqueFiles = Array.from(new Set(symbols.map((symbol) => symbol.filePath))).slice(0, 3);
    const files = [] as ContextData['files'];

    for (const filePath of uniqueFiles) {
      const [fileInfoResponse, dependentsResponse] = await Promise.all([
        this.callTool('genie_get_file_info', { filePath }),
        this.callTool('genie_get_dependents', { filePath }),
      ]);

      const fileInfo = this.toFileInfo(fileInfoResponse);
      if (!fileInfo) {
        continue;
      }

      files.push({
        info: fileInfo,
        imports: this.toStringList(fileInfoResponse, 'imports'),
        dependents: this.toStringList(dependentsResponse, 'dependents'),
      });
    }

    return {
      query: prompt,
      summary,
      symbols,
      files,
    };
  }

  private async callTool(name: string, args: Record<string, unknown>): Promise<Record<string, unknown>> {
    const response = (await this.client.callTool({
      name,
      arguments: args,
    })) as MCPStructuredResponse;

    if (response.isError) {
      const text = response.content?.find((item) => item.type === 'text')?.text;
      throw new Error(text ?? `MCP tool failed: ${name}`);
    }

    if (response.structuredContent && typeof response.structuredContent === 'object') {
      return response.structuredContent;
    }

    const text = response.content?.find((item) => item.type === 'text')?.text;
    if (text) {
      try {
        const parsed = JSON.parse(text) as Record<string, unknown>;
        if (parsed && typeof parsed === 'object') {
          return parsed;
        }
      } catch {
        // fallthrough
      }
    }

    return {};
  }

  private toSummary(payload: Record<string, unknown>): ProjectSummary & { languages: string[] } {
    return {
      totalFiles: this.toNumber(payload.totalFiles),
      totalSymbols: this.toNumber(payload.totalSymbols),
      totalDependencies: this.toNumber(payload.totalDependencies),
      languages: Array.isArray(payload.languages)
        ? payload.languages.filter((value): value is string => typeof value === 'string')
        : [],
    };
  }

  private toSymbols(payload: Record<string, unknown>): SymbolInfo[] {
    const rows = Array.isArray(payload.results) ? payload.results : [];
    return rows
      .map((row): SymbolInfo | null => {
        if (!row || typeof row !== 'object') {
          return null;
        }
        const objectRow = row as Record<string, unknown>;
        if (typeof objectRow.file !== 'string' || typeof objectRow.symbol !== 'string') {
          return null;
        }
        return {
          filePath: objectRow.file,
          name: objectRow.symbol,
          kind: typeof objectRow.type === 'string' ? objectRow.type : 'unknown',
          line: this.toNumber(objectRow.line),
          column: this.toNumber(objectRow.column),
        };
      })
      .filter((row): row is SymbolInfo => row !== null);
  }

  private toFileInfo(payload: Record<string, unknown>): FileInfo | null {
    if (typeof payload.path !== 'string' || typeof payload.language !== 'string') {
      return null;
    }

    return {
      path: payload.path,
      language: payload.language,
      size: 0,
      mtimeMs: 0,
      hash: '',
    };
  }

  private toStringList(payload: Record<string, unknown>, key: string): string[] {
    const value = payload[key];
    if (!Array.isArray(value)) {
      return [];
    }
    return value.filter((item): item is string => typeof item === 'string');
  }

  private toNumber(value: unknown): number {
    return typeof value === 'number' && Number.isFinite(value) ? value : 0;
  }

  private extractSearchToken(prompt: string): string {
    const idMatch = prompt.match(/[A-Za-z_][A-Za-z0-9_]*/g);
    if (!idMatch || idMatch.length === 0) {
      return prompt.trim() || 'project';
    }

    return idMatch[idMatch.length - 1];
  }
}
