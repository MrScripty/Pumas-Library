"""Private text embedding and rerank operations for the Pumas gateway.

Mounted under ``/api`` beside the image and audio operations. Each request
names a loaded ``model_id``; responses use the shapes llama.cpp's
``/v1/embeddings`` and ``/v1/rerank`` return, so Pumas projects them the same
way. As with the other operations, a disconnect requests cancellation between
batches, and the device lease stays held until the worker has stopped.
"""

import asyncio
import logging
import threading
from contextlib import AsyncExitStack

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field, field_validator
from validation import MODEL_NAME_MAX_LENGTH

logger = logging.getLogger(__name__)

router = APIRouter()

MAX_ITEMS = 1024


class EmbeddingRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)
    model_id: str = Field(min_length=1, max_length=MODEL_NAME_MAX_LENGTH)
    input: list[str] = Field(min_length=1, max_length=MAX_ITEMS)
    dimensions: int | None = Field(default=None, gt=0)


class RerankRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)
    model_id: str = Field(min_length=1, max_length=MODEL_NAME_MAX_LENGTH)
    query: str = Field(min_length=1)
    documents: list[str] = Field(min_length=1, max_length=MAX_ITEMS)

    @field_validator("query")
    @classmethod
    def _query_nonblank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("query must be nonblank")
        return value

    @field_validator("documents")
    @classmethod
    def _documents_nonblank(cls, value: list[str]) -> list[str]:
        if any(not document.strip() for document in value):
            raise ValueError("every document must be nonblank")
        return value


def _is_cuda_oom(error: BaseException) -> bool:
    oom = getattr(getattr(torch, "cuda", None), "OutOfMemoryError", None)
    return isinstance(oom, type) and isinstance(error, oom)


def failure(status: int, code: str, message: str):
    """A refusal with a stable code and a caller-safe reason."""
    return HTTPException(status_code=status, detail={"code": code, "message": message})


def _defer_current_cancellation() -> None:
    task = asyncio.current_task()
    if task is not None:
        task.uncancel()


async def _owned(work, request: Request):
    """Run ``work(cancel)`` in a thread, holding the caller's lease until it stops."""
    from loaders.text_encoder_loader import TextEncoderCancelled

    cancel = threading.Event()
    worker = asyncio.create_task(asyncio.to_thread(work, cancel))
    owner_cancellation = None
    try:
        while not worker.done():
            if await request.is_disconnected():
                cancel.set()
            await asyncio.sleep(0.02)
        try:
            return worker.result()
        except TextEncoderCancelled:
            raise failure(499, "cancelled", "Request stopped") from None
    except asyncio.CancelledError as error:
        cancel.set()
        _defer_current_cancellation()
        owner_cancellation = error
    finally:
        cancel.set()
        while not worker.done():
            try:
                await asyncio.sleep(0.01)
            except asyncio.CancelledError:
                _defer_current_cancellation()
        if not worker.cancelled():
            worker.exception()
    if owner_cancellation is not None:
        raise owner_cancellation


async def _leased(request: Request, model_id: str, kind: str, work):
    from model_manager import SlotConflict

    async with AsyncExitStack() as lease:
        try:
            loaded = await lease.enter_async_context(
                request.app.state.model_manager.text_encoder_lease(model_id, kind)
            )
        except KeyError:
            raise failure(503, "model_unavailable", "Load the model in Pumas first") from None
        except ValueError as error:
            raise failure(400, "unsupported_model", str(error)) from None
        except SlotConflict as error:
            raise failure(409, error.code, str(error)) from None
        try:
            return await _owned(lambda cancel: work(loaded.model, cancel), request)
        except HTTPException:
            raise
        except Exception as error:
            if _is_cuda_oom(error):
                logger.warning("Text encoding ran out of GPU memory", exc_info=True)
                raise failure(
                    507, "out_of_memory", "Insufficient GPU memory; free memory before retrying"
                ) from error
            if isinstance(error, ValueError):
                # Pumas-authored refusals (token limits, dimensions) carry the reason.
                raise failure(400, "invalid_request", str(error)) from error
            logger.exception("Text encoding failed")
            raise failure(
                502, "backend_failure", "Text runtime failed; inspect the Pumas runtime log"
            ) from error


@router.post("/text/embeddings")
async def text_embeddings(payload: EmbeddingRequest, request: Request):
    from loaders.text_encoder_loader import TEXT_EMBEDDING, embed

    vectors, tokens = await _leased(
        request,
        payload.model_id,
        TEXT_EMBEDDING,
        lambda encoder, cancel: embed(encoder, payload.input, payload.dimensions, cancel),
    )
    return {
        "object": "list",
        "data": [
            {"object": "embedding", "index": index, "embedding": vector}
            for index, vector in enumerate(vectors)
        ],
        "usage": {"prompt_tokens": tokens, "total_tokens": tokens},
    }


@router.post("/text/rerank")
async def text_rerank(payload: RerankRequest, request: Request):
    from loaders.text_encoder_loader import TEXT_RERANK, rerank

    scores, tokens = await _leased(
        request,
        payload.model_id,
        TEXT_RERANK,
        lambda encoder, cancel: rerank(encoder, payload.query, payload.documents, cancel),
    )
    return {
        "results": [
            {"index": index, "relevance_score": score} for index, score in enumerate(scores)
        ],
        "usage": {"prompt_tokens": tokens, "total_tokens": tokens},
    }
