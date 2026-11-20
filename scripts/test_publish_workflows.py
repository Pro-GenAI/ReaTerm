"""Offline publication safety checks; never contacts PyPI or the npm registry.

Run: python3 -m unittest discover -s scripts -p 'test_publish*.py' -v
Mocked npm tests use disposable repositories and a fake npm executable.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
NPM_SCRIPT = ROOT / "src/typescript/scripts/npm_publish.sh"
PYPI_WORKFLOW = ROOT / ".github/workflows/publish-pypi.yml"


class PyPIConfigurationTests(unittest.TestCase):
    def test_release_workflow_matches_real_package_and_requires_tests(self):
        text = PYPI_WORKFLOW.read_text(encoding="utf-8")
        for expected in (
            "release:",
            "types: [published]",
            "https://pypi.org/project/reaterm/",
            "src/python/pyproject.toml",
            'py["name"] != "reaterm"',
            'npm["name"] != "reaterm"',
            "src/python/dist/*",
            "make verify",
            "python -m build --wheel --sdist",
            "python -m twine check dist/*",
            "REATERM_BIN",
            "unittest discover",
            "actions/upload-artifact@v4",
            "actions/download-artifact@v4",
            "needs: build-and-test",
            "id-token: write",
            "pypa/gh-action-pypi-publish@v1.13.0",
        ):
            with self.subTest(required=expected):
                self.assertIn(expected, text)
        for stale in ("agent-rt", "[api,cli]", "pytest-asyncio"):
            self.assertNotIn(stale, text)

    def test_all_package_names_versions_are_aligned(self):
        import re
        def toml_string(path, field):
            content = (ROOT / path).read_text(encoding="utf-8")
            match = re.search(r"(?m)^" + re.escape(field) + r'\s*=\s*"([^"]+)"\s*$', content)
            if not match:
                self.fail(f"Missing {field} in {path}")
            return match.group(1)
        npm = json.loads((ROOT / "src/typescript/package.json").read_text(encoding="utf-8"))
        self.assertEqual(
            (toml_string("src/python/pyproject.toml", "name"), toml_string("src/rust/Cargo.toml", "name"), npm["name"]),
            ("reaterm", "reaterm", "reaterm"),
        )
        self.assertEqual(toml_string("src/python/pyproject.toml", "version"), toml_string("src/rust/Cargo.toml", "version"))
        self.assertEqual(toml_string("src/python/pyproject.toml", "version"), npm["version"])
        self.assertEqual(npm["publishConfig"]["access"], "public")
        self.assertEqual(toml_string("src/python/pyproject.toml", "requires-python"), ">=3.11")


@unittest.skipUnless(shutil.which("bash") and shutil.which("node"), "bash and node required")
class MockedNpmPublisherTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="reaterm-publish-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        package = self.root / "src" / "typescript"
        scripts = package / "scripts"
        scripts.mkdir(parents=True)
        self.script = scripts / "npm_publish.sh"
        shutil.copyfile(NPM_SCRIPT, self.script)
        shutil.copyfile(ROOT / "src/typescript/package.json", package / "package.json")
        shutil.copyfile(ROOT / "src/typescript/README.md", package / "README.md")

        rust = self.root / "src/rust"
        (rust / "target/release").mkdir(parents=True)
        (rust / "Cargo.toml").write_text('[package]\nname = "reaterm"\n', encoding="utf-8")
        self.binary = rust / "target/release/reaterm"
        self.binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        self.binary.chmod(0o700)

        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.log = self.root / "npm.log"
        npm_mock = self.bin_dir / "npm"
        npm_mock.write_text(
            """#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
args = sys.argv[1:]
with Path(os.environ["MOCK_NPM_LOG"]).open("a", encoding="utf-8") as output:
    output.write(" ".join(args) + "\\n")
if os.environ.get("NPM_TOKEN"):
    sys.exit(49)  # build/install/test must not receive publishing tokens
if args[:1] == [os.environ.get("MOCK_FAIL_NPM")]:
    sys.exit(43)
if args[:1] == ["pack"]:
    package = json.loads(Path("package.json").read_text(encoding="utf-8"))
    data = [{"name": package["name"], "version": package["version"], "files": [
        {"path": x} for x in ["dist/src/index.js", "dist/src/index.d.ts",
                              "dist/src/tools.js", "dist/src/tools.d.ts",
                              "dist/src/mcp-server.js", "dist/src/mcp-server.d.ts",
                              "README.md", "package.json"]]}]
    if os.environ.get("MOCK_BAD_PACK") == "1":
        data[0]["files"].pop(0)
    print(json.dumps(data))
if args[:1] == ["whoami"]:
    config = os.environ.get("NPM_CONFIG_USERCONFIG", "")
    if config:
        with Path(os.environ["MOCK_CONFIG_LOG"]).open("w", encoding="utf-8") as output:
            output.write(config)
    print("mock-user")
""",
            encoding="utf-8",
        )
        npm_mock.chmod(0o700)
        self.config_log = self.root / "auth-config-path"

    def invoke(self, *args, extra_env=None):
        env = os.environ.copy()
        for name in ("BASH_ENV", "NPM_CONFIG_USERCONFIG", "NPM_TOKEN", "NPM_REGISTRY", "MOCK_FAIL_NPM", "MOCK_BAD_PACK", "PACKAGE_MANAGER"):
            env.pop(name, None)
        env.update({
            "PATH": str(self.bin_dir) + os.pathsep + os.environ["PATH"],
            "REATERM_BIN": str(self.binary),
            "MOCK_NPM_LOG": str(self.log),
            "MOCK_CONFIG_LOG": str(self.config_log),
            "NPM_TOKEN": "fake-token-never-publish",
        })
        if extra_env:
            env.update(extra_env)
        return subprocess.run(
            ["bash", str(self.script), *args], cwd=self.root, env=env,
            text=True, capture_output=True, timeout=30,
        )

    def logged(self):
        return self.log.read_text(encoding="utf-8").splitlines() if self.log.exists() else []

    def test_dry_run_skips_auth(self):
        result = self.invoke("--dry-run")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(self.logged()), 3, self.logged())
        self.assertTrue(self.logged()[0].startswith("install --no-package-lock"))
        self.assertEqual(self.logged()[1], "run check")
        self.assertTrue(self.logged()[2].startswith("pack --dry-run --json"))
        self.assertFalse(self.config_log.exists())
        self.assertFalse((self.root / "src/typescript/.npmrc.publish").exists())

    def test_inherited_bash_env_does_not_run_in_publisher(self):
        bash_env = self.root / "bash_env"
        bash_env.write_text("npm --version\n", encoding="utf-8")
        with patch.dict(os.environ, {"BASH_ENV": str(bash_env)}):
            result = self.invoke("--help")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.logged(), [])

    def test_help_and_bad_flags_never_install(self):
        self.assertEqual(self.invoke("--help").returncode, 0)
        for argv in [
            ("--publish", "--dry-run"),
            ("--access", "admin"),
            ("--access", "restricted"),
            ("--tag", "--publish"),
            ("--whatever",),
            ("--help", "--publish"),
        ]:
            with self.subTest(args=argv):
                result = self.invoke(*argv)
                self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.logged(), [])

    def test_install_failure_stops_before_build_tests_and_publish(self):
        result = self.invoke("--publish", extra_env={"MOCK_FAIL_NPM": "install"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.logged()), 1)
        self.assertTrue(self.logged()[0].startswith("install "))
        self.assertFalse(self.config_log.exists())

    def test_invalid_tarball_stops_before_publish(self):
        result = self.invoke("--publish", extra_env={"MOCK_BAD_PACK": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.logged()), 3)
        self.assertNotIn("whoami", " ".join(self.logged()))
        self.assertFalse(self.config_log.exists())

    def test_default_publish_uses_private_temp_auth(self):
        result = self.invoke("--tag", "next")
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.logged()
        self.assertEqual(len(commands), 5, commands)
        self.assertEqual(commands[3], "whoami --registry https://registry.npmjs.org/")
        self.assertEqual(commands[4], "publish --registry https://registry.npmjs.org/ --tag next --access public")
        self.assertTrue(self.config_log.exists())
        path = Path(self.config_log.read_text(encoding="utf-8"))
        self.assertFalse(path.exists(), "temporary token config was not removed")
        self.assertNotIn("fake-token-never-publish", result.stdout + result.stderr)

    def test_missing_native_binary_fails_before_install(self):
        self.binary.unlink()
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.logged(), [])

    def test_custom_http_registry_is_denied_for_real_publish(self):
        result = self.invoke("--publish", extra_env={"NPM_REGISTRY": "http://example.test"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.logged(), [])


if __name__ == "__main__":
    unittest.main()
