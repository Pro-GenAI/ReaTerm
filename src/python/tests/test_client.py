import os
from pathlib import Path
import unittest
from unittest.mock import patch
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from reaterm import ReaTerm, ReaTermError


class ClientTests(unittest.TestCase):
    def test_uses_argv_not_shell(self):
        with patch("reaterm.client._capture_capped") as run:
            run.return_value.returncode = 0
            run.return_value.stdout = "hello\n"
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                value = ReaTerm(binary="/opt/bin/reaterm", root=".").run("cat", "file;touch PWNED")
            self.assertEqual(value, "hello\n")
            argv = run.call_args.args[0]
            self.assertEqual(argv[-2:], ["cat", "file;touch PWNED"])

    def test_rejects_failed_native_operation(self):
        with patch("reaterm.client._capture_capped") as run:
            run.return_value.returncode = 2
            run.return_value.stderr = "reaterm: denied"
            with patch.dict(os.environ, {"REATERM_DECISION_MODEL": ""}):
                with self.assertRaises(ReaTermError):
                    ReaTerm(binary="reaterm").run("rm", "file")

    def test_model_veto_fail_closed(self):
        with patch.dict(os.environ, {"REATERM_DECISION_MODEL": "veto-model", "REATERM_OPENAI_API_KEY": ""}, clear=True):
            with self.assertRaises(ReaTermError):
                ReaTerm(binary="reaterm").run("ls")


if __name__ == "__main__":
    unittest.main()
