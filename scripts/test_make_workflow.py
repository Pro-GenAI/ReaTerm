"""Contract tests for root GNU Make setup/test/verify orchestration.

Each language is replaced with a disposable fake Makefile; no real toolchain,
network, or installation is invoked. The test specifically verifies that a
failed setup does not run that language's tests, and a failed test prevents
setup of any subsequent language, including with make -j.
"""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
LANGUAGES = ("rust", "python", "typescript", "website")
STEPS = [f"{language}.{target}" for language in LANGUAGES for target in ("setup", "test")]


@unittest.skipUnless(shutil.which("make"), "requires GNU make")
class RootMakeWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="reaterm-make-")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.log = self.base / "steps.log"

        for language in LANGUAGES:
            directory = self.base / language
            directory.mkdir()
            content = (
                ".PHONY: setup test verify\n"
                "setup:\n"
                f"\t@printf '%s\\n' '{language}.setup' >> \"$$TEST_LOG\"\n"
                f"\t@if [ \"$$FAIL_AT\" = '{language}.setup' ]; then exit 31; fi\n"
                "test:\n"
                f"\t@printf '%s\\n' '{language}.test' >> \"$$TEST_LOG\"\n"
                f"\t@if [ \"$$FAIL_AT\" = '{language}.test' ]; then exit 32; fi\n"
                "verify:\n"
                "\t@$(MAKE) --no-print-directory setup\n"
                "\t@$(MAKE) --no-print-directory test\n"
            )
            (directory / "Makefile").write_text(content, encoding="utf-8")

    def run_root(self, target, fail_at="", parallel=False):
        env = os.environ.copy()
        env["TEST_LOG"] = str(self.log)
        env["FAIL_AT"] = fail_at
        args = ["make", "--no-print-directory", "-f", str(ROOT / "Makefile")]
        if parallel:
            args.append("-j8")
        args.extend(
            [
                target,
                f"RUST_DIR={self.base / 'rust'}",
                f"PYTHON_DIR={self.base / 'python'}",
                f"TYPESCRIPT_DIR={self.base / 'typescript'}",
                f"WEBSITE_DIR={self.base / 'website'}",
            ]
        )
        process = subprocess.run(args, cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
        observed = self.log.read_text(encoding="utf-8").splitlines() if self.log.exists() else []
        return process, observed

    def test_verify_is_sequential_and_complete(self):
        process, observed = self.run_root("verify")
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(observed, STEPS)

    def test_python_setup_rejects_old_interpreter_before_creating_venv(self):
        python = self.base / "old-python"
        python.write_text(
            "#!/bin/sh\nexec " + shutil.which("python3")
            + " -c 'import sys; sys.version_info = (3, 9); exec(sys.argv[1])' \"$2\"\n",
            encoding="utf-8",
        )
        python.chmod(0o755)
        venv = self.base / "venv"
        process = subprocess.run(
            ["make", "-C", str(ROOT / "src/python"), "setup", f"PYTHON={python}", f"VENV_DIR={venv}"],
            capture_output=True, text=True, timeout=30,
        )
        self.assertNotEqual(process.returncode, 0)
        self.assertIn("requires Python 3.11 or newer", process.stderr)
        self.assertFalse(venv.exists())

    def test_verify_stays_sequential_even_with_make_parallel_flag(self):
        process, observed = self.run_root("verify", parallel=True)
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(observed, STEPS)

    def test_each_failed_setup_stops_before_tests_and_next_language(self):
        for language in LANGUAGES:
            with self.subTest(language=language):
                self.log.unlink(missing_ok=True)
                failed = f"{language}.setup"
                process, observed = self.run_root("verify", fail_at=failed)
                self.assertNotEqual(process.returncode, 0)
                self.assertEqual(observed, STEPS[: STEPS.index(failed) + 1])
                self.assertNotIn(f"{language}.test", observed)

    def test_each_failed_test_stops_before_next_language(self):
        for language in LANGUAGES:
            with self.subTest(language=language):
                self.log.unlink(missing_ok=True)
                failed = f"{language}.test"
                process, observed = self.run_root("verify", fail_at=failed)
                self.assertNotEqual(process.returncode, 0)
                self.assertEqual(observed, STEPS[: STEPS.index(failed) + 1])

    def test_rust_setup_failure_prevents_every_test(self):
        process, observed = self.run_root("verify", fail_at="rust.setup", parallel=True)
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual(observed, ["rust.setup"])

    def test_rust_test_failure_does_not_reach_python(self):
        process, observed = self.run_root("verify", fail_at="rust.test", parallel=True)
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual(observed, ["rust.setup", "rust.test"])

    def test_setup_and_test_are_individually_available(self):
        process, observed = self.run_root("setup")
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(observed, [f"{lang}.setup" for lang in LANGUAGES])
        self.log.unlink()
        process, observed = self.run_root("test")
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(observed, [f"{lang}.test" for lang in LANGUAGES])

    def test_setup_stops_at_failed_install(self):
        process, observed = self.run_root("setup", fail_at="python.setup")
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual(observed, ["rust.setup", "python.setup"])

    def test_test_stops_at_failed_suite(self):
        process, observed = self.run_root("test", fail_at="python.test")
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual(observed, ["rust.test", "python.test"])


if __name__ == "__main__":
    unittest.main()
