"""Cohere ASR (``CohereAsrForConditionalGeneration``) for the Torch sidecar.

Loads an installed package with Transformers' native implementation (5.4 or
newer), from local files only and without repository code, and transcribes
one bounded 16 kHz mono recording per call. The caller holds the slot's device
lease (``ModelManager.audio_lease``) until the worker returns.
"""

import json
from pathlib import Path
from threading import Event
from typing import Any

from audio_contract import LANGUAGES, MAX_AUDIO_SAMPLES, MAX_TEXT_BYTES, SAMPLE_RATE

COHERE_ASR = "cohere-asr"
MAX_NEW_TOKENS = 512
_MAX_CONFIG_BYTES = 1024 * 1024


class SpeechRuntimeUnsupported(RuntimeError):
    """The installed runtime lacks Transformers' native Cohere ASR support."""


class SpeechCancelled(RuntimeError):
    """Transcription stopped after cooperative cancellation."""


def _supported_device(device: Any) -> str:
    kind = getattr(device, "type", str(device).split(":", 1)[0])
    if kind not in ("cpu", "cuda"):
        raise SpeechRuntimeUnsupported("Cohere ASR runs on CPU or CUDA")
    return kind


def _native_api():
    # Lazy imports keep text and image work independent of Transformers 5.
    try:
        from transformers import (
            AutoProcessor,
            CohereAsrForConditionalGeneration,
            StoppingCriteria,
            StoppingCriteriaList,
        )
    except ImportError as error:
        raise SpeechRuntimeUnsupported(
            "Cohere ASR needs a Torch runtime installed with the cohere-asr adapter "
            "(Transformers >=5.4)"
        ) from error
    return AutoProcessor, CohereAsrForConditionalGeneration, StoppingCriteria, StoppingCriteriaList


def validate_installed_package(model_path: Path) -> Path:
    """Refuse a path that is not a local Cohere ASR package, with its reason."""
    root = Path(model_path).resolve(strict=True)
    if not root.is_dir():
        raise ValueError("Cohere ASR requires an installed package directory")
    config_path = root / "config.json"
    if not config_path.is_file() or not (root / "model.safetensors").is_file():
        raise ValueError("Cohere ASR requires config.json and model.safetensors")
    with config_path.open("rb") as reader:
        raw = reader.read(_MAX_CONFIG_BYTES + 1)
    try:
        config = json.loads(raw) if len(raw) <= _MAX_CONFIG_BYTES else None
    except ValueError:
        config = None
    if not isinstance(config, dict) or config.get("model_type") != "cohere_asr":
        raise ValueError("Installed package is not a Cohere ASR model")
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


def _eos_tokens(model) -> frozenset:
    eos = getattr(getattr(model, "generation_config", None), "eos_token_id", None)
    if eos is None:
        return frozenset()
    return frozenset([eos] if isinstance(eos, int) else eos)


def transcribe(model, processor, pcm16le: bytes, language: str, cancel: Event) -> tuple[str, str]:
    """Transcribe one recording; return ``(text, finish_reason)``.

    ``finish_reason`` is ``"stop"`` when generation ended on an end-of-sequence
    token and ``"length"`` when it reached ``MAX_NEW_TOKENS``.
    """
    _check_cancel(cancel)
    _supported_device(model.device)
    if (
        type(pcm16le) is not bytes
        or not pcm16le
        or len(pcm16le) % 2
        or len(pcm16le) > MAX_AUDIO_SAMPLES * 2
    ):
        raise ValueError("Audio must be 1-480000 mono PCM16LE samples at 16 kHz")
    if type(language) is not str or language not in LANGUAGES:
        raise ValueError(f"language must be one of: {', '.join(sorted(LANGUAGES))}")
    _, _, stopping_base, stopping_list = _native_api()
    import numpy as np
    import torch

    class CancelCriterion(stopping_base):
        def __call__(self, input_ids, scores, **kwargs):
            return cancel.is_set()

    audio = np.frombuffer(pcm16le, dtype="<i2").astype(np.float32) / 32768.0
    inputs = processor(audio, sampling_rate=SAMPLE_RATE, return_tensors="pt", language=language)
    _check_cancel(cancel)
    inputs = inputs.to(model.device, dtype=model.dtype)
    # Chunking metadata the processor returns; generate() does not accept it.
    inputs = {key: value for key, value in inputs.items() if key != "audio_chunk_index"}
    with torch.inference_mode():
        output = model.generate(
            **inputs,
            max_new_tokens=MAX_NEW_TOKENS,
            stopping_criteria=stopping_list([CancelCriterion()]),
        )
    _check_cancel(cancel)
    if model.device.type == "cuda":
        torch.cuda.synchronize(model.device)
    eos = _eos_tokens(model)
    sequences = output.tolist() if hasattr(output, "tolist") else list(output)
    finish_reason = (
        "stop" if all(sequence and sequence[-1] in eos for sequence in sequences) else "length"
    )
    texts = processor.batch_decode(output, skip_special_tokens=True)
    text = " ".join(part.strip() for part in texts if isinstance(part, str)).strip()
    if len(text.encode("utf-8")) > MAX_TEXT_BYTES:
        raise ValueError("ASR runtime returned an oversized transcript")
    return text, finish_reason
