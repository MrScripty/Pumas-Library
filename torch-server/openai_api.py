"""OpenAI-compatible API endpoints.

Provides /v1/models, /v1/chat/completions, and /v1/completions.
"""

import json
import logging
import time
import uuid
from typing import AsyncGenerator, Optional

import torch
from fastapi import APIRouter, HTTPException, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)

router = APIRouter()


# --- Request/Response Models ---

class ChatMessage(BaseModel):
    role: str
    content: str


class ChatCompletionRequest(BaseModel):
    model: str
    messages: list[ChatMessage]
    temperature: float = 0.7
    top_p: float = 1.0
    max_tokens: Optional[int] = None
    stream: bool = False
    stop: Optional[list[str]] = None


class CompletionRequest(BaseModel):
    model: str
    prompt: str
    temperature: float = 0.7
    top_p: float = 1.0
    max_tokens: Optional[int] = 256
    stream: bool = False
    stop: Optional[list[str]] = None


class ChatChoice(BaseModel):
    index: int = 0
    message: ChatMessage
    finish_reason: str = "stop"


class CompletionChoice(BaseModel):
    index: int = 0
    text: str
    finish_reason: str = "stop"


class UsageInfo(BaseModel):
    prompt_tokens: int = 0
    completion_tokens: int = 0
    total_tokens: int = 0


class ChatCompletionResponse(BaseModel):
    id: str
    object: str = "chat.completion"
    created: int
    model: str
    choices: list[ChatChoice]
    usage: UsageInfo = Field(default_factory=UsageInfo)


class CompletionResponse(BaseModel):
    id: str
    object: str = "text_completion"
    created: int
    model: str
    choices: list[CompletionChoice]
    usage: UsageInfo = Field(default_factory=UsageInfo)


# --- Endpoints ---

@router.get("/models")
async def list_models(request: Request):
    """List loaded models in OpenAI format."""
    manager = request.app.state.model_manager
    model_names = manager.list_model_names()

    return {
        "object": "list",
        "data": [
            {
                "id": name,
                "object": "model",
                "created": int(time.time()),
                "owned_by": "local",
            }
            for name in model_names
        ],
    }


@router.post("/chat/completions")
async def chat_completions(req: ChatCompletionRequest, request: Request):
    """Chat completion endpoint (streaming + non-streaming)."""
    manager = request.app.state.model_manager
    loaded = manager.get_model_for_inference(req.model)

    if loaded is None:
        raise HTTPException(status_code=404, detail=f"Model '{req.model}' not loaded")

    if req.stream:
        return StreamingResponse(
            _stream_chat(loaded, req),
            media_type="text/event-stream",
        )

    # Non-streaming
    output_text = _generate(loaded, _format_chat_prompt(req.messages), req)

    return ChatCompletionResponse(
        id=f"chatcmpl-{uuid.uuid4().hex[:8]}",
        created=int(time.time()),
        model=req.model,
        choices=[
            ChatChoice(message=ChatMessage(role="assistant", content=output_text))
        ],
    )


@router.post("/completions")
async def completions(req: CompletionRequest, request: Request):
    """Text completion endpoint."""
    manager = request.app.state.model_manager
    loaded = manager.get_model_for_inference(req.model)

    if loaded is None:
        raise HTTPException(status_code=404, detail=f"Model '{req.model}' not loaded")

    if req.stream:
        return StreamingResponse(
            _stream_completion(loaded, req),
            media_type="text/event-stream",
        )

    output_text = _generate(loaded, req.prompt, req)

    return CompletionResponse(
        id=f"cmpl-{uuid.uuid4().hex[:8]}",
        created=int(time.time()),
        model=req.model,
        choices=[CompletionChoice(text=output_text)],
    )


# --- Generation Helpers ---

def _format_chat_prompt(messages: list[ChatMessage]) -> str:
    """Format chat messages into a single prompt string."""
    parts = []
    for msg in messages:
        if msg.role == "system":
            parts.append(f"System: {msg.content}")
        elif msg.role == "user":
            parts.append(f"User: {msg.content}")
        elif msg.role == "assistant":
            parts.append(f"Assistant: {msg.content}")
    parts.append("Assistant:")
    return "\n".join(parts)


def _generate(loaded, prompt: str, req) -> str:
    """Generate text from a loaded model."""
    tokenizer = loaded.tokenizer
    model = loaded.model
    device = loaded.device

    inputs = tokenizer(prompt, return_tensors="pt").to(device)
    max_new = getattr(req, "max_tokens", None) or 256

    with torch.no_grad():
        outputs = model.generate(
            **inputs,
            max_new_tokens=max_new,
            temperature=max(req.temperature, 0.01),
            top_p=req.top_p,
            do_sample=req.temperature > 0,
        )

    # Decode only the newly generated tokens
    input_len = inputs["input_ids"].shape[1]
    generated = outputs[0][input_len:]
    return tokenizer.decode(generated, skip_special_tokens=True)


async def _stream_chat(loaded, req: ChatCompletionRequest) -> AsyncGenerator[str, None]:
    """Stream chat completion tokens via SSE."""
    prompt = _format_chat_prompt(req.messages)
    resp_id = f"chatcmpl-{uuid.uuid4().hex[:8]}"

    async for token in _stream_tokens(loaded, prompt, req):
        chunk = {
            "id": resp_id,
            "object": "chat.completion.chunk",
            "created": int(time.time()),
            "model": req.model,
            "choices": [
                {
                    "index": 0,
                    "delta": {"content": token},
                    "finish_reason": None,
                }
            ],
        }
        yield f"data: {json.dumps(chunk)}\n\n"

    # Final chunk
    final = {
        "id": resp_id,
        "object": "chat.completion.chunk",
        "created": int(time.time()),
        "model": req.model,
        "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
    }
    yield f"data: {json.dumps(final)}\n\n"
    yield "data: [DONE]\n\n"


async def _stream_completion(loaded, req: CompletionRequest) -> AsyncGenerator[str, None]:
    """Stream text completion tokens via SSE."""
    resp_id = f"cmpl-{uuid.uuid4().hex[:8]}"

    async for token in _stream_tokens(loaded, req.prompt, req):
        chunk = {
            "id": resp_id,
            "object": "text_completion",
            "created": int(time.time()),
            "model": req.model,
            "choices": [{"index": 0, "text": token, "finish_reason": None}],
        }
        yield f"data: {json.dumps(chunk)}\n\n"

    final = {
        "id": resp_id,
        "object": "text_completion",
        "created": int(time.time()),
        "model": req.model,
        "choices": [{"index": 0, "text": "", "finish_reason": "stop"}],
    }
    yield f"data: {json.dumps(final)}\n\n"
    yield "data: [DONE]\n\n"


async def _stream_tokens(loaded, prompt: str, req) -> AsyncGenerator[str, None]:
    """Generate and yield tokens one at a time."""
    import asyncio

    tokenizer = loaded.tokenizer
    model = loaded.model
    device = loaded.device

    inputs = tokenizer(prompt, return_tensors="pt").to(device)
    max_new = getattr(req, "max_tokens", None) or 256
    input_ids = inputs["input_ids"]

    for _ in range(max_new):
        with torch.no_grad():
            outputs = model(input_ids)
            logits = outputs.logits[:, -1, :]

            if req.temperature > 0:
                logits = logits / max(req.temperature, 0.01)
                probs = torch.softmax(logits, dim=-1)
                next_token = torch.multinomial(probs, num_samples=1)
            else:
                next_token = logits.argmax(dim=-1, keepdim=True)

        if next_token.item() == tokenizer.eos_token_id:
            break

        token_str = tokenizer.decode(next_token[0], skip_special_tokens=True)
        yield token_str

        input_ids = torch.cat([input_ids, next_token], dim=-1)

        # Yield control to event loop between tokens
        await asyncio.sleep(0)
