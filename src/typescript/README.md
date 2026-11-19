# reaterm — Read-only terminal access for Node.js

[![PyPI version](https://img.shields.io/pypi/v/reaterm?label=PyPI)](https://pypi.org/project/reaterm/)
[![npm version](https://img.shields.io/npm/v/reaterm?label=npm)](https://www.npmjs.com/package/reaterm)
[![MIT license](https://img.shields.io/github/license/Pro-GenAI/ReaTerm?label=license)](../../LICENSE)

[![Tested Linux x86-64 and ARM64](https://img.shields.io/badge/tested-Linux%20x86--64%20%26%20ARM64-2ea44f)](../../docs/TESTING.md)
[![Tested Windows x86-64](https://img.shields.io/badge/tested-Windows%20x86--64-2ea44f)](../../docs/TESTING.md)
[![Tested macOS Intel](https://img.shields.io/badge/tested-macOS%20Intel-2ea44f)](../../docs/TESTING.md)
[![Tested macOS Apple Silicon M-series](https://img.shields.io/badge/tested-macOS%20Apple%20Silicon%20(M--series)-2ea44f)](../../docs/TESTING.md)

**Explore files from JavaScript without offering an arbitrary shell.**

ReaTerm provides an asynchronous Node.js and TypeScript interface backed
by a native Rust read-only terminal. It passes inspected commands as
individual arguments, uses a fixed command allowlist, and restricts
filesystem access through OS-backed protections.

## Why ReaTerm?

- **A one-line API:** `await reaterm.run("ls -al")`.
- **The commands you actually need for inspection:** list directories,
  read files, search literal text, and inspect metadata.
- **Predictable access:** select a root; unsupported commands,
  dangerous flags, and unsafe paths are denied.
- **Optional AI veto:** an OpenAI-compatible model can refuse
  an operation but cannot grant capabilities denied by Rust.

## See it in action

The npm package is named **`reaterm`** (unscoped).

~~~js
import * as reaterm from "reaterm";

console.log(await reaterm.run("ls -al"));
console.log(await reaterm.run('cat "README.md"', { root: "." }));
~~~

For an explicit argument interface:

~~~ts
import { ReaTerm } from "reaterm";

const terminal = new ReaTerm({ root: "." });
console.log(await terminal.run("grep", "-n", "TODO", "README.md"));
~~~

Both interfaces return a `Promise<string>` and invoke the native
read-only engine without a shell.

**Requires Node.js 20+ and the separately installed ReaTerm Rust CLI.**
Supported: Linux x86-64/ARM64, macOS Intel/Apple Silicon, and Windows
x86-64. Published npm availability has not been verified. The protection
applies to the ReaTerm process, not the caller's shell.

Looking for OpenAI or Anthropic function-tool schemas?
Those are currently offered by the [Python integration](../python/README.md).

[Website](https://reaterm.pro-genai.com/) ·
[See the security model](../../docs/THREAT_MODEL.md)
