import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { parseCommandLine, ReaTermError } from "../src/index.js";

interface Fixture {
  input: string;
  argv?: string[];
  error?: boolean;
}

const fixturePath = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../../shared/tokenizer-fixtures.json",
);
const cases = JSON.parse(readFileSync(fixturePath, "utf8")) as Fixture[];

test("single-string tokenizer matches the shared Python shlex corpus", () => {
  for (const item of cases) {
    if (item.error) {
      assert.throws(() => parseCommandLine(item.input), ReaTermError, item.input);
    } else {
      assert.deepEqual(parseCommandLine(item.input), item.argv, item.input);
    }
  }
});
