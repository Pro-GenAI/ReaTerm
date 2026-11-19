# ReaTerm — A terminal built to look, not touch

**[Explore ReaTerm](https://reaterm.pro-genai.com/)**

ReaTerm helps applications and AI agents inspect files and directories
without providing an unrestricted shell. Built in Rust, it offers
a small, explicit set of read-only commands with scoped paths and
OS-backed safeguards.

## What makes it different?

- **Purpose-built inspection:** eight read-only commands instead of
  arbitrary executables.
- **Security-focused design:** scoped roots, bounded output,
  and native enforcement on supported systems.
- **Easy access:** one-line Python and Node.js APIs.
- **AI workflows:** Python function-tool definitions for OpenAI
  and Anthropic backed by the same native engine.

The website walks through ReaTerm's approach, shows copyable
language examples, and links to the supported integrations.

ReaTerm limits its **own native process**, not the caller's shell.
The Python and JavaScript packages require the separately installed
Rust executable.

**[Try ReaTerm](../../README.md)** ·
**[Python](../../src/python/README.md)** ·
**[Node.js](../../src/typescript/README.md)** ·
**[Security](../THREAT_MODEL.md)**
