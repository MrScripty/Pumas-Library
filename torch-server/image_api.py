"""Private image generation provider operation for the Pumas gateway.

The public OpenAI-compatible image route is gateway-owned. This module
exposes only the private provider operation mounted under ``/api``: a
strict request with an explicit ``model_id`` and numeric dimensions, and
a private result carrying the PNG payload plus run metadata.

Admitted generation is duration-unbounded: elapsed time and response silence
never fail the operation. A disconnect or owner cancellation requests worker
cancellation, but the device lease remains held until that worker has actually
stopped. A lost transport before a terminal result is an unknown outcome and
is never replayed here.
"""

import asyncio
import base64
import io
import logging
import secrets
import threading
import time
from contextlib import AsyncExitStack

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field, field_validator

from diffusion import GenerationCancelled

logger = logging.getLogger(__name__)

router = APIRouter()
MAX_PNG_BYTES = 8 * 1024 * 1024


class ImageRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)
    model_id: str = Field(min_length=1, max_length=256)
    prompt: str = Field(min_length=1, max_length=4000)
    width: int = Field(gt=0)
    height: int = Field(gt=0)
    seed: int | None = Field(default=None, ge=0, le=4294967295)

    @field_validator("prompt", "model_id")
    @classmethod
    def nonblank(cls, value):
        if not value.strip():
            raise ValueError("Value must not be blank")
        return value


def failure(status: int, code: str, message: str):
    return HTTPException(status_code=status, detail={"code": code, "message": message})


def _diagnose(state: str, message: str, *, warning: bool = False) -> None:
    log = logger.warning if warning else logger.info
    log(message, extra={"generation_state": state})


def _defer_current_cancellation() -> None:
    """Consume one task-cancel request while owned worker cleanup finishes."""
    task = asyncio.current_task()
    if task is not None:
        task.uncancel()


async def owned_generation(adapter, payload: ImageRequest, request: Request, clock=time.monotonic):
    """Run one admitted generation without an elapsed-duration deadline.

    ``clock`` is injectable only for duration reporting. Advancing it cannot
    stop the operation.
    """
    cancel = threading.Event()
    seed = payload.seed if payload.seed is not None else secrets.randbits(32)
    width, height = payload.width, payload.height
    started = clock()
    worker = asyncio.create_task(
        asyncio.to_thread(adapter.generate, payload.prompt, width, height, seed, cancel)
    )
    cancellation_requested = False
    cancellation_completed = False
    owner_cancellation = None
    try:
        while not worker.done():
            if await request.is_disconnected():
                if not cancellation_requested:
                    cancellation_requested = True
                    _diagnose("cancellation_requested", "Image generation cancellation requested")
                cancel.set()
            # Poll the independently owned worker. Awaiting it, even through a
            # wait wrapper, would let request-task cancellation mark the
            # asyncio wrapper done while its executor thread kept running.
            await asyncio.sleep(0.1)
        try:
            image = worker.result()
        except GenerationCancelled:
            cancellation_completed = True
            _diagnose("cancellation_completed", "Image generation cancellation completed")
            raise failure(499, "cancelled", "Image generation stopped") from None
        except Exception:
            _diagnose("runtime_failure", "Image generation runtime failed", warning=True)
            raise
        if cancellation_requested:
            cancellation_completed = True
            _diagnose(
                "cancellation_completed",
                "Image generation stopped after disconnect; discarding its late result",
            )
            raise failure(499, "cancelled", "Image generation stopped")
        output = io.BytesIO()
        image.save(output, format="PNG")
        content = output.getvalue()
        if image.size != (width, height) or len(content) > MAX_PNG_BYTES:
            raise failure(502, "invalid_backend_response", "Runtime returned an invalid image")
        result = {
            "png_base64": base64.b64encode(content).decode("ascii"),
            "seed": seed,
            "steps": adapter.steps,
            "guidance": adapter.guidance,
            "memory_policy": adapter.memory_policy,
            "duration_seconds": round(clock() - started, 3),
        }
        _diagnose("completed", "Image generation completed")
        return result
    except asyncio.CancelledError as error:
        if not cancellation_requested:
            cancellation_requested = True
            _diagnose("cancellation_requested", "Image generation owner cancelled")
        cancel.set()
        # Consume this cancellation until the executor worker has stopped.
        # Re-raising before cleanup would make every shielded await observe the
        # same cancellation immediately and could release neither task nor
        # lease safely.
        _defer_current_cancellation()
        owner_cancellation = error
    finally:
        cancel.set()
        # Cancelling an asyncio task does not stop its executor thread. Keep the
        # caller's GPU lease until the worker confirms it has actually stopped.
        if not worker.done():
            _diagnose(
                "cleanup_pending",
                "Image generation cleanup pending; retaining device lease",
            )
        while not worker.done():
            try:
                # Poll rather than await the worker directly: direct task
                # cancellation propagation could cancel the wrapper while its
                # executor thread continues running.
                await asyncio.sleep(0.01)
            except asyncio.CancelledError:
                _defer_current_cancellation()
                continue
        if worker.done() and not worker.cancelled():
            worker.exception()  # retrieve a failure after transport cancellation
        if cancellation_requested and not cancellation_completed:
            _diagnose("cancellation_completed", "Image generation cancellation completed")
    if owner_cancellation is not None:
        raise owner_cancellation


@router.post("/images/generate")
async def generate_image(payload: ImageRequest, request: Request):
    async with AsyncExitStack() as lease:
        try:
            adapter = await lease.enter_async_context(
                request.app.state.model_manager.image_lease(payload.model_id)
            )
        except KeyError:
            raise failure(503, "model_unavailable", "Load the image model in Pumas first") from None
        except ValueError:
            raise failure(
                400, "unsupported_model", "Selected model does not support this operation"
            ) from None
        except RuntimeError as error:
            if str(error) == "Image runtime is busy":
                raise failure(409, "runtime_busy", "Image runtime is busy") from None
            logger.exception("Image lease acquisition failed")
            raise failure(
                502, "backend_failure", "Image runtime failed; inspect the Pumas runtime log"
            ) from error

        try:
            return await owned_generation(adapter, payload, request)
        except HTTPException:
            raise
        except torch.cuda.OutOfMemoryError as error:
            logger.warning("Image generation ran out of GPU memory", exc_info=True)
            raise failure(
                507, "out_of_memory", "Insufficient GPU memory; free memory before retrying"
            ) from error
        except Exception as error:
            logger.exception("Image generation failed")
            raise failure(
                502, "backend_failure", "Image runtime failed; inspect the Pumas runtime log"
            ) from error
