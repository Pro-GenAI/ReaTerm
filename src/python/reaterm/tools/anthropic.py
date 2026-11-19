"""Anthropic Messages tool definition for read-only native ReaTerm.

Importing this schema does not require the optional Anthropic Python SDK.
"""
from ._execution import execute_reaterm_tool
from ._schema import TOOL_DESCRIPTION, TOOL_NAME, input_schema

reaterm_tool_anthropic = {
    "name": TOOL_NAME,
    "description": TOOL_DESCRIPTION,
    "input_schema": input_schema(),
}

reaterm = reaterm_tool_anthropic

__all__ = ["reaterm_tool_anthropic", "reaterm", "execute_reaterm_tool"]
