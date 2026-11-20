# reaterm — Read-only terminal access for Node.js

[![MIT license](https://img.shields.io/github/license/Pro-GenAI/ReaTerm?label=license)](../../LICENSE)

<a href="https://pepy.tech/projects/reaterm"><img src="https://static.pepy.tech/personalized-badge/reaterm?period=total&amp;units=NONE&amp;left_color=BLACK&amp;right_color=GREEN&amp;left_text=PyPI%20Downloads" alt="PyPI total downloads" height="28"></a>
<a href="https://www.npmjs.com/package/reaterm"><img src="https://shieldcn.dev/npm/dt/reaterm.svg?label=npm%20downloads" alt="npm total downloads" height="28"></a>

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

**AI-agent integration:** the TypeScript package also exports strict
OpenAI Chat Completions/Responses and Anthropic tool schemas:
`reatermToolOpenAI`, `reatermToolOpenAIResponses`,
`reatermToolAnthropic`, and `executeReatermTool`. The optional
`reaterm-mcp` stdio server offers one read-only inspection tool, with the
root set by the MCP host instead of the model. The native CLI is still
required. See the [integration examples](../../docs/TOOL_INTEGRATIONS.md).

[Website](https://reaterm.pro-genai.com/) ·
[See the security model](../../docs/THREAT_MODEL.md)


## Security and installation notes

This is an **unaudited prototype**, not a production data-loss-prevention
boundary. The native executable is installed separately from the Node
adapter and must be discoverable on `PATH` or via `REATERM_BIN`.
See [source installation](../../docs/DEVELOPMENT.md).

ReaTerm denies common secret filenames by default, but does not scan file
contents or redact arbitrary credentials. Output remains untrusted model input;
it may contain prompt injection. The wrapper defaults to a 15-second native
subprocess wall timeout, whereas Unix native execution has a 10-second CPU
budget; standalone native I/O can still block. Linux requires Landlock ABI v3
(approximately kernel 6.2+). See the
[threat model](../../docs/THREAT_MODEL.md) before exposing a directory to an agent.
