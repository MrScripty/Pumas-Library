"""Private audio transcription operation for the Pumas gateway.

Mounted under ``/api`` beside the image operation. The request names the
loaded ``model_id`` and carries one PCM recording as the typed model-operations
contract describes it; the sidecar normalizes it to 16 kHz mono and returns the
transcript with its finish reason.

As with image generation, a disconnect requests worker cancellation, but the
device lease stays held until the worker thread has actually stopped.
"""

import asyncio
import logging
import threading
from contextlib import AsyncExitStack

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field

from audio_contract import LANGUAGES
from audio_input import AudioInputError, normalize_audio

logger = logging.getLogger(__name__)

router = APIRouter()


class TranscriptionRequest(BaseModel):
    model_config = ConfigDict(extra="forbid", strict=True)
    model_id: str = Field(min_length=1, max_length=256)
    audio: dict
    language: str = Field(default="en", min_length=2, max_length=8)


def failure(status: int, code: str, message: str):
    """A refusal with a stable code and a caller-safe reason."""
    return HTTPException(status_code=status, detail={"code": code, "message": message})


def _defer_current_cancellation() -> None:
    task = asyncio.current_task()
    if task is not None:
        task.uncancel()


async def owned_transcription(loaded, pcm16le: bytes, language: str, request: Request):
    """Run one transcription, holding the caller's lease until the worker stops."""
    from loaders.cohere_asr_loader import SpeechCancelled, transcribe

    cancel = threading.Event()
    worker = asyncio.create_task(
        asyncio.to_thread(transcribe, loaded.model, loaded.tokenizer, pcm16le, language, cancel)
    )
    owner_cancellation = None
    try:
        while not worker.done():
            if await request.is_disconnected():
                cancel.set()
            # Poll the worker: awaiting it would let request cancellation mark
            # the wrapper done while its executor thread kept running.
            await asyncio.sleep(0.05)
        try:
            text, finish_reason = worker.result()
        except SpeechCancelled:
            raise failure(499, "cancelled", "Transcription stopped") from None
        if cancel.is_set():
            raise failure(499, "cancelled", "Transcription stopped")
        return {"text": text, "finish_reason": finish_reason}
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


@router.post("/audio/transcriptions")
async def transcribe_audio(payload: TranscriptionRequest, request: Request):
    from model_manager import SlotConflict

    if payload.language not in LANGUAGES:
        raise failure(
            400,
            "invalid_request",
            f"language must be one of: {', '.join(sorted(LANGUAGES))}",
        )
    try:
        normalized = normalize_audio(payload.audio)
    except AudioInputError as error:
        raise failure(400, "invalid_request", str(error)) from None
    import base64

    pcm16le = base64.b64decode(normalized["data_base64"])
    async with AsyncExitStack() as lease:
        try:
            loaded = await lease.enter_async_context(
                request.app.state.model_manager.audio_lease(payload.model_id)
            )
        except KeyError:
            raise failure(503, "model_unavailable", "Load the audio model in Pumas first") from None
        except ValueError:
            raise failure(
                400, "unsupported_model", "Selected model does not support audio transcription"
            ) from None
        except SlotConflict as error:
            raise failure(409, error.code, str(error)) from None
        try:
            return await owned_transcription(loaded, pcm16le, payload.language, request)
        except HTTPException:
            raise
        except torch.cuda.OutOfMemoryError as error:
            logger.warning("Transcription ran out of GPU memory", exc_info=True)
            raise failure(
                507, "out_of_memory", "Insufficient GPU memory; free memory before retrying"
            ) from error
        except ValueError as error:
            raise failure(400, "invalid_request", str(error)) from error
        except Exception as error:
            logger.exception("Transcription failed")
            raise failure(
                502, "backend_failure", "Audio runtime failed; inspect the Pumas runtime log"
            ) from error
