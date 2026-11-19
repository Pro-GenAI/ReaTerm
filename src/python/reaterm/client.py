"""Python adapter: argv-only subprocess execution and optional model veto.

Never invoke a shell or use the model as an authorization source. The Rust
binary enforces the deterministic allowlist and filesystem sandbox.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
from typing import Optional


class ReaTermError(RuntimeError):
    """An execution failure or denied decision."""


def _binary_path(explicit: Optional[str]) -> str:
    if explicit:
        return explicit
    configured = os.getenv("REATERM_BIN")
    if configured:
        return configured
    # Editable/source checkout convenience: project/src/python/reaterm/client.py
    repo = Path(__file__).resolve().parents[3]
    for profile in ("release", "debug"):
        native_name = "reaterm.exe" if os.name == "nt" else "reaterm"
        candidate = repo / "src" / "rust" / "target" / profile / native_name
        if candidate.is_file():
            return str(candidate)
    installed = shutil.which("reaterm")
    if installed:
        return installed
    raise ReaTermError(
        "ReaTerm binary not found: cargo build --manifest-path src/rust/Cargo.toml "
        "--release, then set REATERM_BIN or install it on PATH"
    )


class ReaTerm:
    """Execute native ReaTerm operations with an optional LLM veto.

    Commands are passed as individual arguments, e.g. run("cat", "README.md").
    No shlex or shell expansion is performed.
    """

    def __init__(
        self,
        *,
        root: str = ".",
        binary: Optional[str] = None,
        timeout: float = 15,
        decision_model: Optional[str] = None,
    ) -> None:
        self.root = str(Path(root).resolve())
        self.binary = binary
        self.timeout = timeout
        self.decision_model = decision_model or os.getenv("REATERM_DECISION_MODEL")

    def _model_allows(self, args: tuple[str, ...]) -> None:
        if not self.decision_model:
            return
        try:
            from openai import OpenAI  # official SDK; optional dependency
        except ImportError as exc:
            raise ReaTermError(
                "Model veto enabled; install official OpenAI SDK: pip install 'reaterm[decision]'"
            ) from exc
        key = os.getenv("REATERM_OPENAI_API_KEY") or os.getenv("OPENAI_API_KEY")
        base_url = os.getenv("REATERM_OPENAI_BASE_URL") or os.getenv("OPENAI_BASE_URL")
        if not key:
            raise ReaTermError("REATERM_OPENAI_API_KEY or OPENAI_API_KEY is required")
        kwargs: dict[str, str] = {"api_key": key}
        if base_url:
            kwargs["base_url"] = base_url
        try:
            client = OpenAI(**kwargs)
            result = client.chat.completions.create(
                model=self.decision_model,
                messages=[
                    {
                        "role": "system",
                        "content": (
                            "You are a conservative read-only command veto checker. "
                            "Respond with exactly ALLOW or DENY, no other text. "
                            "ALLOW only if the specified argv is obviously a non-mutating "
                            "inspection operation. Never authorize code execution, shell "
                            "syntax, filesystem writes, or dangerous/unrecognized flags. "
                            "Your answer is advisory: the native executor independently "
                            "enforces its stricter allowlist."
                        ),
                    },
                    {
                        "role": "user",
                        "content": f"Command argv (repr, no shell): {args!r}",
                    },
                ],
            )
            response = result.choices[0].message.content or ""
        except Exception as exc:
            raise ReaTermError(f"model veto unavailable: {exc}") from exc
        if response.strip() != "ALLOW":
            raise ReaTermError("command denied by optional decision model")

    def run(self, command: str, *args: str) -> str:
        """Run an approved command and return stdout; raise on any failure."""
        if not command or not all(isinstance(x, str) for x in (command, *args)):
            raise ValueError("command and each argument must be strings")
        tokens = (command, *args)
        self._model_allows(tokens)
        argv = [_binary_path(self.binary), "--root", self.root, *tokens]
        try:
            completed = subprocess.run(
                argv,
                shell=False,
                capture_output=True,
                text=True,
                timeout=self.timeout,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise ReaTermError(f"ReaTerm execution failed: {exc}") from exc
        if completed.returncode:
            raise ReaTermError(completed.stderr.strip() or f"reaterm exited {completed.returncode}")
        return completed.stdout
