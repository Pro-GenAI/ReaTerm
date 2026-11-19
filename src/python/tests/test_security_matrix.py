"""Cross-adapter security regression matrix; no external model calls."""

import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from reaterm import ReaTermError
from reaterm.tools import execute_reaterm_tool


OPERATIONS = (
    ("pwd", []),
    ("ls", ["-al"]),
    ("cat", ["file;$(touch PWNED)"]),
    ("head", ["-n", "1", "file;$(touch PWNED)"]),
    ("tail", ["-n", "1", "file;$(touch PWNED)"]),
    ("wc", ["-l", "file;$(touch PWNED)"]),
    ("grep", ["-n", "needle", "file;$(touch PWNED)"]),
    ("stat", ["file;$(touch PWNED)"]),
)


class OfflineCommandSecurity(unittest.TestCase):
    def test_every_allowed_tool_command_preserves_argv_and_trusted_root(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(
            os.environ, {"REATERM_DECISION_MODEL": ""}
        ), patch("reaterm.client.subprocess.run") as execute:
            execute.return_value.returncode = 0
            execute.return_value.stdout = "safe\n"
            for command, args in OPERATIONS:
                with self.subTest(command=command):
                    self.assertEqual(
                        execute_reaterm_tool(
                            {"command": command, "args": args},
                            root=root,
                            binary="/trusted/reaterm",
                            timeout=2,
                        ),
                        "safe\n",
                    )
                    self.assertEqual(
                        execute.call_args.args[0],
                        ["/trusted/reaterm", "--root", str(Path(root).resolve()), command, *args],
                    )
                    self.assertFalse(execute.call_args.kwargs["shell"])
                    self.assertEqual(execute.call_args.kwargs["timeout"], 2)
            self.assertFalse((Path(root) / "PWNED").exists())

    def test_tool_arguments_cannot_override_root_binary_or_command_allowlist(self):
        invalid = (
            {"command": "ls", "args": [], "root": "/"},
            {"command": "ls", "args": [], "binary": "/bin/sh"},
            {"command": "ls", "args": [], "timeout": 0},
            {"command": "ls", "args": [], "decision_model": "allow-all"},
            {"command": "sh", "args": ["-c", "touch PWNED"]},
            {"command": "rm", "args": ["file"]},
            {"command": "ls -al", "args": []},
            {"command": "cat", "args": [42]},
            {"command": "cat", "args": "file"},
        )
        with patch("reaterm.tools._execution.ReaTerm") as client:
            for payload in invalid:
                with self.subTest(payload=payload), self.assertRaises(ValueError):
                    execute_reaterm_tool(payload, root="/trusted")
            client.assert_not_called()

    def test_native_failure_never_returns_partial_stdout(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}), patch(
            "reaterm.client.subprocess.run"
        ) as execute:
            execute.return_value.returncode = 2
            execute.return_value.stdout = "SENSITIVE_CONTENT\n"
            execute.return_value.stderr = "reaterm: denied"
            with self.assertRaisesRegex(ReaTermError, "denied"):
                execute_reaterm_tool(
                    {"command": "cat", "args": ["../private.txt"]},
                    binary="/trusted/reaterm",
                )


@unittest.skipUnless(os.environ.get("REATERM_BIN"), "requires native ReaTerm binary")
class NativeCommandSecurity(unittest.TestCase):
    def test_all_file_readers_deny_absolute_and_parent_escape(self):
        with tempfile.TemporaryDirectory(prefix="reaterm-matrix-root-") as root, \
             tempfile.TemporaryDirectory(prefix="reaterm-matrix-outside-") as outside:
            protected = Path(root) / "protected.txt"
            protected.write_text("unchanged\n", encoding="utf-8")
            secret = Path(outside) / "secret.txt"
            secret.write_text("DO_NOT_EXPOSE\n", encoding="utf-8")
            before = protected.stat()
            relative = "../" + Path(outside).name + "/secret.txt"
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                for path in (relative, str(secret)):
                    for command in ("ls", "cat", "head", "tail", "wc", "grep", "stat"):
                        args = ["DO_NOT_EXPOSE", path] if command == "grep" else [path]
                        with self.subTest(path=path, command=command):
                            with self.assertRaises(ReaTermError):
                                execute_reaterm_tool(
                                    {"command": command, "args": args}, root=root
                                )
            after = protected.stat()
            self.assertEqual(protected.read_text(encoding="utf-8"), "unchanged\n")
            self.assertEqual(secret.read_text(encoding="utf-8"), "DO_NOT_EXPOSE\n")
            self.assertEqual(before.st_mode, after.st_mode)
            self.assertEqual(before.st_mtime_ns, after.st_mtime_ns)
            self.assertEqual(sorted(p.name for p in Path(root).iterdir()), ["protected.txt"])

    def test_final_symlinks_are_not_read_by_content_commands(self):
        with tempfile.TemporaryDirectory(prefix="reaterm-link-root-") as root, \
             tempfile.TemporaryDirectory(prefix="reaterm-link-outside-") as outside:
            target = Path(outside) / "secret.txt"
            target.write_text("DO_NOT_EXPOSE\n", encoding="utf-8")
            link = Path(root) / "link.txt"
            try:
                link.symlink_to(target)
            except (OSError, NotImplementedError) as exc:
                self.skipTest(f"symlink creation unavailable: {exc}")
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                for command in ("cat", "head", "tail", "wc", "grep"):
                    args = ["DO_NOT_EXPOSE", "link.txt"] if command == "grep" else ["link.txt"]
                    with self.subTest(command=command), self.assertRaises(ReaTermError):
                        execute_reaterm_tool({"command": command, "args": args}, root=root)
            self.assertEqual(target.read_text(encoding="utf-8"), "DO_NOT_EXPOSE\n")


if __name__ == "__main__":
    unittest.main()
