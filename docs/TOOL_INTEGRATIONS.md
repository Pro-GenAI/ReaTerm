# ReaTerm AI tool integrations (Python and MCP)

ReaTerm provides JSON schemas that can be passed to the official OpenAI
and Anthropic Python SDKs. A model can request a read-only inspection
operation, but passing a tool schema does not execute it. Your application
must validate and execute each tool call, and submit the result back to
the provider. The shared executor uses the native Rust binary.

A tool request has the same shape with both providers:

~~~json
{"command": "ls", "args": ["-al"]}
~~~

The application, not the model, selects the root directory (default ".").
Install the Rust CLI separately or set REATERM_BIN. The provider SDKs are
optional dependencies, not needed merely to import schemas.

## OpenAI Chat Completions

Install the optional OpenAI SDK: pip install openai

Use the actual Python SDK method client.chat.completions.create, not
completion.create. This example shows one round trip:

~~~python
from openai import OpenAI
from reaterm.tools.openai import reaterm_tool_openai, execute_reaterm_tool

client = OpenAI()
messages = [{"role": "user", "content": "List files in the project root."}]
first = client.chat.completions.create(
    model="YOUR_OPENAI_MODEL",
    messages=messages,
    tools=[reaterm_tool_openai],
)
message = first.choices[0].message
messages.append(message.model_dump(exclude_none=True))
for call in message.tool_calls or []:
    if call.type != "function" or call.function.name != "reaterm":
        continue
    # arguments is a JSON string produced by the model.
    try:
        result = execute_reaterm_tool(call.function.arguments, root=".")
    except (ValueError, RuntimeError) as exc:
        result = f"ReaTerm error: {exc}"
    messages.append({"role": "tool", "tool_call_id": call.id, "content": result})

if message.tool_calls:
    follow_up = client.chat.completions.create(
        model="YOUR_OPENAI_MODEL", messages=messages, tools=[reaterm_tool_openai]
    )
    print(follow_up.choices[0].message.content)
else:
    print(message.content)
~~~

For the OpenAI Responses API, import reaterm_tool_openai_responses
from reaterm.tools.openai. Use it in client.responses.create(..., tools=[...]).
Responses function-call output uses the function_call_output item with the
original call_id; its arguments are also a JSON string.

## Anthropic Messages

Install the optional Anthropic SDK: pip install anthropic

Use the actual method client.messages.create. This example shows one
round trip:

~~~python
from anthropic import Anthropic
from reaterm.tools.anthropic import reaterm_tool_anthropic, execute_reaterm_tool

client = Anthropic()
messages = [{"role": "user", "content": "List files in the project root."}]
first = client.messages.create(
    model="YOUR_ANTHROPIC_MODEL",
    max_tokens=512,
    messages=messages,
    tools=[reaterm_tool_anthropic],
)
tool_uses = [block for block in first.content if block.type == "tool_use"]
if tool_uses:
    messages.append({
        "role": "assistant",
        "content": [block.model_dump(exclude_none=True) for block in first.content],
    })
    results = []
    for block in tool_uses:
        if block.name != "reaterm":
            continue
        # Anthropic block.input is a dict, not a JSON string.
        try:
            result = execute_reaterm_tool(block.input, root=".")
        except (ValueError, RuntimeError) as exc:
            result = f"ReaTerm error: {exc}"
        results.append({
            "type": "tool_result",
            "tool_use_id": block.id,
            "content": result,
        })
    messages.append({"role": "user", "content": results})
    follow_up = client.messages.create(
        model="YOUR_ANTHROPIC_MODEL",
        max_tokens=512,
        messages=messages,
        tools=[reaterm_tool_anthropic],
    )
    print(follow_up.content)
else:
    print(first.content)
~~~

For production use, implement a loop because either model can request
further tool calls, and return errors using the provider's expected
tool-result format.

## Security and data boundaries

- Only pwd, ls, cat, head, tail, wc, grep, and stat are eligible. Native
  Rust enforces the full command and flag allowlist, capability-relative
  paths, OS write restrictions and resource limits independently.
- The tool input uses separate argv strings. The tool does not invoke
  a shell, perform shell expansions, or execute an arbitrary program.
- The model cannot set root, executable path or decision-model settings.
  Choose a minimal readable directory in trusted application code.
- Running a tool does not automatically send data to the provider, but
  including its stdout in a tool-result message does. Do not expose
  confidential files in the selected root unless their disclosure is
  intended. ReaTerm itself is not an authentication boundary.
- A configured optional ReaTerm decision model may veto an operation.
  This is separate from the OpenAI/Anthropic tools shown above and
  cannot authorize an otherwise forbidden command.


## Model Context Protocol (MCP) stdio server

The Python package also provides a local MCP server with one tool,
`run_command`. It delegates to the **same** `execute_reaterm_tool` and
native Rust executable used by the OpenAI/Anthropic adapters. No shell,
HTTP listener or arbitrary subprocess command is introduced.

Install the optional official MCP Python SDK v2 and the Python wrapper,
then install/build the native Rust CLI separately:

~~~sh
python -m pip install 'reaterm[mcp]'
# In a source checkout, instead use:
# python -m pip install -e './src/python[mcp]'
~~~

Configure a compatible MCP host to launch the server over stdio. For clients
using the `mcpServers` configuration format, an example is:

~~~json
{
  "mcpServers": {
    "reaterm": {
      "command": "reaterm-mcp",
      "args": [
        "--root", "/absolute/path/to/allowed-project",
        "--binary", "/absolute/path/to/reaterm"
      ]
    }
  }
}
~~~

`--root` is mandatory; the directory must exist. `--binary` is optional
when the native executable can be discovered using `REATERM_BIN` or PATH.
The MCP host, **not** the model, configures both paths. The server has no
network transport and writes protocol messages only to stdout.

The exposed tool accepts the following arguments:

~~~json
{"command": "grep", "args": ["-n", "TODO", "README.md"]}
~~~

The tool name is `run_command`; `command` is limited to `pwd`, `ls`,
`cat`, `head`, `tail`, `wc`, `grep`, and `stat`. Each argument is an
individual literal argv token, never interpreted by a shell. The native
Rust executor validates flags and paths, enforces resource bounds, and
fails closed if OS protections are unavailable. Native denial is returned
as a tool error by the MCP SDK.

**Confidentiality warning:** clients can read files inside the operator's
selected root and may send returned content to an external model. Only
expose a minimal directory without credentials or other data that the
client is not permitted to see. Optional ReaTerm decision-model vetos
remain controlled by the launching environment.
