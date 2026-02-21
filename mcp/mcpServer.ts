import http, { IncomingMessage, ServerResponse } from 'node:http';
import { URL } from 'node:url';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { SSEServerTransport } from '@modelcontextprotocol/sdk/server/sse.js';
import { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import { AuthInfo } from '@modelcontextprotocol/sdk/server/auth/types.js';
import { z } from 'zod';
import { IGenieService } from '../common/genieService';
import { FileInfo, SymbolInfo } from '../common/types';

export interface MCPToolRequest {
  name: string;
  arguments?: Record<string, unknown>;
}

export interface MCPToolResponse {
  data?: Record<string, unknown>;
  error?: string;
}

interface GenieMCPServerOptions {
  name?: string;
  version?: string;
}

interface SSEHttpOptions {
  host?: string;
  port?: number;
  ssePath?: string;
  postPath?: string;
}

interface SSEHttpHandle {
  port: number;
  host: string;
  ssePath: string;
  postPath: string;
  close: () => Promise<void>;
}

interface FileInfoToolOutput extends Record<string, unknown> {
  path: string;
  language: string;
  symbols: Array<{
    name: string;
    kind: string;
    line: number;
    column: number;
  }>;
  imports: string[];
  exports: string[];
}

interface SymbolSearchOutput extends Record<string, unknown> {
  results: Array<{
    file: string;
    symbol: string;
    type: string;
    line: number;
    column: number;
  }>;
}

interface DependenciesOutput extends Record<string, unknown> {
  dependencies: string[];
}

interface DependentsOutput extends Record<string, unknown> {
  dependents: string[];
}

interface SummaryOutput extends Record<string, unknown> {
  totalFiles: number;
  totalSymbols: number;
  totalDependencies: number;
  languages: string[];
}

export class GenieMCPServer {
  private readonly service: IGenieService;
  private readonly server: McpServer;
  private readonly languages = new Set<string>();
  private httpServer: http.Server | null = null;
  private sseTransport: SSEServerTransport | null = null;

  constructor(service: IGenieService, options?: GenieMCPServerOptions) {
    this.service = service;
    this.server = new McpServer({
      name: options?.name ?? 'genie-mcp',
      version: options?.version ?? '1.0.0',
    });

    this.registerTools();
  }

  async handleToolCall(request: MCPToolRequest): Promise<MCPToolResponse> {
    try {
      switch (request.name) {
        case 'genie_get_file_info':
          return { data: await this.getFileInfo(request.arguments) };
        case 'genie_search_symbols':
          return { data: await this.searchSymbols(request.arguments) };
        case 'genie_get_dependencies':
          return { data: await this.getDependencies(request.arguments) };
        case 'genie_get_dependents':
          return { data: await this.getDependents(request.arguments) };
        case 'genie_get_summary':
          return { data: await this.getProjectSummary() };
        default:
          return { error: `Unknown tool: ${request.name}` };
      }
    } catch (error) {
      return { error: this.errorMessage(error) };
    }
  }

  async startStdio(): Promise<void> {
    await this.server.connect(new StdioServerTransport());
  }

  async startSSE(transport: SSEServerTransport): Promise<void> {
    this.sseTransport = transport;
    await this.server.connect(transport);
    await transport.start();
  }

  async startSSEHttpServer(options?: SSEHttpOptions): Promise<SSEHttpHandle> {
    const host = options?.host ?? '127.0.0.1';
    const port = options?.port ?? 0;
    const ssePath = options?.ssePath ?? '/mcp/sse';
    const postPath = options?.postPath ?? '/mcp/messages';

    const server = http.createServer(async (req, res) => {
      if (!req.url || !req.method) {
        res.writeHead(400).end();
        return;
      }

      const url = new URL(req.url, `http://${host}`);
      if (req.method === 'GET' && url.pathname === ssePath) {
        if (this.sseTransport) {
          res.writeHead(409, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: 'SSE session already active' }));
          return;
        }
        try {
          const transport = new SSEServerTransport(postPath, res);
          this.sseTransport = transport;
          await this.startSSE(transport);
        } catch (error) {
          this.sseTransport = null;
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: this.errorMessage(error) }));
        }
        return;
      }

      if (req.method === 'POST' && url.pathname === postPath) {
        if (!this.sseTransport) {
          res.writeHead(404, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: 'No active SSE session' }));
          return;
        }

        try {
          await this.sseTransport.handlePostMessage(req as IncomingMessage & { auth?: AuthInfo }, res as ServerResponse);
        } catch (error) {
          res.writeHead(500, { 'content-type': 'application/json' });
          res.end(JSON.stringify({ error: this.errorMessage(error) }));
        }
        return;
      }

      res.writeHead(404, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: 'Not found' }));
    });

    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(port, host, () => resolve());
    });

    this.httpServer = server;
    const address = server.address();
    const actualPort = typeof address === 'object' && address ? address.port : port;

    return {
      port: actualPort,
      host,
      ssePath,
      postPath,
      close: async () => {
        await this.close();
      },
    };
  }

  async close(): Promise<void> {
    const httpServer = this.httpServer;
    this.httpServer = null;

    if (httpServer) {
      await new Promise<void>((resolve) => {
        httpServer.close(() => resolve());
      });
    }

    this.sseTransport = null;
    await this.server.close();
  }

  private registerTools(): void {
    this.server.registerTool(
      'genie_get_file_info',
      {
        description: 'Get information about a specific file including symbols and imports',
        inputSchema: {
          filePath: z.string().min(1),
        },
        outputSchema: {
          path: z.string(),
          language: z.string(),
          symbols: z.array(
            z.object({
              name: z.string(),
              kind: z.string(),
              line: z.number(),
              column: z.number(),
            })
          ),
          imports: z.array(z.string()),
          exports: z.array(z.string()),
        },
      },
      async (args) => {
        const result = await this.getFileInfo(args);
        return this.success(result);
      }
    );

    this.server.registerTool(
      'genie_search_symbols',
      {
        description: 'Search for symbols (functions, classes, components) by name',
        inputSchema: {
          query: z.string().min(1),
          limit: z.number().int().positive().max(500).optional(),
        },
        outputSchema: {
          results: z.array(
            z.object({
              file: z.string(),
              symbol: z.string(),
              type: z.string(),
              line: z.number(),
              column: z.number(),
            })
          ),
        },
      },
      async (args) => {
        const result = await this.searchSymbols(args);
        return this.success(result);
      }
    );

    this.server.registerTool(
      'genie_get_dependencies',
      {
        description: 'Get what files this file imports',
        inputSchema: {
          filePath: z.string().min(1),
        },
        outputSchema: {
          dependencies: z.array(z.string()),
        },
      },
      async (args) => {
        const result = await this.getDependencies(args);
        return this.success(result);
      }
    );

    this.server.registerTool(
      'genie_get_dependents',
      {
        description: 'Get what files import this file (impact analysis)',
        inputSchema: {
          filePath: z.string().min(1),
        },
        outputSchema: {
          dependents: z.array(z.string()),
        },
      },
      async (args) => {
        const result = await this.getDependents(args);
        return this.success(result);
      }
    );

    this.server.registerTool(
      'genie_get_summary',
      {
        description: 'Get project-wide statistics',
        outputSchema: {
          totalFiles: z.number(),
          totalSymbols: z.number(),
          totalDependencies: z.number(),
          languages: z.array(z.string()),
        },
      },
      async () => {
        const result = await this.getProjectSummary();
        return this.success(result);
      }
    );
  }

  private async getFileInfo(args: Record<string, unknown> | undefined): Promise<FileInfoToolOutput> {
    const filePath = this.requireString(args, 'filePath');
    const fileInfo = this.service.getFileInfo(filePath);
    if (!fileInfo) {
      throw new Error(`File not found in index: ${filePath}`);
    }

    const symbols = this.symbolsForFile(fileInfo.path);
    const imports = this.service.getDependencies(filePath);

    this.languages.add(fileInfo.language);

    return {
      path: fileInfo.path,
      language: fileInfo.language,
      symbols: symbols.map((symbol) => ({
        name: symbol.name,
        kind: symbol.kind,
        line: symbol.line,
        column: symbol.column,
      })),
      imports,
      // Export-level metadata is not exposed by native core yet.
      exports: [],
    };
  }

  private async searchSymbols(args: Record<string, unknown> | undefined): Promise<SymbolSearchOutput> {
    const query = this.requireString(args, 'query');
    const limit = this.optionalNumber(args, 'limit') ?? 100;

    const rows = this.service.searchSymbols(query).slice(0, limit);
    for (const row of rows) {
      const info = this.service.getFileInfo(row.filePath);
      if (info?.language) {
        this.languages.add(info.language);
      }
    }

    return {
      results: rows.map((row) => ({
        file: row.filePath,
        symbol: row.name,
        type: row.kind,
        line: row.line,
        column: row.column,
      })),
    };
  }

  private async getDependencies(args: Record<string, unknown> | undefined): Promise<DependenciesOutput> {
    const filePath = this.requireString(args, 'filePath');
    return {
      dependencies: this.service.getDependencies(filePath),
    };
  }

  private async getDependents(args: Record<string, unknown> | undefined): Promise<DependentsOutput> {
    const filePath = this.requireString(args, 'filePath');
    return {
      dependents: this.service.getDependents(filePath),
    };
  }

  private async getProjectSummary(): Promise<SummaryOutput> {
    const summary = this.service.getProjectSummary();
    return {
      totalFiles: summary.totalFiles,
      totalSymbols: summary.totalSymbols,
      totalDependencies: summary.totalDependencies,
      languages: Array.from(this.languages).sort(),
    };
  }

  private symbolsForFile(filePath: string): SymbolInfo[] {
    const query = this.baseNameForSearch(filePath);
    return this.service.searchSymbols(query).filter((symbol) => symbol.filePath === filePath);
  }

  private baseNameForSearch(filePath: string): string {
    const normalized = filePath.replace(/\\/g, '/');
    const fileName = normalized.split('/').pop() ?? normalized;
    return fileName.replace(/\.[^.]+$/, '');
  }

  private requireString(obj: Record<string, unknown> | undefined, key: string): string {
    const value = obj?.[key];
    if (typeof value !== 'string' || value.trim().length === 0) {
      throw new Error(`Invalid argument: ${key} must be a non-empty string`);
    }
    return value;
  }

  private optionalNumber(obj: Record<string, unknown> | undefined, key: string): number | undefined {
    const value = obj?.[key];
    if (typeof value === 'number' && Number.isFinite(value)) {
      return value;
    }
    return undefined;
  }

  private success(data: Record<string, unknown>): CallToolResult {
    return {
      content: [{ type: 'text', text: JSON.stringify(data) }],
      structuredContent: data,
    };
  }

  private errorMessage(error: unknown): string {
    if (error instanceof Error) {
      return error.message;
    }
    return String(error);
  }
}

export function normalizeFileInfoForMCP(fileInfo: FileInfo, symbols: SymbolInfo[], imports: string[]): FileInfoToolOutput {
  return {
    path: fileInfo.path,
    language: fileInfo.language,
    symbols: symbols.map((symbol) => ({
      name: symbol.name,
      kind: symbol.kind,
      line: symbol.line,
      column: symbol.column,
    })),
    imports,
    exports: [],
  };
}
