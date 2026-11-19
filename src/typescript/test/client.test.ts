import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { ReaTerm, ReaTermError } from "../src/index.js";

test("native read-only inspection and denied write", { skip: !process.env.REATERM_BIN }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-ts-"));
  try {
    await writeFile(join(root, "safe.txt"), "hello\n");
    const client = new ReaTerm({ root, binary: process.env.REATERM_BIN });
    assert.equal(await client.run("cat", "safe.txt"), "hello\n");
    await assert.rejects(client.run("rm", "safe.txt"), ReaTermError);
    assert.equal(await readFile(join(root, "safe.txt"), "utf8"), "hello\n");
    await assert.rejects(client.run("cat", "../safe.txt"), ReaTermError);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("model option never required for ordinary execution", () => {
  const client = new ReaTerm({ root: ".", decisionModel: "" });
  assert.equal(client.decisionModel, "");
});
