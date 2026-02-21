"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.loadGenieNativeBindings = loadGenieNativeBindings;
/* eslint-disable @typescript-eslint/no-var-requires */
const node_fs_1 = __importDefault(require("node:fs"));
const node_path_1 = __importDefault(require("node:path"));
function loadGenieNativeBindings() {
    const localNode = node_path_1.default.resolve(__dirname, 'genie-core.node');
    if (node_fs_1.default.existsSync(localNode)) {
        return require(localNode);
    }
    const stagedNode = node_path_1.default.resolve(__dirname, '..', '..', '..', 'genie', 'node', 'genie-core.node');
    if (node_fs_1.default.existsSync(stagedNode)) {
        return require(stagedNode);
    }
    const rootLoader = node_path_1.default.resolve(__dirname, '..', '..', 'index.js');
    if (node_fs_1.default.existsSync(rootLoader)) {
        return require(rootLoader);
    }
    throw new Error('GENIE native binary not found. Expected genie/node/genie-core.node');
}
