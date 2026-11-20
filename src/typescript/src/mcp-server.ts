#!/usr/bin/env node
/**
 * Stdio-only MCP server. Only the trusted launcher chooses --root/--binary.
 * This module is a separate entrypoint: importing the main adapter does not
 * load an MCP SDK or start a server.
 */
import { realpathSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { McpServer } from "@modelcontextprotocol/server";
import { serveStdio } from "@modelcontextprotocol/server/stdio";
import * as z from "zod/v4";
import { ReaTermError } from "./index.js";
import { executeReatermTool, READ_COMMANDS, TOOL_DESCRIPTION } from "./tools.js";

export interface ServerOptions {
  root: string;
  binary?: string;
}

export function parseServerArgs(argv: readonly string[]): ServerOptions {
  let root: string | undefined;
  let binary: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--root" && i + 1 < argv.length && root === undefined) {
      root = argv[++i];
    } else if (argv[i] === "--binary" && i + 1 < argv.length && binary === undefined) {
      binary = argv[++i];
    } else {
      throw new Error("Usage: reaterm-mcp --root DIR [--binary EXECUTABLE]");
    }
  }
  if (!root) throw new Error("MCP root must be explicitly selected with --root");
  const selected = realpathSync(resolve(root));
  if (!statSync(selected).isDirectory()) throw new Error("MCP root must be a directory");
  return { root: selected, ...(binary ? { binary } : {}) };
}

export function createServer(options: ServerOptions): McpServer {
  const selected = parseServerArgs([
    "--root", options.root,
    ...(options.binary ? ["--binary", options.binary] : []),
  ]);
  const server = new McpServer({ name: "reaterm", version: "0.0.1" });
  server.registerTool("run_command", {
    description: TOOL_DESCRIPTION,
    inputSchema: z.object({
      command: z.enum(READ_COMMANDS),
      args: z.array(z.string()),
    }).strict(),
  }, async ({ command, args }) => {
    try {
      const result = await executeReatermTool(
        { command, args },
        { root: selected.root, binary: selected.binary },
      );
      return { content: [{ type: "text", text: result }] };
    } catch (error) {
      const message = error instanceof ReaTermError ? error.message : "Invalid tool request";
      return { content: [{ type: "text", text: message }], isError: true };
    }
  });
  return server;
}

export async function main(argv: readonly string[] = process.argv.slice(2)): Promise<void> {
  const options = parseServerArgs(argv);
  await serveStdio(() => createServer(options));
}

// Start transport only when executed as the npm bin, never when imported.
if (process.argv[1] &&
    import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : "MCP server failed");
    process.exitCode = 1;
  });
}
