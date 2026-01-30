from pydantic import BaseModel, Field
from typing import Any, Dict, List, Optional, Union
from dataclasses import dataclass


class CompletionRequest(BaseModel):
    model: str
    prompt: Union[str, List[str]]
    max_tokens: Optional[int] = None
    temperature: Optional[float] = 0.7
    top_p: Optional[float] = 0.9
    stream: Optional[bool] = False
    stop: Optional[Union[str, List[str]]] = None
    repetition_penalty: Optional[float] = 1.1


class ChatMessage(BaseModel):
    role: str = Field(..., pattern="^(system|user|assistant)$")
    content: str


class ChatCompletionRequest(BaseModel):
    model: str
    messages: List[ChatMessage]
    chat_start: bool
    python_code: str
    max_tokens: Optional[int] = None
    temperature: Optional[float] = 0.7
    top_p: Optional[float] = 0.9
    stream: Optional[bool] = False
    stop: Optional[Union[str, List[str]]] = None
    repetition_penalty: Optional[float] = 1.1


class CompletionResponse(BaseModel):
    id: str
    object: str = "text_completion"
    created: int
    model: str
    choices: List[Dict[str, Any]]
    usage: Dict[str, int]


class ChatCompletionResponse(BaseModel):
    id: str
    object: str = "chat.completion"
    created: int
    model: str
    choices: List[Dict[str, Any]]
    # usage: Dict[str, int]


class ModelInfo(BaseModel):
    id: str
    object: str = "model"
    owned_by: str = "mlx-knife"
    permission: List = []
    context_length: Optional[int] = None


class StartRequest(BaseModel):
    model: str
    memory_path: str
    system_prompt: str


class downloadRequest(BaseModel):
    model: str


class ResponsesRequest(BaseModel):
    model: Optional[str] = None
    input: Optional[str] = None
    reasoning: Optional[Dict[str, Any]] = None
    previous_response_id: Optional[str] = None
    stream: Optional[bool] = False
    tools: Optional[List[Dict[str, Any]]] = None
    temperature: Optional[float] = 1
    top_p: Optional[float] = 1
    max_output_tokens: Optional[int] = None


class ResponsesResponse(BaseModel):
    id: str
    object: str = "response"
    created_at: int
    status: str
    completed_at: Optional[int] = None
    error: Optional[Dict[str, Any]] = None
    incomplete_details: Optional[Dict[str, Any]] = None
    instructions: Optional[str] = None
    max_output_tokens: Optional[int] = None
    model: str
    output: List[Dict[str, Any]]
    parallel_tool_calls: bool = True
    previous_response_id: Optional[str] = None
    reasoning: Optional[Dict[str, Any]] = Field(default_factory=dict)
    store: bool = True
    temperature: float = 1.0
    text: Dict[str, Any] = Field(default_factory=lambda: {"format": {"type": "text"}})
    tool_choice: Union[str, Dict[str, Any]] = "auto"
    tools: List[Dict[str, Any]] = Field(default_factory=list)
    top_p: float = 1.0
    truncation: str = "disabled"
    usage: Dict[str, Any]
    user: Optional[str] = None
    metadata: Dict[str, Any] = Field(default_factory=dict)


@dataclass
class GenerationMetrics:
    """Benchmarking metrics for token generation."""

    ttft_ms: float  # Time to first token in milliseconds
    total_tokens: int  # Total tokens generated
    tokens_per_second: float  # Throughput
    total_latency_s: float  # End-to-end latency in seconds
