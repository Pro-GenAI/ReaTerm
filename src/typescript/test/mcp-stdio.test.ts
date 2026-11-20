/**
 * Wire-level MCP v2 tests using the official client, not a hand-rolled
 * JSON-RPC parser or a mock MCP server. The SDK owns handshake, framing,
 * version negotiation, schema parsing and stdio child-process lifecycle.
 */
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { Client } from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";

const entrypoint = resolve(dirname(fileURLToPath(import.meta.url)), "../src/mcp-server.js");

function textOf(result: { content?: Array<{ type: string; text?: string }> }): string {
  return result.content?.map((part) => part.type === "text" ? (part.text ?? "") : "").join("") ?? "";
}

async function withMcp(
  root: string,
  binary: string,
  decisionModel: string,
  invoke: (client: Client) => Promise<void>,
): Promise<void> {
  // Do not inherit an operator's real credentials into test processes.
  const env: Record<string, string> = {};
  for (const [name, value] of Object.entries(process.env)) {
    if (value !== undefined) env[name] = value;
  }
  Object.assign(env, {
    REATERM_DECISION_MODEL: decisionModel,
    REATERM_OPENAI_API_KEY: "",
    OPENAI_API_KEY: "",
    REATERM_OPENAI_BASE_URL: "",
    OPENAI_BASE_URL: "",
  });
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [entrypoint, "--root", root, "--binary", binary],
    env,
    stderr: "pipe",
  });
  const client = new Client(
    { name: "reaterm-protocol-test", version: "1.0.0" },
    { versionNegotiation: { mode: "auto" } },
  );
  let stderr = "";
  transport.stderr?.on("data", (chunk: Buffer) => { stderr += chunk.toString("utf8"); });
  try {
    await client.connect(transport);
    await invoke(client);
  } catch (error) {
    assert.fail(`MCP protocol integration failed: ${String(error)}\n${stderr}`);
  } finally {
    await client.close();
  }
}

test("stdio SDK discovers exactly one tool and preserves the model veto", { timeout: 15000 }, async () => {
  await withMcp(
    process.cwd(),
    resolve(process.cwd(), "not-an-executable-reaterm"),
    "offline-veto-test",
    async (client) => {
      const listing = await client.listTools();
      assert.deepEqual(listing.tools.map((tool) => tool.name), ["run_command"]);
      const schema = listing.tools[0]?.inputSchema;
      assert.equal(schema?.type, "object");
      assert.ok(schema?.properties?.command);
      assert.deepEqual(schema?.required, ["command", "args"]);

      // Missing provider credentials MUST fail closed, before native spawn.
      const denied = await client.callTool({
        name: "run_command",
        arguments: { command: "pwd", args: [] },
      });
      assert.equal(denied.isError, true);
      assert.match(textOf(denied), /Decision model requires .*API_KEY/);
    },
  );
});

test("stdio SDK validates the schema and respects the native read-only boundary", {
  timeout: 15000,
  skip: !process.env.REATERM_BIN,
}, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-mcp-stdio-"));
  try {
    await writeFile(join(root, "visible.txt"), "intact\n", "utf8");
    await withMcp(root, process.env.REATERM_BIN!, "", async (client) => {
      const readable = await client.callTool({
        name: "run_command",
        arguments: { command: "cat", args: ["visible.txt"] },
      });
      assert.notEqual(readable.isError, true, textOf(readable));
      assert.equal(textOf(readable), "intact\n");

      const forbidden = [
        { command: "rm", args: ["visible.txt"] },
        { command: "cat", args: ["../outside"] },
        { command: "cat", args: ["visible.txt"], root: "/" },
      ];
      for (const args of forbidden) {
        const denied = await client.callTool({ name: "run_command", arguments: args });
        assert.equal(denied.isError, true, `unexpected MCP success: ${JSON.stringify(args)}`);
      }
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
