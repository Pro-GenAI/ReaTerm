# reaterm — Read-only terminal access from Python

[![PyPI version](https://img.shields.io/pypi/v/reaterm?label=PyPI)](https://pypi.org/project/reaterm/)
[![npm version](https://img.shields.io/npm/v/reaterm?label=npm)](https://www.npmjs.com/package/reaterm)
[![MIT license](https://img.shields.io/github/license/Pro-GenAI/ReaTerm?label=license)](../../LICENSE)

[![Tested Linux x86-64 and ARM64](https://img.shields.io/badge/tested-Linux%20x86--64%20%26%20ARM64-2ea44f)](../../docs/TESTING.md)
[![Tested Windows x86-64](https://img.shields.io/badge/tested-Windows%20x86--64-2ea44f)](../../docs/TESTING.md)
[![Tested macOS Intel](https://img.shields.io/badge/tested-macOS%20Intel-2ea44f)](../../docs/TESTING.md)
[![Tested macOS Apple Silicon M-series](https://img.shields.io/badge/tested-macOS%20Apple%20Silicon%20(M--series)-2ea44f)](../../docs/TESTING.md)

**Give your Python app eyes into a project—not a writable shell.**

ReaTerm brings a focused, read-only command interface to Python.
Its native Rust engine implements a short inspection allowlist instead
of forwarding arbitrary strings to a shell. Choose a filesystem root,
read the information you need, and keep writes out of the native
execution path.

## Why use ReaTerm?

- **One line to inspect:** `reaterm.run("ls -al")`.
- **Everything needed for a quick look:** directory listings,
  file contents, literal text searches, and metadata.
- **AI-ready tool integrations:** OpenAI, Anthropic, and a local stdio
  Model Context Protocol (MCP) server.
- **Security by design:** fixed commands, bounded reads,
  scoped paths, and OS-backed restrictions.

## A small API that gets to the point

~~~python
import reaterm

print(reaterm.run("ls -al"))
print(reaterm.run('cat "README.md"'))
~~~

For precise argv and a reusable root:

~~~python
from reaterm import ReaTerm

terminal = ReaTerm(root=".")
print(terminal.run("grep", "-n", "TODO", "README.md"))
~~~

## Give AI assistants a read-only tool

~~~python
from reaterm.tools.openai import reaterm_tool_openai
from reaterm.tools.anthropic import reaterm_tool_anthropic

# OpenAI: tools=[reaterm_tool_openai]
# Anthropic: tools=[reaterm_tool_anthropic]
~~~

Your application executes returned tool calls explicitly and sends
results back. Tool schemas alone never run commands, and returning
file content to a provider can disclose information in the selected root.

## Connect an MCP client

~~~sh
reaterm-mcp --root /absolute/path/to/project
~~~

Compatible MCP clients can invoke the `run_command` tool using a read-only
command name and exact argv tokens. The server exposes no shell or arbitrary
executable access; the root is selected when the server starts, not by the
model. The optional MCP SDK is installed separately from the base package.

[Explore the tool integrations](../../docs/TOOL_INTEGRATIONS.md)

**Requires Python 3.11+ and a separately installed native Rust CLI.**
Supported: Linux x86-64/ARM64, macOS Intel/Apple Silicon, and Windows
x86-64. Published PyPI availability has not been verified. The native
restrictions apply to ReaTerm itself, not a parent shell.

[Website](https://reaterm.pro-genai.com/) ·
[See the security model](../../docs/THREAT_MODEL.md)
