"""OpenAI-shaped text-generation API endpoints.

Provides /v1/models, /v1/chat/completions, and /v1/completions.

Generation runs on a worker thread under the slot's device lease, so the event
loop keeps answering /health and control routes. A client disconnect or owner
cancellation stops the worker at its next decoding step; the lease is held
until the worker has actually returned.
"""

import asyncio
import functools
import logging
import threading
import time
import uuid
from contextlib import AsyncExitStack
from typing import Literal

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator

from model_manager import SlotConflict

logger = logging.getLogger(__name__)

router = APIRouter()

# Request-body safety bounds, not model limits: they cap parsing and tokenizer
# work before the model-derived context check runs. Output length is bounded
# only by the loaded model's context (see _check_context).
MODEL_ID_MAX_CHARS = 256
INPUT_TEXT_MAX_CHARS = 1_000_000
CHAT_MESSAGES_MAX_ITEMS = 256

# Config fields that name a causal model's maximum sequence length, in the
# order transformers architectures use them.
_POSITION_LIMIT_FIELDS = (
    "max_position_embeddings",
    "n_positions",
    "max_sequence_length",
    "seq_length",
    "n_ctx",
)
_WORKER_POLL_SECONDS = 0.1


class TextGenerationCancelled(RuntimeError):
    """The worker observed cancellation and stopped generating."""


class ContextLengthExceeded(ValueError):
    """The prompt plus requested output does not fit the model's context."""


# --- Request/Response Models ---


class ClosedRequestModel(BaseModel):
    """Reject input outside the explicitly supported request contract."""

    model_config = ConfigDict(extra="forbid", strict=True)


class ChatMessage(ClosedRequestModel):
    role: Literal["system", "user", "assistant"]
    content: str = Field(min_length=1, max_length=INPUT_TEXT_MAX_CHARS)


class TextGenerationRequest(ClosedRequestModel):
    model: str = Field(min_length=1, max_length=MODEL_ID_MAX_CHARS)
    temperature: float = Field(default=0.7, ge=0.0, le=2.0)
    top_p: float = Field(default=1.0, gt=0.0, le=1.0)
    max_tokens: int = Field(default=256, ge=1)
    stream: Literal[False] = False
    stop: None = None

    @field_validator("model")
    @classmethod
    def validate_model_identity(cls, value: str) -> str:
        if value.isspace():
            raise ValueError("model must not be blank")
        return value

    @model_validator(mode="after")
    def validate_sampling_contract(self) -> "TextGenerationRequest":
        if self.temperature == 0.0 and self.top_p != 1.0:
            raise ValueError("top_p must be 1 when temperature is 0")
        return self


class ChatCompletionRequest(TextGenerationRequest):
    messages: list[ChatMessage] = Field(min_length=1, max_length=CHAT_MESSAGES_MAX_ITEMS)

    @model_validator(mode="after")
    def validate_total_message_size(self) -> "ChatCompletionRequest":
        if sum(len(message.content) for message in self.messages) > INPUT_TEXT_MAX_CHARS:
            raise ValueError(f"messages exceed {INPUT_TEXT_MAX_CHARS} characters")
        return self


class CompletionRequest(TextGenerationRequest):
    prompt: str = Field(min_length=1, max_length=INPUT_TEXT_MAX_CHARS)


class AssistantMessage(BaseModel):
    role: Literal["assistant"]
    content: str


class ChatChoice(BaseModel):
    index: int = 0
    message: AssistantMessage
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
    """Generate one non-streaming text response for a chat prompt."""
    output_text = await _leased_generation(request, req, _format_chat_prompt(req.messages))

    return ChatCompletionResponse(
        id=f"chatcmpl-{uuid.uuid4().hex[:8]}",
        created=int(time.time()),
        model=req.model,
        choices=[ChatChoice(message=AssistantMessage(role="assistant", content=output_text))],
    )


@router.post("/completions")
async def completions(req: CompletionRequest, request: Request):
    """Generate one non-streaming text completion."""
    output_text = await _leased_generation(request, req, req.prompt)

    return CompletionResponse(
        id=f"cmpl-{uuid.uuid4().hex[:8]}",
        created=int(time.time()),
        model=req.model,
        choices=[CompletionChoice(text=output_text)],
    )


# --- Generation Helpers ---


def failure(status: int, code: str, message: str) -> HTTPException:
    """A refusal with a stable code and a caller-safe reason."""
    return HTTPException(status_code=status, detail={"code": code, "message": message})


def _is_cuda_oom(error: BaseException) -> bool:
    oom = getattr(getattr(torch, "cuda", None), "OutOfMemoryError", None)
    return isinstance(oom, type) and isinstance(error, oom)


async def _leased_generation(request: Request, req: TextGenerationRequest, prompt: str) -> str:
    """Run one generation under the slot's device lease and map its outcome."""
    manager = request.app.state.model_manager
    async with AsyncExitStack() as lease:
        try:
            loaded = await lease.enter_async_context(manager.text_lease(req.model))
        except KeyError:
            raise failure(404, "model_unavailable", f"Model '{req.model}' not loaded") from None
        except ValueError as error:
            raise failure(400, "unsupported_model", str(error)) from None
        except SlotConflict as error:
            raise failure(409, error.code, str(error)) from None

        cancel = threading.Event()
        work = functools.partial(_generate, loaded, prompt, req, cancel)
        try:
            return await run_owned_worker(work, cancel, request)
        except ContextLengthExceeded as error:
            raise failure(400, "context_length_exceeded", str(error)) from None
        except TextGenerationCancelled:
            raise failure(499, "cancelled", "Text generation stopped") from None
        except Exception as error:
            if _is_cuda_oom(error):
                logger.warning("Text generation ran out of device memory", exc_info=True)
                # Release the failed step's cached blocks while the lease is held.
                torch.cuda.empty_cache()
                raise failure(
                    507,
                    "out_of_memory",
                    "Insufficient device memory for this generation; "
                    "shorten the prompt, lower max_tokens, or free memory",
                ) from None
            logger.exception("Text generation failed")
            raise failure(
                500, "backend_failure", "Text runtime failed; inspect the Pumas runtime log"
            ) from None


async def run_owned_worker(work, cancel: threading.Event, request: Request):
    """Run ``work`` on a thread and return only after that thread has stopped.

    A disconnect sets ``cancel``; the worker's result then becomes a
    TextGenerationCancelled outcome. Task cancellation also sets ``cancel`` and
    is re-raised only after the worker returns, so the caller's lease is never
    released while device work is still running.
    """
    worker = asyncio.create_task(asyncio.to_thread(work))
    owner_cancellation = None
    try:
        while not worker.done():
            if not cancel.is_set() and await request.is_disconnected():
                logger.info("Text generation cancellation requested by disconnect")
                cancel.set()
            # Poll instead of awaiting the worker: cancelling an awaited
            # wrapper would not stop its executor thread.
            await asyncio.sleep(_WORKER_POLL_SECONDS)
    except asyncio.CancelledError as error:
        cancel.set()
        owner_cancellation = error
        task = asyncio.current_task()
        if task is not None:
            task.uncancel()
    finally:
        if not worker.done():
            cancel.set()
        while not worker.done():
            try:
                await asyncio.sleep(0.01)
            except asyncio.CancelledError:
                task = asyncio.current_task()
                if task is not None:
                    task.uncancel()
    if owner_cancellation is not None:
        if not worker.cancelled():
            worker.exception()  # retrieved; the owner's cancellation wins
        raise owner_cancellation
    result = worker.result()
    if cancel.is_set():
        raise TextGenerationCancelled("Text generation stopped")
    return result


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


def model_position_limit(model) -> tuple[int, str] | None:
    """Return the model's maximum sequence length and the config field naming it."""
    config = getattr(model, "config", None)
    for name in _POSITION_LIMIT_FIELDS:
        value = getattr(config, name, None)
        if isinstance(value, int) and not isinstance(value, bool) and value > 0:
            return value, name
    return None


def _check_context(model, input_len: int, max_tokens: int) -> None:
    """Refuse, never truncate, a request that cannot fit the model's context."""
    limit = model_position_limit(model)
    if limit is None:
        return
    positions, field = limit
    if input_len + max_tokens > positions:
        raise ContextLengthExceeded(
            f"Prompt is {input_len} tokens and max_tokens is {max_tokens}; together they "
            f"exceed this model's {positions}-token context ({field}). Shorten the prompt "
            f"or lower max_tokens to at most {max(positions - input_len, 0)}."
        )


def _cancel_criteria(cancel: threading.Event):
    """A transformers StoppingCriteria that ends decoding once ``cancel`` is set."""
    from transformers import StoppingCriteria, StoppingCriteriaList

    class CancelRequested(StoppingCriteria):
        def __call__(self, input_ids, scores, **kwargs):
            stop = cancel.is_set()
            return torch.full(
                (input_ids.shape[0],), stop, dtype=torch.bool, device=input_ids.device
            )

    return StoppingCriteriaList([CancelRequested()])


def _generate(loaded, prompt: str, req, cancel: threading.Event) -> str:
    """Generate text from a loaded model (runs on a worker thread)."""
    tokenizer = loaded.tokenizer
    model = loaded.model
    device = loaded.device

    if cancel.is_set():
        raise TextGenerationCancelled("Text generation stopped")
    inputs = tokenizer(prompt, return_tensors="pt").to(device)
    input_len = inputs["input_ids"].shape[-1]
    _check_context(model, input_len, req.max_tokens)
    with torch.no_grad():
        outputs = model.generate(
            **inputs,
            max_new_tokens=req.max_tokens,
            temperature=max(req.temperature, 0.01),
            top_p=req.top_p,
            do_sample=req.temperature > 0,
            stopping_criteria=_cancel_criteria(cancel),
        )
    if cancel.is_set():
        raise TextGenerationCancelled("Text generation stopped")

    # Decode only the newly generated tokens
    generated = outputs[0][input_len:]
    return tokenizer.decode(generated, skip_special_tokens=True)
