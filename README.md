<p align="center">
  <img src="assets/reaterm-cover.svg" alt="ReaTerm — Observe. Never alter. A read-only terminal built in Rust with Python and TypeScript integrations." width="100%">
</p>

# ReaTerm

**Inspect files. Not everything needs a writable shell.**

ReaTerm is a native, read-only terminal built in Rust. Instead of allowing
arbitrary shell commands, it offers a deliberately small set of
file-inspection operations. Explore a project from the terminal,
Python, or Node.js—and give AI assistants a bounded tool for reading
information without handing them an unrestricted command runner.

**[Explore the website](https://reaterm.pro-genai.com/)** ·
**[Python](src/python/README.md)** ·
**[Node.js](src/typescript/README.md)**

## Why ReaTerm?

- **Read-only by design.** Eight built-in inspection commands, no arbitrary
  program execution, and unsupported operations denied by default.
- **Defense in depth.** Capability-scoped paths, bounded output, and
  OS-backed read-only enforcement on supported systems.
- **One-line integration.** Use `reaterm.run("ls -al")` in Python or
  `await reaterm.run("ls -al")` in JavaScript.
- **Made for AI agents.** Python function-tool definitions for OpenAI
  and Anthropic, plus a local stdio MCP server for compatible AI clients.

## Simple to use

<details open>
<summary><strong>Python</strong></summary>

~~~python
import reaterm

print(reaterm.run("ls -al"))
print(reaterm.run('cat "README.md"'))
~~~

</details>

<details>
<summary><strong>Node.js / TypeScript</strong></summary>

~~~js
import * as reaterm from "reaterm";

console.log(await reaterm.run("ls -al"));
~~~

</details>

Both language adapters require the separately installed **ReaTerm native
Rust executable**. Registry publication is not verified; the examples
illustrate the APIs rather than claiming packages are available.

## A bounded tool for AI workflows

~~~python
from reaterm.tools.openai import reaterm_tool_openai
from reaterm.tools.anthropic import reaterm_tool_anthropic

# Pass the corresponding definition in your provider's tools list.
~~~

The model requests an approved inspection operation; your application
explicitly executes the request through ReaTerm and returns the result.
There is no shell. Tool results shared with a provider may disclose
readable files from your selected root.

**[See OpenAI, Anthropic & MCP examples](docs/TOOL_INTEGRATIONS.md)**

## Connect an MCP client

Use the local MCP server to offer one `run_command` inspection tool:

~~~sh
reaterm-mcp --root /absolute/path/to/project
~~~

The MCP server uses the separately installed Rust binary. The client can
request only supported read-only commands, not an arbitrary shell, and
cannot choose the directory exposed by the server. The operator chooses
that directory when starting the server.

## Small surface. Clear boundaries.

| Built-in commands | Not permitted |
| --- | --- |
| `pwd`, `ls`, `cat`, `head`, `tail`, `wc`, `grep`, `stat` | `rm`, `mv`, `cp`, shells, interpreters, arbitrary executables |

ReaTerm supports Linux x86-64/ARM64, macOS Intel/Apple Silicon, and
Windows x86-64 with platform-specific protections. It restricts **its
own process**, not the parent shell or other programs. The selected
directory is not a confidentiality barrier.

[How ReaTerm protects inspection](docs/THREAT_MODEL.md) ·
[ReaTerm website](https://reaterm.pro-genai.com/)
