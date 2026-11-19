/** Security regressions shared with the native Rust and Python adapters. */
import assert from "node:assert/strict";
import { mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";
import { ReaTerm, ReaTermError } from "../src/index.js";

const hasNative = Boolean(process.env.REATERM_BIN) &&
  ["linux", "darwin", "win32"].includes(process.platform);

test("invalid runtime argv values fail closed before a native process can start", async () => {
  const terminal = new ReaTerm({
    binary: "/definitely/nonexistent/reaterm-binary",
    decisionModel: "",
  });
  for (const args of [
    [""],
    [null],
    [123],
    ["cat", null],
    ["ls", 42],
    ["cat", { root: "/" }],
  ]) {
    await assert.rejects(
      terminal.run(...(args as [string, ...string[]])),
      /command and each argument must be strings/,
      JSON.stringify(args)
    );
  }
});

test("all native file readers deny traversal and absolute paths", { skip: !hasNative }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-ts-matrix-root-"));
  const outside = await mkdtemp(join(tmpdir(), "reaterm-ts-matrix-outside-"));
  try {
    const safe = join(root, "safe.txt");
    const secret = join(outside, "secret.txt");
    await writeFile(safe, "unchanged\n");
    await writeFile(secret, "DO_NOT_EXPOSE\n");
    const before = await stat(safe);
    const terminal = new ReaTerm({ root, binary: process.env.REATERM_BIN, decisionModel: "" });
    const relativeEscape = "../" + basename(outside) + "/secret.txt";
    for (const path of [relativeEscape, secret]) {
      for (const command of ["ls", "cat", "head", "tail", "wc", "grep", "stat"]) {
        const args = command === "grep" ? ["DO_NOT_EXPOSE", path] : [path];
        await assert.rejects(terminal.run(command, ...args), ReaTermError, command + ": " + path);
      }
    }
    const after = await stat(safe);
    assert.equal(await readFile(safe, "utf8"), "unchanged\n");
    assert.equal(await readFile(secret, "utf8"), "DO_NOT_EXPOSE\n");
    assert.equal(before.mode, after.mode);
    assert.equal(before.mtimeMs, after.mtimeMs);
    assert.deepEqual(await readdir(root), ["safe.txt"]);
  } finally {
    await rm(root, { recursive: true, force: true });
    await rm(outside, { recursive: true, force: true });
  }
});

test("native rejects shell and mutation commands without changing fixtures", { skip: !hasNative }, async () => {
  const root = await mkdtemp(join(tmpdir(), "reaterm-ts-matrix-deny-"));
  try {
    const safe = join(root, "safe.txt");
    await writeFile(safe, "unchanged\n");
    const terminal = new ReaTerm({ root, binary: process.env.REATERM_BIN, decisionModel: "" });
    for (const [command, ...args] of [
      ["sh", "-c", "echo unsafe > safe.txt"],
      ["python3", "-c", "open('PWNED','w')"],
      ["node", "-e", "require('fs').writeFileSync('PWNED','x')"],
      ["rm", "safe.txt"],
      ["touch", "PWNED"],
      ["chmod", "777", "safe.txt"],
      ["cat", "safe.txt", "--help"],
    ]) {
      await assert.rejects(terminal.run(command, ...args), ReaTermError, command);
    }
    assert.equal(await readFile(safe, "utf8"), "unchanged\n");
    assert.deepEqual(await readdir(root), ["safe.txt"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
