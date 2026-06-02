from openai_harmony import HarmonyEncodingName, ReasoningEffort, Role, load_harmony_encoding

from server.backend.commons import build_harmony_conversation
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

    assert (
        "<|start|>assistant<|channel|>commentary to=read <|constrain|>json"
        '<|message|>{"path":"changelog.md"}<|call|>'
        "<|start|>read<|channel|>commentary<|message|>contents<|end|>"
    ) in prompt

    default_conversation = build_harmony_conversation(ReasoningEffort.LOW, request.input)
    default_prompt = encoding.decode(
        encoding.render_conversation_for_completion(default_conversation, Role.ASSISTANT)
    )

    assert "<|start|>assistant<|channel|>commentary to=read" not in default_prompt
