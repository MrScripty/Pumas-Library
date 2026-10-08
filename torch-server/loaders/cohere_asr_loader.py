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

from audio_contract import LANGUAGES, MAX_AUDIO_SAMPLES, MAX_TEXT_BYTES, SAMPLE_RATE
from native_speech_result import NativeSpeechResult

COHERE_ASR = "cohere-asr"


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
    return _transcribe(model, processor, pcm16le, language, cancel, detailed=False)


def transcribe_detailed(model, processor, pcm16le, language, cancel) -> NativeSpeechResult:
    """Require concrete EOS/prompt/bound evidence; never infer stop from text.

    This projection is not installed-runtime or read-set qualification. The
    production gate remains closed. Unsupported decoder/prompt configuration,
    forced EOS, ambiguous or shortened output refuses a successful result.
    """
    return _transcribe(model, processor, pcm16le, language, cancel, detailed=True)


def _tokens(value):
    if hasattr(value, "tolist"):
        value = value.tolist()
    if (
        type(value) is not list
        or not value
        or any(type(token) is not int or token < 0 for token in value)
    ):
        raise SpeechRuntimeUnsupported("Native generation token evidence is unsupported")
    return value


def _generation_contract(model, inputs):
    config = getattr(model, "generation_config", None)
    if (
        getattr(getattr(model, "config", None), "is_encoder_decoder", None) is not True
        or config is None
        or getattr(config, "forced_eos_token_id", None) is not None
    ):
        raise SpeechRuntimeUnsupported("Native generation terminal semantics are unqualified")
    eos = getattr(config, "eos_token_id", None)
    if type(eos) is int:
        eos = [eos]
    eos = frozenset(_tokens(eos))
    prefix = inputs.get("decoder_input_ids")
    if prefix is None:
        prefix = _tokens([getattr(config, "decoder_start_token_id", None)])
    else:
        if hasattr(prefix, "tolist"):
            prefix = prefix.tolist()
        if type(prefix) is not list or len(prefix) != 1:
            raise SpeechRuntimeUnsupported("Native decoder prompt evidence is unsupported")
        prefix = _tokens(prefix[0])
    return prefix, eos


def _finish_reason(sequence, prefix, eos):
    tokens = _tokens(sequence)
    if tokens[: len(prefix)] != prefix:
        raise SpeechRuntimeUnsupported("Native decoder prompt lineage is unsupported")
    generated = tokens[len(prefix) :]
    if not generated or len(generated) > 512 or any(token in eos for token in generated[:-1]):
        raise SpeechRuntimeUnsupported("Native generation terminal evidence is incoherent")
    # At the explicit token bound report length, including a terminal token at
    # that bound; do not claim natural completion from a forced boundary token.
    if len(generated) == 512:
        return "length"
    if generated[-1] in eos:
        return "stop"
    raise SpeechRuntimeUnsupported("Native generation omitted terminal evidence")


def _transcribe(model, processor, pcm16le, language, cancel, *, detailed):
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
        contract = _generation_contract(model, inputs) if detailed else None
        with torch.inference_mode():
            output = model.generate(
                **inputs, max_new_tokens=512, stopping_criteria=stopping_list([CancelCriterion()])
            )
        _check_cancel(cancel)
        if len(output) != 1:
            raise ValueError("ASR runtime returned an unexpected batch")
        finish_reason = _finish_reason(output[0], *contract) if detailed else None
        text = processor.decode(output[0], skip_special_tokens=True)
        _check_cancel(cancel)
        if not isinstance(text, str) or len(text.encode("utf-8")) > MAX_TEXT_BYTES:
            raise ValueError("ASR runtime returned an invalid or oversized transcript")
        return NativeSpeechResult(text.strip(), finish_reason) if detailed else text.strip()
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
