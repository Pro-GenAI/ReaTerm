"""Rigorous Python adapter tests; no real provider calls or host writes.

Run:
    python -m unittest discover -s src/python/tests -v
Set REATERM_BIN to the compiled native CLI to enable live integration tests.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from reaterm import ReaTerm, ReaTermError


def fake_openai(reply="ALLOW", error=None):
    """Fabricate the official SDK import surface without network access."""
    module = types.ModuleType("openai")
    client = Mock()
    if error:
        client.chat.completions.create.side_effect = error
    else:
        result = types.SimpleNamespace(
            choices=[types.SimpleNamespace(message=types.SimpleNamespace(content=reply))]
        )
        client.chat.completions.create.return_value = result
    module.OpenAI = Mock(return_value=client)
    return module, client


class SecurityAdapterTests(unittest.TestCase):
    def test_args_not_shell_interpreted_or_split(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch("reaterm.client.subprocess.run") as execute:
                execute.return_value.returncode = 0
                execute.return_value.stdout = "safe\n"
                argv = ("cat", "$(touch HACK)", "; rm -rf .", "file name")
                value = ReaTerm(binary="/opt/greaterm", root=".").run(*argv)
                self.assertEqual(value, "safe\n")
                self.assertEqual(execute.call_args.args[0][-len(argv):], list(argv))
                self.assertIs(execute.call_args.kwargs["shell"], False)
                self.assertIs(execute.call_args.kwargs["check"], False)

    def test_broken_native_binary_fails_closed(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch("reaterm.client.subprocess.run", side_effect=FileNotFoundError("missing")):
                with self.assertRaisesRegex(ReaTermError, "execution failed"):
                    ReaTerm(binary="missing").run("ls")

    def test_native_timeout_fails_closed(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch(
                "reaterm.client.subprocess.run",
                side_effect=subprocess.TimeoutExpired(cmd=["reaterm"], timeout=1),
            ):
                with self.assertRaisesRegex(ReaTermError, "execution failed"):
                    ReaTerm(binary="reaterm", timeout=1).run("ls")

    def test_reject_invalid_argv_values(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            term = ReaTerm(binary="reaterm")
            for command in ("", None, 123):
                with self.subTest(command=command), self.assertRaises(ValueError):
                    term.run(command)
            with self.assertRaises(ValueError):
                term.run("ls", None)

    def test_model_deny_never_spawns_binary(self):
        fake, client = fake_openai("DENY")
        with patch.dict(
            os.environ,
            {"REATERM_DECISION_MODEL": "test-model", "REATERM_OPENAI_API_KEY": "test-key"},
            clear=True,
        ), patch.dict(sys.modules, {"openai": fake}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            with self.assertRaisesRegex(ReaTermError, "denied by optional"):
                ReaTerm(binary="reaterm").run("ls")
            execute.assert_not_called()
            client.chat.completions.create.assert_called_once()

    def test_model_can_never_override_native_denial(self):
        fake, _ = fake_openai("ALLOW")
        with patch.dict(
            os.environ,
            {"REATERM_DECISION_MODEL": "test-model", "REATERM_OPENAI_API_KEY": "test-key"},
            clear=True,
        ), patch.dict(sys.modules, {"openai": fake}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 2
            execute.return_value.stderr = "reaterm: denied: chmod"
            with self.assertRaisesRegex(ReaTermError, "denied"):
                ReaTerm(binary="reaterm").run("chmod", "777", "file")
            execute.assert_called_once()

    def test_model_malformed_or_empty_responses_fail_closed(self):
        for reply in ("", "allow", "ALLOWED", "ALLOW now", "DENY", "YES"):
            fake, _ = fake_openai(reply)
            with self.subTest(reply=reply), patch.dict(
                os.environ,
                {"REATERM_DECISION_MODEL": "test-model", "REATERM_OPENAI_API_KEY": "test-key"},
                clear=True,
            ), patch.dict(sys.modules, {"openai": fake}), patch(
                "reaterm.client.subprocess.run"
            ) as execute:
                with self.assertRaises(ReaTermError):
                    ReaTerm(binary="reaterm").run("ls")
                execute.assert_not_called()

    def test_model_api_exception_fails_closed(self):
        fake, _ = fake_openai(error=RuntimeError("offline"))
        with patch.dict(
            os.environ,
            {"REATERM_DECISION_MODEL": "test-model", "REATERM_OPENAI_API_KEY": "test-key"},
            clear=True,
        ), patch.dict(sys.modules, {"openai": fake}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            with self.assertRaisesRegex(ReaTermError, "model veto unavailable"):
                ReaTerm(binary="reaterm").run("ls")
            execute.assert_not_called()

    def test_custom_openai_env_precedence(self):
        fake, _ = fake_openai("ALLOW")
        with patch.dict(
            os.environ,
            {
                "REATERM_DECISION_MODEL": "custom-model",
                "REATERM_OPENAI_API_KEY": "custom-key",
                "REATERM_OPENAI_BASE_URL": "http://127.0.0.1:9999/v1",
                "OPENAI_API_KEY": "fallback-key",
                "OPENAI_BASE_URL": "http://127.0.0.1:8888/v1",
            },
            clear=True,
        ), patch.dict(sys.modules, {"openai": fake}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 0
            execute.return_value.stdout = "ok"
            self.assertEqual(ReaTerm(binary="reaterm").run("ls"), "ok")
            fake.OpenAI.assert_called_once_with(
                api_key="custom-key", base_url="http://127.0.0.1:9999/v1"
            )

    def test_missing_model_credentials_fail_before_launch(self):
        fake, _ = fake_openai()
        with patch.dict(
            os.environ, {"REATERM_DECISION_MODEL": "model"}, clear=True
        ), patch.dict(sys.modules, {"openai": fake}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            with self.assertRaisesRegex(ReaTermError, "API_KEY"):
                ReaTerm(binary="reaterm").run("ls")
            execute.assert_not_called()


@unittest.skipUnless(sys.platform.startswith(("linux", "darwin", "win32")) and os.environ.get("REATERM_BIN"),
                     "requires supported native CLI; set REATERM_BIN")
class RealNativeBridgeTests(unittest.TestCase):
    def test_native_bridge_reads_and_refuses_mutations(self):
        with tempfile.TemporaryDirectory(prefix="reaterm-py-") as directory:
            path = Path(directory) / "literal;touch BAD"
            path.write_text("intact\n", encoding="utf-8")
            binary = os.environ["REATERM_BIN"]
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                client = ReaTerm(root=directory, binary=binary)
                self.assertEqual(client.run("cat", path.name), "intact\n")
                for operation in ("--help", "--version", "--limits", "--about",
                                  "rm", "chmod", "touch", "chown", "mv"):
                    with self.subTest(operation=operation), self.assertRaises(ReaTermError):
                        client.run(operation, path.name)
                with self.assertRaises(ReaTermError):
                    client.run("cat", "../outside")
            self.assertEqual(path.read_text(encoding="utf-8"), "intact\n")
            self.assertFalse((Path(directory) / "BAD").exists())
