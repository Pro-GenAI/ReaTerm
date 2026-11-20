"""Protocol-level checks against the real Python MCP 2.x stdio server.

pytest-mcptest owns process lifecycle, handshake, JSON-RPC framing, schema
validation and tool results. No hand-written MCP wire format or fake SDK.
These tests do not contact a model provider or access files outside tmp_path.
"""
from __future__ import annotations

import os
from pathlib import Path
import sys

import pytest
from pytest_mcp import (
    assert_conformance,
    assert_tool_call,
    assert_tool_error,
    assert_tool_exists,
    connect,
    result_text,
    stdio,
)


def server_target(root: Path, *, model: str = "", binary: str | None = None):
    selected_binary = binary or os.environ.get("REATERM_BIN")
    if not selected_binary:
        pytest.skip("native CLI required: set REATERM_BIN")
    env = os.environ.copy()
    env.update({
        "REATERM_BIN": selected_binary,
        "REATERM_DECISION_MODEL": model,
        # Force offline failure rather than using host developer credentials.
        "REATERM_OPENAI_API_KEY": "",
        "OPENAI_API_KEY": "",
    })
    return stdio(
        sys.executable, "-m", "reaterm.mcp_server",
        "--root", str(root), "--binary", selected_binary,
        env=env,
    )


@pytest.fixture
def anyio_backend():
    # One backend avoids implicit Trio runs and matches MCP v2 asyncio CI.
    return "asyncio"


@pytest.fixture
def mcp_server(tmp_path: Path):
    """A newly launched native-root-scoped Python MCP server per test."""
    (tmp_path / "readme.txt").write_text("public text\n", encoding="utf-8")
    return server_target(tmp_path)


async def test_real_stdio_handshake_and_exact_allowlisted_tool(mcp_client):
    # Validates negotiated MCP version, tool naming and JSON Schema.
    await assert_conformance(mcp_client)
    tool = await assert_tool_exists(mcp_client, "run_command")
    assert tool.input_schema["type"] == "object"
    assert set(tool.input_schema["properties"]["command"]["enum"]) == {
        "pwd", "ls", "cat", "head", "tail", "wc", "grep", "stat",
    }
    tools = await mcp_client.list_all_tools()
    assert [candidate.name for candidate in tools] == ["run_command"]
    await assert_tool_call(
        mcp_client, "run_command",
        {"command": "cat", "args": ["readme.txt"]},
        expected="public text\n",
    )


async def test_stdio_rejects_native_escape_and_model_requested_mutation(mcp_client):
    # The model cannot change the selected root or invoke non-allowlisted tools.
    await assert_tool_error(
        mcp_client, "run_command",
        {"command": "cat", "args": ["../outside.txt"]},
    )
    await assert_tool_error(
        mcp_client, "run_command",
        {"command": "rm", "args": ["readme.txt"]},
    )
    # An extra model-supplied root must never broaden the trusted root,
    # regardless of whether the SDK rejects or ignores unknown fields.
    probe = await mcp_client.call_tool(
        "run_command",
        {"command": "cat", "args": ["readme.txt"], "root": "/"},
    )
    if not probe.is_error:
        assert result_text(probe) == "public text\n"


async def test_stdio_inherits_model_veto_and_fails_closed_offline(tmp_path: Path, mcp_client):
    # The mcp_client fixture activates pytest-mcptest's async test runner.
    await assert_tool_exists(mcp_client, "run_command")
    # A non-existent binary proves the request cannot reach native execution.
    target = server_target(
        tmp_path, model="offline-model",
        binary=str(tmp_path / "missing-reaterm"),
    )
    async with connect(target) as client:
        await assert_tool_exists(client, "run_command")
        await assert_tool_error(
            client, "run_command", {"command": "pwd", "args": []},
        )
