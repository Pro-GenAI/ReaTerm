"""Stdio MCP server exposing only the native ReaTerm inspection allowlist.

The trusted MCP host selects the root at process startup. Tool input cannot
change the root, executable, timeout or optional decision-model policy.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Literal, Sequence

from reaterm.tools._execution import execute_reaterm_tool
from reaterm.tools._schema import TOOL_DESCRIPTION

ReadCommand = Literal["pwd", "ls", "cat", "head", "tail", "wc", "grep", "stat"]


def create_server(*, root: str, binary: str | None = None):
    """Create a local-only MCP server bound to a trusted filesystem root."""
    # Resolve once, before accepting tool calls; missing roots fail closed.
    selected_root = Path(root).expanduser().resolve(strict=True)
    if not selected_root.is_dir():
        raise ValueError("MCP root must be a directory")

    try:
        from mcp.server.mcpserver import MCPServer
    except ImportError as exc:
        raise RuntimeError("MCP SDK missing: install 'reaterm[mcp]' (mcp>=2,<3)") from exc

    server = MCPServer(
        "reaterm",
        instructions=(
            "Read-only inspection of the root chosen by the server operator. "
            "Only native ReaTerm commands are available; no shell or writes. "
            "File contents returned to the client may be sensitive."
        ),
    )

    @server.tool(name="run_command", description=TOOL_DESCRIPTION)
    def run_command(command: ReadCommand, args: list[str]) -> str:
        """Run one allowlisted inspection command with exact argv tokens."""
        return execute_reaterm_tool(
            {"command": command, "args": args}, root=str(selected_root), binary=binary
        )

    return server


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description="ReaTerm read-only MCP stdio server")
    parser.add_argument(
        "--root", required=True, help="Trusted directory visible to MCP tool calls"
    )
    parser.add_argument(
        "--binary", help="Path to the separately installed native ReaTerm executable"
    )
    options = parser.parse_args(argv)
    try:
        server = create_server(root=options.root, binary=options.binary)
    except (OSError, ValueError, RuntimeError) as exc:
        parser.error(str(exc))
    server.run(transport="stdio")


if __name__ == "__main__":
    main()
