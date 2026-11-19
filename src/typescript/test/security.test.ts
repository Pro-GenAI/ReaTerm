/**
 * Defense-in-depth integration tests for the Node ESM adapter.
 * Mock model endpoint is bound only to local loopback; no provider calls.
 * Integration tests require a compiled native binary in REATERM_BIN.
 */
import assert from "node:assert/strict";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { ReaTerm, ReaTermError } from "../src/index.js";

const hasNative = Boolean(process.env.REATERM_BIN) && ["linux", "darwin", "win32"].includes(process.platform);

test("direct argv preserves metacharacters instead of running a shell", { skip: !hasNative }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-argv-"));
  try {
    const name = "literal;touch PWNED";
    await writeFile(join(root, name), "intact\n");
    const terminal = new ReaTerm({ root, binary: process.env.REATERM_BIN, decisionModel: "" });
    assert.equal(await terminal.run("cat", name), "intact\n");
    assert.deepEqual(await readdir(root), [name]);
    await assert.rejects(terminal.run("cat", "../outside"), ReaTermError);
    await assert.rejects(terminal.run("cat", "$(touch PWNED)"), ReaTermError);
    assert.deepEqual(await readdir(root), [name]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("native mutation commands are denied and file modes stay unchanged", { skip: !hasNative }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-policy-"));
  try {
    const file = join(root, "file.txt");
    await writeFile(file, "protected\n");
    const mode = (await stat(file)).mode;
    const terminal = new ReaTerm({ root, binary: process.env.REATERM_BIN, decisionModel: "" });
    for (const command of ["--help", "--version", "--limits", "--about", "rm", "cp", "mv", "touch", "mkdir", "chmod", "chown", "chgrp", "truncate", "tee", "sh"]) {
      await assert.rejects(terminal.run(command, "file.txt"), ReaTermError, command);
    }
    assert.equal(await terminal.run("cat", "file.txt"), "protected\n");
    assert.equal((await stat(file)).mode, mode);
    assert.deepEqual(await readdir(root), ["file.txt"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("missing native executable rejects instead of running a fallback shell", async () => {
  const terminal = new ReaTerm({
    binary: "/definitely/nonexistent/reaterm-executable",
    decisionModel: "",
  });
  await assert.rejects(terminal.run("ls"), ReaTermError);
});

test("model without key denies before launching binary", async () => {
  const terminal = new ReaTerm({
    binary: "/definitely/nonexistent/reaterm-executable",
    decisionModel: "test-model",
    apiKey: "",
  });
  await assert.rejects(terminal.run("ls"), /API_KEY/);
});

/** Stand up a local fake OpenAI-compatible completion API for SDK testing. */
async function localDecisionServer(
  selected: { verdict: string },
  received: Array<{ authorization: string | undefined; body: Record<string, unknown> }>
) {
  const server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    let body: Record<string, unknown>;
    try {
      body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<string, unknown>;
    } catch {
      res.writeHead(400).end();
      return;
    }
    received.push({ authorization: req.headers.authorization, body });
    res.writeHead(200, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        id: "chatcmpl-reaterm-test",
        object: "chat.completion",
        created: 1,
        model: "local-test",
        choices: [
          {
            index: 0,
            message: { role: "assistant", content: selected.verdict },
            finish_reason: "stop",
          },
        ],
      })
    );
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const port = (server.address() as AddressInfo).port;
  return {
    baseURL: "http://127.0.0.1:" + port + "/v1",
    close: () => new Promise<void>((resolve, reject) => {
      server.close((err) => (err ? reject(err) : resolve()));
    }),
  };
}

test("official OpenAI SDK can veto but never override native allowlist", { skip: !hasNative }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-model-"));
  const selected = { verdict: "DENY" };
  const received: Array<{ authorization: string | undefined; body: Record<string, unknown> }> = [];
  const local = await localDecisionServer(selected, received);
  try {
    await writeFile(join(root, "protected"), "unchanged\n");
    const terminal = new ReaTerm({
      root,
      binary: process.env.REATERM_BIN,
      decisionModel: "local-test",
      apiKey: "test-local-key",
      baseURL: local.baseURL,
    });
    await assert.rejects(terminal.run("cat", "protected"), /denied by optional/);
    selected.verdict = "ALLOW";
    assert.equal(await terminal.run("cat", "protected"), "unchanged\n");
    await assert.rejects(terminal.run("chmod", "777", "protected"), ReaTermError);
    assert.equal(await readFile(join(root, "protected"), "utf8"), "unchanged\n");
    assert.equal(received.length, 3);
    assert.equal(received[0].authorization, "Bearer test-local-key");
    assert.equal(received[0].body.model, "local-test");
    assert.ok(JSON.stringify(received[2].body.messages).includes("chmod"));
  } finally {
    await local.close();
    await rm(root, { recursive: true, force: true });
  }
});
