"""Tests for the pip-level reaterm.run(command_line) convenience API."""
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import reaterm
from reaterm import ReaTermError, run


class ConvenienceTests(unittest.TestCase):
    def test_package_level_import_and_split(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch("reaterm.client._capture_capped") as execute:
                execute.return_value.returncode = 0
                execute.return_value.stdout = "listing\n"
                self.assertIs(run, reaterm.run)
                self.assertEqual(run("ls -al", root=".", binary="/bin/reaterm"), "listing\n")
                self.assertEqual(execute.call_args.args[0][-2:], ["ls", "-al"])

    def test_quoted_path_preserved_as_one_operand(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch("reaterm.client._capture_capped") as execute:
                execute.return_value.returncode = 0
                execute.return_value.stdout = "contents"
                self.assertEqual(run('cat "file name.txt"', binary="reaterm"), "contents")
                self.assertEqual(execute.call_args.args[0][-2:], ["cat", "file name.txt"])

    def test_invalid_syntax_rejected_before_launch(self):
        invalid = [
            "", " \t ", 'cat "unterminated', "ls;", "ls | cat",
            "cat $(touch BAD)", "ls > result", "ls\ncat other",
            "ls & cat", "ls \x60whoami\x60", "ls\\",
            '"" ls', None, 123,
        ]
        with patch("reaterm.client._capture_capped") as execute:
            for command_line in invalid:
                with self.subTest(command_line=command_line):
                    with self.assertRaises(ValueError):
                        run(command_line, binary="/bin/reaterm")
            execute.assert_not_called()

    def test_native_denial_propagates(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
            with patch("reaterm.client._capture_capped") as execute:
                execute.return_value.returncode = 2
                execute.return_value.stderr = "denied"
                with self.assertRaises(ReaTermError):
                    run("rm file.txt", binary="reaterm")


if __name__ == "__main__":
    unittest.main()
