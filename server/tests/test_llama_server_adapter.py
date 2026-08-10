import asyncio

from server.backend.llama_server.openresponses_adapter import (
    generate_response_chat_stream,
    openresponses_input_to_messages,
    openresponses_tools_to_chat_tools,
)
from server.schemas import ResponsesRequest


def _minimal_request() -> ResponsesRequest:
    return ResponsesRequest.model_validate(
        {
            "model": "test/model",
            "input": "hello",
            "stream": True,
        }
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


def test_request_body_includes_usage_streaming():
    """The adapter must request token usage in the stream so it doesn't fall
    back to length-based estimates.
    """
    captured: dict = {}

    async def fake_stream(body):
        captured.update(body)
        # An empty stream still yields a well-formed SSE sequence (created +
        # completed + [DONE]) because no deltas arrive.
        return
        yield  # make this an async generator

    import asyncio

    from unittest.mock import patch

    with patch(
        "server.backend.llama_server.openresponses_adapter.stream_chat_completions",
        new=fake_stream,
    ):
        asyncio.run(_drain(_minimal_request()))

    assert captured.get("stream_options") == {"include_usage": True}


async def _drain(request: ResponsesRequest):
    async for _ in generate_response_chat_stream(request, {}):
        pass


def _parse_sse(events: list[str]) -> list[tuple[str, dict]]:
    """Parse the yielded SSE strings into (event_name, payload) pairs.

    A single yielded string may contain several SSE events (separated by blank
    lines), so split on blank-line boundaries first.
    """
    import json

    parsed = []
    for chunk in events:
        for raw_event in chunk.split("\n\n"):
            name = None
            data = None
            for line in raw_event.splitlines():
                if line.startswith("event: "):
                    name = line[len("event: "):]
                elif line.startswith("data: "):
                    raw = line[len("data: "):]
                    if raw == "[DONE]" or not raw.startswith("{"):
                        continue
                    data = json.loads(raw)
            if name is not None:
                parsed.append((name, data or {}))
    return parsed


async def _drain_tool_stream(request: ResponsesRequest, chunks: list[dict]) -> list[str]:
    from unittest.mock import patch

    async def fake_stream(body):
        for c in chunks:
            yield c

    out: list[str] = []
    with patch(
        "server.backend.llama_server.openresponses_adapter.stream_chat_completions",
        new=fake_stream,
    ):
        async for piece in generate_response_chat_stream(request, {}):
            out.append(piece)
    return out


def test_tool_call_finish_reason_uses_consistent_id_and_no_double_emit():
    """A streamed tool call followed by finish_reason == "tool_calls" must emit
    exactly one function_call added/done pair with a consistent id — not mint a
    fresh id and re-emit a second done (the pre-fix bug).
    """
    import asyncio

    chunks = [
        {
            "choices": [
                {
                    "delta": {
                        "tool_calls": [
                            {
                                "index": 0,
                                "id": "call_x",
                                "function": {"name": "read", "arguments": '{"path":"a"}'},
                            }
                        ]
                    },
                    "finish_reason": None,
                }
            ]
        },
        {"choices": [{"delta": {}, "finish_reason": "tool_calls"}]},
    ]

    out = asyncio.run(_drain_tool_stream(_minimal_request(), chunks))
    events = _parse_sse(out)

    added = [d for n, d in events if n == "response.output_item.added" and d.get("item", {}).get("type") == "function_call"]
    done = [d for n, d in events if n == "response.output_item.done" and d.get("item", {}).get("type") == "function_call"]
    args_done = [d for n, d in events if n == "response.function_call_arguments.done"]

    assert len(added) == 1, f"expected 1 added, got {len(added)}"
    assert len(done) == 1, f"expected 1 done, got {len(done)}"
    assert len(args_done) == 1, f"expected 1 args.done, got {len(args_done)}"
    assert added[0]["item"]["id"] == done[0]["item"]["id"], "added/done tool ids must match"


def test_interleaved_tool_call_indices_keep_independent_state():
    """Two tool calls that interleave (index 0, then 1, then 0 again) must each
    get their own added/done events with consistent ids, and each index's
    argument deltas must accumulate only its own pieces — no cross-index mixing.
    """
    chunks = [
        {"choices": [{"delta": {"tool_calls": [
            {"index": 0, "id": "c0", "function": {"name": "read", "arguments": '{"p":"'}}
        ]}, "finish_reason": None}]},
        {"choices": [{"delta": {"tool_calls": [
            {"index": 1, "id": "c1", "function": {"name": "write", "arguments": '{"p":"b"}'}}
        ]}, "finish_reason": None}]},
        {"choices": [{"delta": {"tool_calls": [
            {"index": 0, "function": {"arguments": 'a","q":"c"}'}}
        ]}, "finish_reason": None}]},
        {"choices": [{"delta": {}, "finish_reason": "tool_calls"}]},
    ]

    out = asyncio.run(_drain_tool_stream(_minimal_request(), chunks))
    events = _parse_sse(out)

    added = [d for n, d in events if n == "response.output_item.added" and d.get("item", {}).get("type") == "function_call"]
    done = [d for n, d in events if n == "response.output_item.done" and d.get("item", {}).get("type") == "function_call"]
    args_delta = [d for n, d in events if n == "response.function_call_arguments.delta"]
    args_done = [d for n, d in events if n == "response.function_call_arguments.done"]

    assert len(added) == 2, f"expected 2 added (one per index), got {len(added)}"
    assert len(done) == 2, f"expected 2 done, got {len(done)}"
    assert len(args_done) == 2, f"expected 2 args.done, got {len(args_done)}"

    # Each added must match a done by id, and ids must be distinct per tool.
    added_ids = {d["item"]["id"] for d in added}
    done_ids = {d["item"]["id"] for d in done}
    assert added_ids == done_ids, f"added/done id sets differ: {added_ids} vs {done_ids}"
    assert len(added_ids) == 2, f"expected 2 distinct ids, got {added_ids}"

    # Names: one read, one write.
    assert {d["item"]["name"] for d in done} == {"read", "write"}

    # Argument deltas grouped by item_id: index 0's pieces concatenate to the
    # full JSON object; index 1's stand alone. No cross-contamination.
    by_id: dict[str, str] = {}
    for d in args_delta:
        by_id.setdefault(d["item_id"], "")
        by_id[d["item_id"]] += d["delta"]

    # Map ids back to names via the done events.
    id_to_name = {d["item"]["id"]: d["item"]["name"] for d in done}
    concat = {id_to_name[i]: txt for i, txt in by_id.items()}
    assert concat["read"] == '{"p":"a","q":"c"}', f"read args mangled: {concat['read']!r}"
    assert concat["write"] == '{"p":"b"}', f"write args mangled: {concat['write']!r}"
    # The 'b' from index 1 must not have leaked into index 0's buffer.
    assert "b" not in concat["read"]


def test_late_function_name_still_emits_added_event():
    """When argument chunks arrive before the function name, the added event must
    still fire (once the name is known) and the buffered arguments must be
    emitted — the pre-fix bug dropped the added event entirely.
    """
    chunks = [
        {"choices": [{"delta": {"tool_calls": [
            {"index": 0, "id": "c0", "function": {"arguments": '{"p":"a"}'}}
        ]}, "finish_reason": None}]},
        {"choices": [{"delta": {"tool_calls": [
            {"index": 0, "function": {"name": "read"}}
        ]}, "finish_reason": None}]},
        {"choices": [{"delta": {}, "finish_reason": "tool_calls"}]},
    ]

    out = asyncio.run(_drain_tool_stream(_minimal_request(), chunks))
    events = _parse_sse(out)

    added = [d for n, d in events if n == "response.output_item.added" and d.get("item", {}).get("type") == "function_call"]
    done = [d for n, d in events if n == "response.output_item.done" and d.get("item", {}).get("type") == "function_call"]
    args_delta = [d for n, d in events if n == "response.function_call_arguments.delta"]
    args_done = [d for n, d in events if n == "response.function_call_arguments.done"]

    assert len(added) == 1, f"expected 1 added (was dropped pre-fix), got {len(added)}"
    assert len(done) == 1, f"expected 1 done, got {len(done)}"
    assert len(args_delta) == 1, f"expected 1 buffered args delta, got {len(args_delta)}"
    assert len(args_done) == 1, f"expected 1 args.done, got {len(args_done)}"
    assert added[0]["item"]["id"] == done[0]["item"]["id"], "added/done ids must match"
    assert added[0]["item"]["name"] == "read"
    # The buffered arguments arrive as a single delta once the name is known.
    assert args_delta[0]["delta"] == '{"p":"a"}'
    assert args_delta[0]["item_id"] == added[0]["item"]["id"]
