/**
 * Fast protocol-level MCP tests using the official v2 in-memory transport.
 * Unlike stubbing a tool callback, this exercises the real Client, Server,
 * input-schema handling and error envelopes without spawning a process.
 */
import assert from "node:assert/strict";
import test from "node:test";
import { Client, InMemoryTransport } from "@modelcontextprotocol/client";
import { createServer } from "../src/mcp-server.js";

test("in-memory MCP client rejects malformed tool requests before native execution", async () => {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = createServer({
    root: process.cwd(),
    binary: "intentionally-nonexistent-reaterm",
  });
  const client = new Client({ name: "reaterm-mock-client", version: "1.0.0" });
  try {
    await server.connect(serverTransport);
    await client.connect(clientTransport);
    const { tools } = await client.listTools();
    assert.deepEqual(tools.map((tool) => tool.name), ["run_command"]);
    const definition = tools[0]!;
    assert.ok(definition.description?.includes("read-only"));
    assert.deepEqual(definition.inputSchema.required, ["command", "args"]);

    // These payloads never reach Rust because the real MCP schema rejects
    // them, regardless of the presence or absence of the native CLI.
    for (const argumentsValue of [
      { command: "sh", args: [] },
      { command: "cat", args: "file.txt" },
      { command: "cat", args: ["file.txt"], root: "/" },
      { command: "cat" },
    ]) {
      const response = await client.callTool({
        name: "run_command",
        arguments: argumentsValue,
      });
      assert.equal(
        response.isError,
        true,
        `invalid MCP input was accepted: ${JSON.stringify(argumentsValue)}`,
      );
    }
  } finally {
    await client.close();
    await server.close();
  }
});
