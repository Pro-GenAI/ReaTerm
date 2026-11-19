"""OpenAI Chat Completions and Responses function tool definitions.

The provider SDK is optional. Applications must execute model tool calls and
submit outputs explicitly; passing tools to a model never runs ReaTerm.
"""
from ._execution import execute_reaterm_tool
from ._schema import TOOL_DESCRIPTION, TOOL_NAME, input_schema

reaterm_tool_openai = {
    "type": "function",
    "function": {
        "name": TOOL_NAME,
        "description": TOOL_DESCRIPTION,
        "parameters": input_schema(),
        "strict": True,
    },
}

# The OpenAI Responses API has a flat, rather than nested, function schema.
reaterm_tool_openai_responses = {
    "type": "function",
    "name": TOOL_NAME,
    "description": TOOL_DESCRIPTION,
    "parameters": input_schema(),
    "strict": True,
}

reaterm = reaterm_tool_openai

__all__ = [
    "reaterm_tool_openai",
    "reaterm_tool_openai_responses",
    "reaterm",
    "execute_reaterm_tool",
]
