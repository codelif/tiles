from openai_harmony import HarmonyEncodingName, ReasoningEffort, Role, load_harmony_encoding

from server.backend.commons import build_harmony_conversation, normalize_harmony_tool_name
from server.schemas import ResponsesRequest


def test_harmony_conversation_replays_function_call_before_output():
    request = ResponsesRequest.model_validate(
        {
            "model": "unsloth/gpt-oss-20b-GGUF",
            "input": [
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read changelog.md"}],
                },
                {
                    "type": "function_call",
                    "id": "toolcall_1",
                    "call_id": "call_1",
                    "name": "read",
                    "arguments": '{"path":"changelog.md"}',
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_1",
                    "output": "contents",
                },
            ],
        }
    )

    conversation = build_harmony_conversation(
        ReasoningEffort.LOW, request.input, replay_function_calls=True
    )
    encoding = load_harmony_encoding(HarmonyEncodingName.HARMONY_GPT_OSS)
    prompt = encoding.decode(
        encoding.render_conversation_for_completion(conversation, Role.ASSISTANT)
    )

    assert "<|start|>assistant to=read<|channel|>commentary json" in prompt
    assert '<|message|>{"path":"changelog.md"}<|call|>' in prompt
    assert (
        "<|start|>read<|channel|>commentary<|message|>contents<|end|>"
        in prompt
    )

    default_conversation = build_harmony_conversation(ReasoningEffort.LOW, request.input)
    default_prompt = encoding.decode(
        encoding.render_conversation_for_completion(default_conversation, Role.ASSISTANT)
    )

    assert "<|start|>assistant to=read<|channel|>commentary" not in default_prompt


def test_harmony_conversation_includes_function_tools():
    request = ResponsesRequest.model_validate(
        {
            "model": "unsloth/gpt-oss-20b-GGUF",
            "input": [
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "read changelog.md"}],
                }
            ],
            "tools": [
                {
                    "type": "function",
                    "name": "read",
                    "description": "Read a file",
                    "parameters": {
                        "type": "object",
                        "required": ["path"],
                        "properties": {"path": {"type": "string"}},
                    },
                }
            ],
        }
    )

    conversation = build_harmony_conversation(
        ReasoningEffort.LOW,
        request.input,
        replay_function_calls=True,
        tools=request.tools,
    )
    encoding = load_harmony_encoding(HarmonyEncodingName.HARMONY_GPT_OSS)
    prompt = encoding.decode(
        encoding.render_conversation_for_completion(conversation, Role.ASSISTANT)
    )

    assert "namespace functions" in prompt
    assert "type read" in prompt
    assert "path: string" in prompt


def test_normalize_harmony_tool_name_only_strips_known_function_namespace():
    request = ResponsesRequest.model_validate(
        {
            "model": "unsloth/gpt-oss-20b-GGUF",
            "input": "hello",
            "tools": [
                {
                    "type": "function",
                    "name": "read",
                    "parameters": {"type": "object", "properties": {}},
                }
            ],
        }
    )

    assert normalize_harmony_tool_name("functions.read", request.tools) == "read"
    assert normalize_harmony_tool_name("read", request.tools) == "read"
    assert (
        normalize_harmony_tool_name("functions.unknown", request.tools)
        == "functions.unknown"
    )
