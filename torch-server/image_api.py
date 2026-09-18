"""Private image generation provider operation for the Pumas gateway.

The public OpenAI-compatible image route is gateway-owned. This module
exposes only the private provider operation mounted under ``/api``: a
strict request with an explicit ``model_id`` and numeric dimensions, and
a private result carrying the PNG payload plus run metadata.
"""

import asyncio
import base64
import io
import secrets
import threading
import time

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field, field_validator

from diffusion import GenerationCancelled

router = APIRouter()
GENERATION_DEADLINE_SECONDS = 600
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


async def owned_generation(adapter, payload: ImageRequest, request: Request):
    cancel = threading.Event()
    seed = payload.seed if payload.seed is not None else secrets.randbits(32)
    width, height = payload.width, payload.height
    started = time.monotonic()
    worker = asyncio.create_task(
        asyncio.to_thread(adapter.generate, payload.prompt, width, height, seed, cancel)
    )
    stop_code = None
    try:
        while not worker.done():
            if time.monotonic() - started >= GENERATION_DEADLINE_SECONDS:
                stop_code = "deadline_exceeded"
                cancel.set()
            elif await request.is_disconnected():
                stop_code = "cancelled"
                cancel.set()
            await asyncio.wait({worker}, timeout=0.1)
        try:
            image = worker.result()
        except GenerationCancelled:
            raise failure(
                504 if stop_code == "deadline_exceeded" else 499,
                stop_code or "cancelled",
                "Image generation stopped",
            ) from None
        if stop_code:
            raise failure(
                504 if stop_code == "deadline_exceeded" else 499,
                stop_code,
                "Image generation stopped",
            )
        output = io.BytesIO()
        image.save(output, format="PNG")
        content = output.getvalue()
        if image.size != (width, height) or len(content) > MAX_PNG_BYTES:
            raise failure(502, "invalid_backend_response", "Runtime returned an invalid image")
        return {
            "png_base64": base64.b64encode(content).decode("ascii"),
            "seed": seed,
            "steps": adapter.steps,
            "guidance": adapter.guidance,
            "memory_policy": adapter.memory_policy,
            "duration_seconds": round(time.monotonic() - started, 3),
        }
    finally:
        cancel.set()
        # Cancelling an asyncio task does not stop its executor thread. Keep the
        # caller's GPU lease until the worker confirms it has actually stopped.
        while not worker.done():
            try:
                await asyncio.shield(worker)
            except asyncio.CancelledError:
                continue
            except Exception:
                break
        if worker.done() and not worker.cancelled():
            worker.exception()  # retrieve a failure after transport cancellation


@router.post("/images/generate")
async def generate_image(payload: ImageRequest, request: Request):
    try:
        async with request.app.state.model_manager.image_lease(payload.model_id) as adapter:
            return await owned_generation(adapter, payload, request)
    except HTTPException:
        raise
    except KeyError:
        raise failure(503, "model_unavailable", "Load the image model in Pumas first") from None
    except ValueError:
        raise failure(
            400, "unsupported_model", "Selected model does not support this operation"
        ) from None
    except torch.cuda.OutOfMemoryError:
        raise failure(
            507, "out_of_memory", "Insufficient GPU memory; free memory before retrying"
        ) from None
    except RuntimeError as error:
        if str(error) == "Image runtime is busy":
            raise failure(409, "runtime_busy", "Image runtime is busy") from None
        raise failure(
            502, "backend_failure", "Image runtime failed; inspect the Pumas runtime log"
        ) from None
