"""Offline provider tool-schema and execution tests."""
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from reaterm import ReaTermError
from reaterm.tools import execute_reaterm_tool
from reaterm.tools.openai import (
    reaterm as openai_alias,
    reaterm_tool_openai,
    reaterm_tool_openai_responses,
)
from reaterm.tools.anthropic import reaterm as anthropic_alias, reaterm_tool_anthropic


class ProviderSchemas(unittest.TestCase):
    def test_openai_chat_completions_tool(self):
        self.assertIs(openai_alias, reaterm_tool_openai)
        self.assertEqual(reaterm_tool_openai["type"], "function")
        function = reaterm_tool_openai["function"]
        self.assertEqual(function["name"], "reaterm")
        self.assertTrue(function["strict"])
        self.assertEqual(function["parameters"]["required"], ["command", "args"])
        self.assertFalse(function["parameters"]["additionalProperties"])
        self.assertIn("ls", function["parameters"]["properties"]["command"]["enum"])
        self.assertNotIn("rm", function["parameters"]["properties"]["command"]["enum"])

    def test_openai_responses_tool(self):
        tool = reaterm_tool_openai_responses
        self.assertEqual(tool["type"], "function")
        self.assertEqual(tool["name"], "reaterm")
        self.assertNotIn("function", tool)
        self.assertEqual(tool["parameters"], reaterm_tool_openai["function"]["parameters"])
        self.assertTrue(tool["strict"])

    def test_anthropic_messages_tool(self):
        self.assertIs(anthropic_alias, reaterm_tool_anthropic)
        self.assertEqual(reaterm_tool_anthropic["name"], "reaterm")
        self.assertNotIn("type", reaterm_tool_anthropic)
        self.assertEqual(
            reaterm_tool_anthropic["input_schema"],
            reaterm_tool_openai["function"]["parameters"],
        )


class Execution(unittest.TestCase):
    def test_openai_json_and_anthropic_mapping_dispatch_exact_argv(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 0
            execute.return_value.stdout = "contents\n"
            for payload in (
                '{"command": "ls", "args": ["-al"]}',
                {"command": "ls", "args": ["-al"]},
            ):
                with self.subTest(payload=payload):
                    self.assertEqual(
                        execute_reaterm_tool(payload, root=".", binary="/opt/bin/reaterm"),
                        "contents\n",
                    )
                    self.assertEqual(execute.call_args.args[0][-2:], ["ls", "-al"])
                    self.assertEqual(execute.call_args.args[0][1], "--root")
                    self.assertFalse(execute.call_args.kwargs["shell"])

    def test_literal_argv_metacharacters_are_not_shell_executed(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 0
            execute.return_value.stdout = "safe\n"
            filename = "file;$(touch PWNED)"
            self.assertEqual(
                execute_reaterm_tool({"command": "cat", "args": [filename]}, binary="reaterm"),
                "safe\n",
            )
            self.assertEqual(execute.call_args.args[0][-2:], ["cat", filename])
            self.assertFalse(execute.call_args.kwargs["shell"])

    def test_invalid_model_inputs_rejected_before_native_execution(self):
        invalid = [
            None, {}, [], "invalid json", "[1]", '{"command":"ls"}',
            {"command": "ls"}, {"command": "ls", "args": [], "root": "/"},
            {"command": "ls -al", "args": []}, {"command": "rm", "args": ["file"]},
            {"command": "sh", "args": []}, {"command": 2, "args": []},
            {"command": "ls", "args": "-al"}, {"command": "ls", "args": [None]},
            {"command": "ls", "args": [42]},
            {"command": "ls", "args": [], "binary": "/bin/sh"},
        ]
        with patch("reaterm.tools._execution.ReaTerm") as client:
            for payload in invalid:
                with self.subTest(payload=payload), self.assertRaises(ValueError):
                    execute_reaterm_tool(payload)
            client.assert_not_called()

    def test_native_denial_propagates(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 2
            execute.return_value.stderr = "denied"
            with self.assertRaises(ReaTermError):
                execute_reaterm_tool(
                    {"command": "ls", "args": ["--unsafe"]}, binary="reaterm"
                )

    def test_model_cannot_select_root_or_binary(self):
        with patch("reaterm.tools._execution.ReaTerm") as client:
            client.return_value.run.return_value = "ok"
            self.assertEqual(
                execute_reaterm_tool(
                    '{"command":"pwd","args":[]}',
                    root="/sandbox",
                    binary="/opt/reaterm",
                    timeout=3,
                    decision_model="my-veto",
                ),
                "ok",
            )
            client.assert_called_once_with(
                root="/sandbox", binary="/opt/reaterm", timeout=3, decision_model="my-veto"
            )
            client.return_value.run.assert_called_once_with("pwd")

    @unittest.skipUnless(os.environ.get("REATERM_BIN"), "requires native ReaTerm CLI")
    def test_real_native_tool_reads_and_rejects_unsafe_operations(self):
        import tempfile

        with tempfile.TemporaryDirectory(prefix="reaterm-provider-tools-") as directory:
            path = Path(directory) / "file;$(touch PWNED)"
            path.write_text("intact\n", encoding="utf-8")
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                self.assertEqual(
                    execute_reaterm_tool(
                        {"command": "cat", "args": [path.name]}, root=directory
                    ),
                    "intact\n",
                )
                with self.assertRaises(ReaTermError):
                    execute_reaterm_tool(
                        {"command": "cat", "args": ["../outside"]}, root=directory
                    )
                with self.assertRaises(ReaTermError):
                    execute_reaterm_tool(
                        {"command": "ls", "args": ["--not-an-option"]}, root=directory
                    )
            self.assertEqual(path.read_text(encoding="utf-8"), "intact\n")
            self.assertFalse((Path(directory) / "PWNED").exists())

    def test_schemas_do_not_require_provider_imports(self):
        self.assertTrue(json.dumps(reaterm_tool_openai))
        self.assertTrue(json.dumps(reaterm_tool_anthropic))


if __name__ == "__main__":
    unittest.main()
