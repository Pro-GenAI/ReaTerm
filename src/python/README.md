# reaterm — Read-only terminal access from Python

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
- **AI-ready tool schemas:** OpenAI Chat Completions, OpenAI Responses,
  and Anthropic Messages integrations.
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

[Explore the tool integrations](../../docs/TOOL_INTEGRATIONS.md)

**Requires Python 3.11+ and a separately installed native Rust CLI.**
Supported: Linux x86-64/ARM64, macOS Intel/Apple Silicon, and Windows
x86-64. Published PyPI availability has not been verified. The native
restrictions apply to ReaTerm itself, not a parent shell.

[Website](https://reaterm.pro-genai.com/) ·
[See the security model](../../docs/THREAT_MODEL.md)
