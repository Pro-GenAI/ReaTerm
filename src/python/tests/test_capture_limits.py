"""Bounded native capture and shallow-install binary discovery regressions."""
import io
import subprocess
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from reaterm.client import _binary_path, _capture_capped, ReaTermError


class CaptureLimitTests(unittest.TestCase):
    def test_capture_uses_popen_without_shell(self):
        with patch("reaterm.client.subprocess.Popen") as spawn:
            child = spawn.return_value
            child.stdout = io.BytesIO(b"safe\n")
            child.stderr = io.BytesIO(b"")
            child.wait.return_value = 0
            result = _capture_capped(["/trusted/reaterm", "pwd"], timeout=3)
            self.assertEqual(result.stdout, "safe\n")
            spawn.assert_called_once_with(
                ["/trusted/reaterm", "pwd"],
                shell=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

    def test_binary_stdout_is_decoded_with_replacement(self):
        child = _capture_capped([
            sys.executable, "-c",
            "import sys; sys.stdout.buffer.write(bytes([255, 0, 10]))"
        ], timeout=5)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(child.stdout, "\ufffd\x00\n")

    def test_stdout_stderr_combined_capture_is_limited(self):
        with self.assertRaisesRegex(ReaTermError, "2 MiB"):
            _capture_capped([
                sys.executable, "-c",
                "import sys; sys.stdout.buffer.write(b'x' * (2*1024*1024)); "
                "sys.stderr.buffer.write(b'y' * 65536)"
            ], timeout=10)

    def test_shallow_install_falls_back_to_path(self):
        with patch("reaterm.client.__file__", "/app/reaterm/client.py"), \
             patch("reaterm.client.shutil.which", return_value="/opt/reat/greaterm"):
            with patch.dict("os.environ", {"REATERM_BIN": ""}):
                self.assertEqual(_binary_path(None), "/opt/reat/greaterm")


if __name__ == "__main__":
    unittest.main()
