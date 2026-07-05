from server.backend.commons import _find_tool
from server.schemas import ResponsesRequest


def test_find_tool_does_not_fabricate_read_when_arguments_do_not_match():
    request = ResponsesRequest.model_validate(
        {
            "model": "unsloth/gpt-oss-20b-GGUF",
            "input": "hello",
            "tools": [
                {
                    "type": "function",
                    "name": "read",
                    "parameters": {
                        "type": "object",
                        "required": ["path"],
                        "properties": {"path": {"type": "string"}},
                    },
                }
            ],
        }
    )

    assert _find_tool(request.tools, "{}") is None
    assert _find_tool(request.tools, "{") is None


def test_optimize_arguments_cmd_to_command():
    model_args = {"cmd": "ls -R ."}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") is not None


def test_optimize_arguments_remove_ls_recursive():
    model_args = {"cmd": "ls -R ."}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") == "ls ."


def test_optimize_arguments_remove_ls_recursive_2():
    model_args = {"cmd": "LS -R ."}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") == "LS ."


def test_optimize_arguments_no_timeout_for_non_bash():
    model_args = {"cmd": "LS -R ."}

    new_args = optimize_arguments("read", model_args)
    assert new_args.get("command") == "LS -R ."
    assert new_args.get("timeout") is None


def test_optimize_arguments_default_timeout_bash_calls():
    model_args = {"cmd": "LS -R ."}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") == "LS ."
    assert new_args.get("timeout") == 60


def test_optimize_arguments_default_timeout_override_bash_calls():
    model_args = {"cmd": "LS -R .", "timeout": 20}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") == "LS ."
    assert new_args.get("timeout") == 20


def test_optimize_arguments_default_timeout_override_bash_calls_but_clamped():
    model_args = {"cmd": "LS -R .", "timeout": 200}

    new_args = optimize_arguments("bash", model_args)
    assert new_args.get("command") == "LS ."
    assert new_args.get("timeout") == 60
