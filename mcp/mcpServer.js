"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.GenieMCPServer = void 0;
exports.normalizeFileInfoForMCP = normalizeFileInfoForMCP;
const node_http_1 = __importDefault(require("node:http"));
const node_url_1 = require("node:url");
const mcp_js_1 = require("@modelcontextprotocol/sdk/server/mcp.js");
const stdio_js_1 = require("@modelcontextprotocol/sdk/server/stdio.js");
const sse_js_1 = require("@modelcontextprotocol/sdk/server/sse.js");
const zod_1 = require("zod");
class GenieMCPServer {
    service;
    server;
    languages = new Set();
    httpServer = null;
    sseTransport = null;
    constructor(service, options) {
        this.service = service;
        this.server = new mcp_js_1.McpServer({
            name: options?.name ?? 'genie-mcp',
            version: options?.version ?? '1.0.0',
        });
        this.registerTools();
    }
    async handleToolCall(request) {
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
        }
        catch (error) {
            return { error: this.errorMessage(error) };
        }
    }
    async startStdio() {
        await this.server.connect(new stdio_js_1.StdioServerTransport());
    }
    async startSSE(transport) {
        this.sseTransport = transport;
        await this.server.connect(transport);
        await transport.start();
    }
    async startSSEHttpServer(options) {
        const host = options?.host ?? '127.0.0.1';
        const port = options?.port ?? 0;
        const ssePath = options?.ssePath ?? '/mcp/sse';
        const postPath = options?.postPath ?? '/mcp/messages';
        const server = node_http_1.default.createServer(async (req, res) => {
            if (!req.url || !req.method) {
                res.writeHead(400).end();
                return;
            }
            const url = new node_url_1.URL(req.url, `http://${host}`);
            if (req.method === 'GET' && url.pathname === ssePath) {
                if (this.sseTransport) {
                    res.writeHead(409, { 'content-type': 'application/json' });
                    res.end(JSON.stringify({ error: 'SSE session already active' }));
                    return;
                }
                try {
                    const transport = new sse_js_1.SSEServerTransport(postPath, res);
                    this.sseTransport = transport;
                    await this.startSSE(transport);
                }
                catch (error) {
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
                    await this.sseTransport.handlePostMessage(req, res);
                }
                catch (error) {
                    res.writeHead(500, { 'content-type': 'application/json' });
                    res.end(JSON.stringify({ error: this.errorMessage(error) }));
                }
                return;
            }
            res.writeHead(404, { 'content-type': 'application/json' });
            res.end(JSON.stringify({ error: 'Not found' }));
        });
        await new Promise((resolve, reject) => {
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
    async close() {
        const httpServer = this.httpServer;
        this.httpServer = null;
        if (httpServer) {
            await new Promise((resolve) => {
                httpServer.close(() => resolve());
            });
        }
        this.sseTransport = null;
        await this.server.close();
    }
    registerTools() {
        this.server.registerTool('genie_get_file_info', {
            description: 'Get information about a specific file including symbols and imports',
            inputSchema: {
                filePath: zod_1.z.string().min(1),
            },
            outputSchema: {
                path: zod_1.z.string(),
                language: zod_1.z.string(),
                symbols: zod_1.z.array(zod_1.z.object({
                    name: zod_1.z.string(),
                    kind: zod_1.z.string(),
                    line: zod_1.z.number(),
                    column: zod_1.z.number(),
                })),
                imports: zod_1.z.array(zod_1.z.string()),
                exports: zod_1.z.array(zod_1.z.string()),
            },
        }, async (args) => {
            const result = await this.getFileInfo(args);
            return this.success(result);
        });
        this.server.registerTool('genie_search_symbols', {
            description: 'Search for symbols (functions, classes, components) by name',
            inputSchema: {
                query: zod_1.z.string().min(1),
                limit: zod_1.z.number().int().positive().max(500).optional(),
            },
            outputSchema: {
                results: zod_1.z.array(zod_1.z.object({
                    file: zod_1.z.string(),
                    symbol: zod_1.z.string(),
                    type: zod_1.z.string(),
                    line: zod_1.z.number(),
                    column: zod_1.z.number(),
                })),
            },
        }, async (args) => {
            const result = await this.searchSymbols(args);
            return this.success(result);
        });
        this.server.registerTool('genie_get_dependencies', {
            description: 'Get what files this file imports',
            inputSchema: {
                filePath: zod_1.z.string().min(1),
            },
            outputSchema: {
                dependencies: zod_1.z.array(zod_1.z.string()),
            },
        }, async (args) => {
            const result = await this.getDependencies(args);
            return this.success(result);
        });
        this.server.registerTool('genie_get_dependents', {
            description: 'Get what files import this file (impact analysis)',
            inputSchema: {
                filePath: zod_1.z.string().min(1),
            },
            outputSchema: {
                dependents: zod_1.z.array(zod_1.z.string()),
            },
        }, async (args) => {
            const result = await this.getDependents(args);
            return this.success(result);
        });
        this.server.registerTool('genie_get_summary', {
            description: 'Get project-wide statistics',
            outputSchema: {
                totalFiles: zod_1.z.number(),
                totalSymbols: zod_1.z.number(),
                totalDependencies: zod_1.z.number(),
                languages: zod_1.z.array(zod_1.z.string()),
            },
        }, async () => {
            const result = await this.getProjectSummary();
            return this.success(result);
        });
    }
    async getFileInfo(args) {
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
    async searchSymbols(args) {
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
    async getDependencies(args) {
        const filePath = this.requireString(args, 'filePath');
        return {
            dependencies: this.service.getDependencies(filePath),
        };
    }
    async getDependents(args) {
        const filePath = this.requireString(args, 'filePath');
        return {
            dependents: this.service.getDependents(filePath),
        };
    }
    async getProjectSummary() {
        const summary = this.service.getProjectSummary();
        return {
            totalFiles: summary.totalFiles,
            totalSymbols: summary.totalSymbols,
            totalDependencies: summary.totalDependencies,
            languages: Array.from(this.languages).sort(),
        };
    }
    symbolsForFile(filePath) {
        const query = this.baseNameForSearch(filePath);
        return this.service.searchSymbols(query).filter((symbol) => symbol.filePath === filePath);
    }
    baseNameForSearch(filePath) {
        const normalized = filePath.replace(/\\/g, '/');
        const fileName = normalized.split('/').pop() ?? normalized;
        return fileName.replace(/\.[^.]+$/, '');
    }
    requireString(obj, key) {
        const value = obj?.[key];
        if (typeof value !== 'string' || value.trim().length === 0) {
            throw new Error(`Invalid argument: ${key} must be a non-empty string`);
        }
        return value;
    }
    optionalNumber(obj, key) {
        const value = obj?.[key];
        if (typeof value === 'number' && Number.isFinite(value)) {
            return value;
        }
        return undefined;
    }
    success(data) {
        return {
            content: [{ type: 'text', text: JSON.stringify(data) }],
            structuredContent: data,
        };
    }
    errorMessage(error) {
        if (error instanceof Error) {
            return error.message;
        }
        return String(error);
    }
}
exports.GenieMCPServer = GenieMCPServer;
function normalizeFileInfoForMCP(fileInfo, symbols, imports) {
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
