from server.backend.llama_server.openresponses_adapter import (
    openresponses_input_to_messages,
    openresponses_tools_to_chat_tools,
)


def test_openresponses_input_maps_tool_roundtrip():
    messages = openresponses_input_to_messages(
        [
            {
                "type": "message",
                "role": "developer",
                "content": [{"type": "input_text", "text": "system rules"}],
            },
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "read foo.csv"}],
            },
            {
                "type": "function_call",
                "id": "fc_1",
                "call_id": "call_abc",
                "name": "read",
                "arguments": '{"path":"foo.csv"}',
            },
            {
                "type": "function_call_output",
                "call_id": "call_abc",
                "output": "col\n1\n",
            },
        ]
    )

    assert messages[0] == {"role": "system", "content": "system rules"}
    assert messages[1] == {"role": "user", "content": "read foo.csv"}
    assert messages[2]["role"] == "assistant"
    assert messages[2]["tool_calls"][0]["function"]["name"] == "read"
    assert messages[3] == {
        "role": "tool",
        "tool_call_id": "call_abc",
        "content": "col\n1\n",
    }


def test_openresponses_tools_map_to_chat_tools():
    tools = openresponses_tools_to_chat_tools(
        [
            {
                "type": "function",
                "name": "bash",
                "description": "run shell",
                "parameters": {
                    "type": "object",
                    "properties": {"command": {"type": "string"}},
                    "required": ["command"],
                },
            }
        ]
    )
    assert tools is not None
    assert tools[0]["function"]["name"] == "bash"
