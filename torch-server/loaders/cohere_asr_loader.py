"""Native, installed-only Cohere ASR adapter; lifecycle remains caller-owned.

This module does not advertise a route or start a worker. Its caller must hold
ModelManager.speech_lease until confirmed worker/device cessation, even after
cancellation. SpeechCleanupUnconfirmed does not authorize lease release; the
owner must fence the device until runtime/process cleanup resolves it.
"""

import json
import sys
from pathlib import Path
from threading import Event
from typing import Any

COHERE_ASR = "cohere-asr"
SAMPLE_RATE = 16000
MAX_AUDIO_SAMPLES = SAMPLE_RATE * 30
MAX_TEXT_BYTES = 16000
LANGUAGES = frozenset("en de fr it es pt el nl pl vi zh ar ja ko".split())


class SpeechRuntimeUnsupported(RuntimeError):
    """The installed runtime lacks the required native ASR implementation."""


class SpeechCancelled(RuntimeError):
    """Synchronous inference has returned after cooperative cancellation."""


class SpeechCleanupUnconfirmed(RuntimeError):
    """The operation owner must fence the device until runtime cleanup resolves it."""

    def __init__(self, original_error, cleanup_error, retained_audio):
        super().__init__("ASR device completion is unconfirmed; runtime cleanup is required")
        self.original_error = original_error
        self.cleanup_error = cleanup_error
        # The wrapper owns this buffer until device/process cessation is proven.
        self.retained_audio = retained_audio


def _supported_device(device: Any) -> str:
    kind = getattr(device, "type", str(device).split(":", 1)[0])
    if kind not in ("cpu", "cuda"):
        raise SpeechRuntimeUnsupported("Initial Cohere ASR adapter supports CPU and CUDA only")
    return kind


def _native_api():
    # Lazy imports preserve text/image workloads on the existing 4.x runtime.
    try:
        from transformers import (
            AutoProcessor,
            CohereAsrForConditionalGeneration,
            StoppingCriteria,
            StoppingCriteriaList,
        )
    except ImportError as error:
        raise SpeechRuntimeUnsupported(
            "A qualified native Cohere ASR runtime (Transformers >=5.4) is required"
        ) from error
    return AutoProcessor, CohereAsrForConditionalGeneration, StoppingCriteria, StoppingCriteriaList


def validate_installed_package(model_path: Path) -> Path:
    """Reject locators and incompatible packages before any library loading.

    The Pumas library owns repository/revision/digest verification and custody;
    this adapter additionally checks the installed native architecture.
    """
    root = model_path.resolve(strict=True)
    if not root.is_dir():
        raise ValueError("Cohere ASR requires an installed package directory")
    config_path = root / "config.json"
    weights = root / "model.safetensors"
    for file in (config_path, weights):
        if not file.is_file() or not file.resolve().is_relative_to(root):
            raise ValueError("Cohere ASR requires local config and safetensors weights")
    if config_path.stat().st_size > 65536:
        raise ValueError("Cohere ASR configuration is too large")
    config = json.loads(config_path.read_text(encoding="utf-8"))
    if (
        not isinstance(config, dict)
        or config.get("model_type") != "cohere_asr"
        or config.get("architectures") != ["CohereAsrForConditionalGeneration"]
    ):
        raise ValueError("Installed package is not native Cohere ASR")
    return root


def load_cohere_asr(model_path: Path, device: Any) -> tuple[Any, Any, str]:
    _supported_device(device)
    root = validate_installed_package(model_path)
    processor_class, model_class, _, _ = _native_api()
    processor = processor_class.from_pretrained(
        str(root), local_files_only=True, trust_remote_code=False
    )
    model = model_class.from_pretrained(
        str(root),
        local_files_only=True,
        trust_remote_code=False,
        use_safetensors=True,
        device_map=str(device),
    )
    model.eval()
    return model, processor, COHERE_ASR


def _check_cancel(cancel: Event) -> None:
    if cancel.is_set():
        raise SpeechCancelled("Speech inference cancelled")


def transcribe(model: Any, processor: Any, pcm16le: bytes, language: str, cancel: Event) -> str:
    """Transcribe one bounded mono 16 kHz recording without creating a thread.

    Encoder/preprocessing may not be interruptible. The caller retains custody
    until confirmed cessation; setting the event alone never releases it.
    SpeechCleanupUnconfirmed requires retained/fenced device custody even though
    this function has raised: resolve runtime/process cleanup before release.
    """
    _check_cancel(cancel)
    device_kind = _supported_device(model.device)
    if (
        type(pcm16le) is not bytes
        or not pcm16le
        or len(pcm16le) % 2
        or len(pcm16le) > MAX_AUDIO_SAMPLES * 2
    ):
        raise ValueError("Audio must be 1–480000 mono PCM16LE samples at 16 kHz")
    if type(language) is not str or language not in LANGUAGES:
        raise ValueError("An explicitly supported transcription language is required")
    _, _, stopping_base, stopping_list = _native_api()
    import numpy as np
    import torch

    class CancelCriterion(stopping_base):
        def __call__(self, input_ids, scores, **kwargs):
            return cancel.is_set()

    audio = np.frombuffer(pcm16le, dtype="<i2").astype(np.float32) / 32768.0
    try:
        inputs = processor(audio, sampling_rate=SAMPLE_RATE, return_tensors="pt", language=language)
        _check_cancel(cancel)
        inputs = inputs.to(model.device, dtype=model.dtype)
        with torch.inference_mode():
            output = model.generate(
                **inputs, max_new_tokens=512, stopping_criteria=stopping_list([CancelCriterion()])
            )
        _check_cancel(cancel)
        if len(output) != 1:
            raise ValueError("ASR runtime returned an unexpected batch")
        text = processor.decode(output[0], skip_special_tokens=True)
        _check_cancel(cancel)
        if not isinstance(text, str) or len(text.encode("utf-8")) > MAX_TEXT_BYTES:
            raise ValueError("ASR runtime returned an invalid or oversized transcript")
        return text.strip()
    finally:
        original_error = sys.exc_info()[1]
        if device_kind == "cuda":
            try:
                torch.cuda.synchronize(model.device)
            except Exception as cleanup_error:
                raise SpeechCleanupUnconfirmed(
                    original_error, cleanup_error, audio
                ) from cleanup_error
        audio.fill(0)
