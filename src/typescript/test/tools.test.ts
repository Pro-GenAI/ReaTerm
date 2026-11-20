import assert from "node:assert/strict";
import test from "node:test";
import {
  inputSchema, reatermToolOpenAI, reatermToolOpenAIResponses,
  reatermToolAnthropic, executeReatermTool,
} from "../src/index.js";
import { parseServerArgs, createServer } from "../src/mcp-server.js";

test("provider tools share one strict argv schema", () => {
  assert.equal(inputSchema.additionalProperties, false);
  assert.deepEqual(inputSchema.required, ["command", "args"]);
  assert.equal(reatermToolOpenAI.function.strict, true);
  assert.equal(reatermToolOpenAI.function.parameters, inputSchema);
  assert.equal(reatermToolOpenAIResponses.parameters, inputSchema);
  assert.equal(reatermToolAnthropic.input_schema, inputSchema);
});

test("model input cannot select root, binary or unrecognized commands", async () => {
  const options = { root: ".", binary: "/definitely/nonexistent", decisionModel: "" };
  for (const input of [
    {}, null, [], "garbage",
    { command: "rm", args: [] },
    { command: "ls", args: "anything" },
    { command: "ls", args: [], root: "/secret" },
    { command: "ls", args: [], binary: "/bin/bash" },
    { command: "ls", args: [10] },
  ]) {
    await assert.rejects(executeReatermTool(input, options));
  }
  await assert.rejects(
    executeReatermTool({ command: "ls", args: [] }, options),
    /Unable to launch/,
  );
});

test("MCP launcher requires a trusted existing root and strict argv", () => {
  assert.throws(() => parseServerArgs([]), /--root/);
  assert.throws(() => parseServerArgs(["--root", "/not/a/real/root"]));
  assert.throws(() => parseServerArgs(["--root", ".", "--unsafe", "value"]));
  assert.throws(() => parseServerArgs(["--root", ".", "--root", "/tmp"]));
  const options = parseServerArgs(["--root", "."]);
  assert.ok(options.root);
  assert.ok(createServer(options));
});
