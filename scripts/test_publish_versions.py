"""Exercise the version helper in a disposable copy of the release files."""
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FILES = (
    "temp/update-versions.sh",
    "src/python/pyproject.toml",
    "src/rust/Cargo.toml",
    "src/rust/Cargo.lock",
    "src/typescript/package.json",
    "src/typescript/package-lock.json",
)


class VersionUpdateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in FILES:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / name, target)

    def run_update(self, *args, input=""):
        return subprocess.run(["bash", str(self.root / FILES[0]), *args], input=input,
                              text=True, capture_output=True)

    def test_updates_all_versions_and_preserves_dependencies(self):
        before = json.loads((self.root / FILES[5]).read_text())
        result = self.run_update("12.34.56")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('version = "12.34.56"', (self.root / FILES[1]).read_text())
        self.assertIn('version = "12.34.56"', (self.root / FILES[2]).read_text())
        self.assertIn('name = "reaterm"\nversion = "12.34.56"', (self.root / FILES[3]).read_text())
        self.assertEqual(json.loads((self.root / FILES[4]).read_text())["version"], "12.34.56")
        after = json.loads((self.root / FILES[5]).read_text())
        self.assertEqual(after["version"], "12.34.56")
        self.assertEqual(after["packages"][""]["version"], "12.34.56")
        self.assertEqual({k: v for k, v in before["packages"].items() if k},
                         {k: v for k, v in after["packages"].items() if k})

    def test_rejection_does_not_change_files(self):
        before = [(self.root / p).read_bytes() for p in FILES[1:]]
        for args in (("invalid",), ("0.0.0",), ("1.0.0", "2.0.0")):
            self.assertNotEqual(self.run_update(*args).returncode, 0)
            self.assertEqual(before, [(self.root / p).read_bytes() for p in FILES[1:]])

    def test_patch_increment_and_confirmation(self):
        self.assertEqual(self.run_update("1.2.3").returncode, 0)
        self.assertEqual(self.run_update().returncode, 0)
        self.assertEqual(json.loads((self.root / FILES[4]).read_text())["version"], "1.2.4")
        self.assertEqual(self.run_update("1.2.3", input="y\n").returncode, 0)


if __name__ == "__main__":
    unittest.main()
