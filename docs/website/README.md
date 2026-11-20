# ReaTerm — A terminal built to look, not touch

[![PyPI version](https://img.shields.io/pypi/v/reaterm?label=PyPI)](https://pypi.org/project/reaterm/)
[![npm version](https://img.shields.io/npm/v/reaterm?label=npm)](https://www.npmjs.com/package/reaterm)
[![MIT license](https://img.shields.io/github/license/Pro-GenAI/ReaTerm?label=license)](../../LICENSE)

<a href="https://pepy.tech/projects/reaterm"><img src="https://static.pepy.tech/personalized-badge/reaterm?period=total&amp;units=NONE&amp;left_color=BLACK&amp;right_color=GREEN&amp;left_text=PyPI%20Downloads" alt="PyPI total downloads" height="28"></a>
<a href="https://www.npmjs.com/package/reaterm"><img src="https://shieldcn.dev/npm/dt/reaterm.svg?label=npm%20downloads" alt="npm total downloads" height="28"></a>

[![Tested Linux x86-64 and ARM64](https://img.shields.io/badge/tested-Linux%20x86--64%20%26%20ARM64-2ea44f)](../TESTING.md)
[![Tested Windows x86-64](https://img.shields.io/badge/tested-Windows%20x86--64-2ea44f)](../TESTING.md)
[![Tested macOS Intel](https://img.shields.io/badge/tested-macOS%20Intel-2ea44f)](../TESTING.md)
[![Tested macOS Apple Silicon M-series](https://img.shields.io/badge/tested-macOS%20Apple%20Silicon%20(M--series)-2ea44f)](../TESTING.md)

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
