import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import * as reaterm from "../src/index.js";
import { ReaTerm, ReaTermError, run } from "../src/index.js";

test("npm package exposes the standalone run function", () => {
  assert.equal(run, reaterm.run);
  assert.equal(typeof run, "function");
});

test("single-string API rejects shell operators and malformed quoting", async () => {
  const options = { binary: "/definitely/missing/reaterm", decisionModel: "" };
  for (const input of [
    "", "  ", 'cat "unfinished', "ls;", "ls | cat", "ls > out",
    "ls && cat", "cat $(touch BAD)", "ls\ncat other",
    "ls \x60whoami\x60", "ls\\", '"" ls',
  ]) {
    await assert.rejects(run(input, options), ReaTermError, JSON.stringify(input));
  }
  await assert.rejects(run(null as unknown as string, options), ReaTermError);
});

test("shorthand matches argv API for ls -al and quoted files", {
  skip: !process.env.REATERM_BIN,
}, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-convenience-"));
  try {
    await writeFile(join(root, "file name.txt"), "safe\n");
    const options = { root, binary: process.env.REATERM_BIN, decisionModel: "" };
    const term = new ReaTerm(options);
    assert.equal(await run("ls -al", options), await term.run("ls", "-al"));
    assert.equal(await run('cat "file name.txt"', options), "safe\n");
    assert.equal(await run("cat file\\ name.txt", options), "safe\n");
    await assert.rejects(run("rm file\\ name.txt", options), ReaTermError);
    assert.equal(await readFile(join(root, "file name.txt"), "utf8"), "safe\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
