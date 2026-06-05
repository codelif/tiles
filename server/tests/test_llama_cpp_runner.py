import pytest
from openai_harmony import (
    Conversation,
    HarmonyEncodingName,
    Message,
    Role,
    load_harmony_encoding,
)

from server.backend.llama_cpp_runner import (
    LlamaRunner,
    _get_env_bool,
    _get_env_int,
    get_model_context_length_gguf,
)
from server.schemas import ToolCallStart


ENCODING = load_harmony_encoding(HarmonyEncodingName.HARMONY_GPT_OSS)


class FakeModel:
    def __init__(self, completion):
        self.completion = completion
        self.was_reset = False

    def generate(self, prompt_tokens, **kwargs):
        yield from ENCODING.encode(self.completion, allowed_special="all")

    def reset(self):
        self.was_reset = True


def collect_completion(completion):
    runner = LlamaRunner("/tmp/gpt-oss")
    runner.model = FakeModel(completion)
    runner._context_length = 8192

    return list(
        runner.generate_streaming_gpt(
            Conversation.from_messages([]), max_tokens=128
        )
    )


def test_gpt_streaming_emits_tool_call_without_answer_marker():
    chunks = collect_completion(
        "<|channel|>analysis<|message|>Need to read file.<|end|>"
        "<|start|>assistant<|channel|>commentary to=read <|constrain|>json"
        '<|message|>{"path":"changelog.md"}<|call|>'
    )

    assert ToolCallStart("read") in chunks
    assert "".join(chunk for chunk in chunks if isinstance(chunk, str)) == (
        "**[Reasoning]**\n\n"
        "Need to read file."
        '{"path":"changelog.md"}'
    )


def test_gpt_streaming_emits_final_answer_marker():
    chunks = collect_completion(
        "<|channel|>analysis<|message|>Say hello.<|end|>"
        "<|start|>assistant<|channel|>final<|message|>Hello.<|return|>"
    )

    assert "".join(chunk for chunk in chunks if isinstance(chunk, str)) == (
        "**[Reasoning]**\n\n"
        "Say hello."
        "\n---\n**[Answer]**\n\n"
        "Hello."
    )


def test_gpt_streaming_rejects_prompt_that_exceeds_context_and_resets_model():
    runner = LlamaRunner("/tmp/gpt-oss")
    model = FakeModel("")
    runner.model = model
    runner._context_length = 64
    conversation = Conversation.from_messages(
        [Message.from_role_and_content(Role.USER, "word " * 200)]
    )

    with pytest.raises(ValueError, match="Start a new session"):
        list(runner.generate_streaming_gpt(conversation, max_tokens=16))

    assert model.was_reset


def test_gguf_context_length_caps_at_30000_by_default(tmp_path):
    (tmp_path / "config.json").write_text('{"context_length": 131072}')

    assert get_model_context_length_gguf(str(tmp_path)) == 30000


def test_gguf_context_length_uses_env_cap(tmp_path, monkeypatch):
    (tmp_path / "config.json").write_text('{"context_length": 131072}')
    monkeypatch.setenv("TILES_LLAMA_CPP_MAX_CTX", "12000")

    assert get_model_context_length_gguf(str(tmp_path)) == 12000


def test_llama_cpp_env_helpers(monkeypatch):
    monkeypatch.setenv("TILES_TEST_INT", "12")
    monkeypatch.setenv("TILES_TEST_BOOL", "false")

    assert _get_env_int("TILES_TEST_INT", 3) == 12
    assert _get_env_int("TILES_MISSING_INT", 3) == 3
    assert _get_env_bool("TILES_TEST_BOOL", True) is False
    assert _get_env_bool("TILES_MISSING_BOOL", True) is True
