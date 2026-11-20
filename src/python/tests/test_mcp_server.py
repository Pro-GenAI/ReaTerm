"""Offline MCP adapter tests; no MCP SDK or external services required."""

import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from reaterm import ReaTermError
from reaterm.mcp_server import create_server, main, ReadCommand
from reaterm.tools._schema import READ_COMMANDS
from typing import get_args


class FakeMCPServer:
    def __init__(self, name, **kwargs):
        self.name = name
        self.options = kwargs
        self.handlers = {}
        self.tool_options = {}
        self.transport = None

    def tool(self, **kwargs):
        def register(fn):
            self.handlers[kwargs["name"]] = fn
            self.tool_options[kwargs["name"]] = kwargs
            return fn
        return register

    def run(self, *, transport):
        self.transport = transport


def fake_mcp_sdk():
    """MCP v2 import shim: unit tests do not install optional dependencies."""
    package = types.ModuleType("mcp")
    package.__path__ = []
    subpackage = types.ModuleType("mcp.server")
    subpackage.__path__ = []
    module = types.ModuleType("mcp.server.mcpserver")
    module.MCPServer = FakeMCPServer
    return patch.dict(sys.modules, {
        "mcp": package, "mcp.server": subpackage, "mcp.server.mcpserver": module
    })


class MCPServerTests(unittest.TestCase):
    def test_tool_annotation_matches_native_allowlist(self):
        self.assertEqual(set(get_args(ReadCommand)), READ_COMMANDS)

    def test_root_is_required_and_invalid_roots_fail_closed(self):
        with patch("sys.stderr"):
            with self.assertRaises(SystemExit) as missing:
                main([])
        self.assertEqual(missing.exception.code, 2)
        with tempfile.TemporaryDirectory() as directory:
            file_path = Path(directory) / "file.txt"
            file_path.write_text("x", encoding="utf-8")
            with self.assertRaises(ValueError):
                create_server(root=str(file_path))
            with self.assertRaises(FileNotFoundError):
                create_server(root=str(Path(directory) / "missing"))

    def test_server_registers_exactly_one_scoped_argv_tool(self):
        with tempfile.TemporaryDirectory() as directory, fake_mcp_sdk(), patch(
            "reaterm.mcp_server.execute_reaterm_tool", return_value="contents\n"
        ) as execute:
            server = create_server(root=directory, binary="/trusted/reaterm")
            self.assertEqual(server.name, "reaterm")
            self.assertEqual(list(server.handlers), ["run_command"])
            self.assertIn("read-only", server.options["instructions"].lower())
            self.assertEqual(server.handlers["run_command"]("ls", ["-al"]), "contents\n")
            execute.assert_called_once_with(
                {"command": "ls", "args": ["-al"]},
                root=str(Path(directory).resolve()), binary="/trusted/reaterm"
            )

    def test_tool_denials_propagate_and_never_invoke_shell(self):
        with tempfile.TemporaryDirectory() as directory, fake_mcp_sdk(), patch.dict(
            os.environ, {"REATERM_DECISION_MODEL": ""}
        ), patch("reaterm.client._capture_capped") as execute:
            server = create_server(root=directory, binary="/trusted/reaterm")
            tool = server.handlers["run_command"]
            execute.return_value.returncode = 0
            execute.return_value.stdout = "read\n"
            self.assertEqual(tool("cat", ["file;$(touch PWNED)"]), "read\n")
            self.assertEqual(execute.call_args.args[0][-2:], ["cat", "file;$(touch PWNED)"])
            with self.assertRaises(ValueError):
                tool("sh", [])
            with self.assertRaises(ValueError):
                tool("ls", {"root": "/"})
            execute.return_value.returncode = 2
            execute.return_value.stderr = "denied"
            with self.assertRaises(ReaTermError):
                tool("cat", ["../outside"])

    def test_entrypoint_uses_stdio_only(self):
        with tempfile.TemporaryDirectory() as directory, patch(
            "reaterm.mcp_server.create_server"
        ) as factory:
            main(["--root", directory, "--binary", "/trusted/reaterm"])
            factory.assert_called_once_with(root=directory, binary="/trusted/reaterm")
            factory.return_value.run.assert_called_once_with(transport="stdio")


if __name__ == "__main__":
    unittest.main()
