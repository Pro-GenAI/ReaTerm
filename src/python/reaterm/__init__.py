"""ReaTerm Python bindings for the native read-only CLI."""

from __future__ import annotations

import shlex

from .client import ReaTerm, ReaTermError

__all__ = ["ReaTerm", "ReaTermError", "run"]


def run(
    command_line: str,
    *,
    root: str = ".",
    binary: str | None = None,
    timeout: float = 15,
    decision_model: str | None = None,
) -> str:
    """Execute a single read-only command string without invoking a shell.

    Shell operators and substitutions are intentionally unsupported. Use
    ReaTerm.run(command, *args) for exact literal argv (including unusual
    filenames) and to reuse a configured client.
    """
    if not isinstance(command_line, str) or not command_line.strip():
        raise ValueError("command_line must be a nonempty string")
    if any(char in command_line for char in "\x00\r\n;&|<>$\x60"):
        raise ValueError("shell operators and substitutions are not supported")
    try:
        args = shlex.split(command_line, posix=True)
    except ValueError as exc:
        raise ValueError(f"invalid command line: {exc}") from exc
    if not args or not args[0]:
        raise ValueError("command_line must start with a command")
    return ReaTerm(
        root=root, binary=binary, timeout=timeout, decision_model=decision_model
    ).run(*args)
