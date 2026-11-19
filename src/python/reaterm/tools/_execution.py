"""Execute model tool-call arguments through the native ReaTerm read-only CLI.

Tool definitions alone do not execute anything or contact a model.
The application, not a model response, selects the filesystem root.
"""
from __future__ import annotations

import json
from collections.abc import Mapping

from reaterm.client import ReaTerm

from ._schema import READ_COMMANDS


def execute_reaterm_tool(
    arguments: Mapping[str, object] | str,
    *,
    root: str = ".",
    binary: str | None = None,
    timeout: float = 15,
    decision_model: str | None = None,
) -> str:
    """Run OpenAI JSON-string or Anthropic mapping tool arguments.

    Accepts exactly: command (allowlisted native name), args (string list).
    The strings remain argv tokens, never shell input. The Rust CLI checks
    allowed flags, safe paths and read-only system enforcement independently.

    The returned stdout can be sent to the provider using its tool-call ID.
    """
    if isinstance(arguments, str):
        try:
            arguments = json.loads(arguments)
        except json.JSONDecodeError as exc:
            raise ValueError("ReaTerm tool arguments must be valid JSON") from exc
    if not isinstance(arguments, Mapping) or set(arguments) != {"command", "args"}:
        raise ValueError("ReaTerm tool arguments require only 'command' and 'args'")

    command = arguments["command"]
    args = arguments["args"]
    if not isinstance(command, str) or command not in READ_COMMANDS:
        raise ValueError("ReaTerm tool command must be a supported read-only command")
    if not isinstance(args, list) or not all(isinstance(arg, str) for arg in args):
        raise ValueError("ReaTerm tool 'args' must be a list of strings")

    return ReaTerm(
        root=root, binary=binary, timeout=timeout, decision_model=decision_model
    ).run(command, *args)
