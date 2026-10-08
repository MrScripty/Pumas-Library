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
MAX_DESCRIPTOR_BYTES = 65536
_REDIRECT_KEYS = frozenset(
    {
        "custom_pipelines",
        "audio_tokenizer",
        "audio_tokenizer_name_or_path",
        "tokenizer_file",
        "vocab_file",
        "merges_file",
        "spm_file",
        "fast_tokenizer_files",
    }
)
_NATIVE_CLASSES = {
    "processor_class": {"CohereAsrProcessor"},
    "feature_extractor_class": {"CohereAsrFeatureExtractor"},
    "feature_extractor_type": {"CohereAsrFeatureExtractor"},
    "tokenizer_class": {"TokenizersBackend", "PreTrainedTokenizerFast", "CohereAsrTokenizer"},
}

# Original public metadata aliases are inert: only installed classes below run.
_ORIGINAL_AUTO_MAP = {
    "AutoConfig": "configuration_cohere_asr.CohereAsrConfig",
    "AutoFeatureExtractor": "processing_cohere_asr.CohereAsrFeatureExtractor",
    "AutoModel": "modeling_cohere_asr.CohereAsrModel",
    "AutoModelForSpeechSeq2Seq": "modeling_cohere_asr.CohereAsrForConditionalGeneration",
    "AutoProcessor": "processing_cohere_asr.CohereAsrProcessor",
    "AutoTokenizer": "tokenization_cohere_asr.CohereAsrTokenizer",
}
_MAP_ROLES = {
    "config.json": frozenset(_ORIGINAL_AUTO_MAP),
    "preprocessor_config.json": {"AutoFeatureExtractor"},
    "processor_config.json": {"AutoProcessor"},
    "tokenizer_config.json": {"AutoTokenizer"},
}


class _NativeProcessor:
    @staticmethod
    def from_pretrained(root, *, local_files_only, trust_remote_code):
        try:
            from transformers import (
                CohereAsrFeatureExtractor,
                CohereAsrProcessor,
                TokenizersBackend,
            )
        except ImportError as error:
            raise SpeechRuntimeUnsupported(
                "Installed native Cohere processor classes are required"
            ) from error
        # No Auto factory observes aliases/auto_map; descriptors stay unchanged.
        feature_extractor = CohereAsrFeatureExtractor.from_pretrained(
            root, local_files_only=local_files_only, trust_remote_code=trust_remote_code
        )
        tokenizer = TokenizersBackend.from_pretrained(
            root, local_files_only=local_files_only, trust_remote_code=trust_remote_code
        )
        return CohereAsrProcessor(feature_extractor=feature_extractor, tokenizer=tokenizer)


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
            CohereAsrForConditionalGeneration,
            StoppingCriteria,
            StoppingCriteriaList,
        )
    except ImportError as error:
        raise SpeechRuntimeUnsupported(
            "A qualified native Cohere ASR runtime (Transformers >=5.4) is required"
        ) from error
    return (
        _NativeProcessor,
        CohereAsrForConditionalGeneration,
        StoppingCriteria,
        StoppingCriteriaList,
    )


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
    for file in (
        config_path,
        weights,
        root / "tokenizer.json",
        root / "preprocessor_config.json",
        root / "tokenizer_config.json",
    ):
        if file.is_symlink() or not file.is_file() or not file.resolve().is_relative_to(root):
            raise ValueError("Cohere ASR requires local config and safetensors weights")
    config = _descriptor(config_path)
    if (
        not isinstance(config, dict)
        or config.get("model_type") != "cohere_asr"
        or config.get("architectures") != ["CohereAsrForConditionalGeneration"]
    ):
        raise ValueError("Installed package is not native Cohere ASR")
    for name in ("preprocessor_config.json", "tokenizer_config.json", "processor_config.json"):
        file = root / name
        if file.exists() or file.is_symlink():
            _descriptor(file)
    return root


def _descriptor(file):
    # local_files_only prevents downloads, but these native descriptor fields
    # can select secondary local/cache paths. No such read set is qualified.
    if file.is_symlink() or not file.is_file():
        raise ValueError("Native ASR descriptor must be a local regular file")
    with file.open("rb") as reader:
        raw = reader.read(MAX_DESCRIPTOR_BYTES + 1)
    if len(raw) > MAX_DESCRIPTOR_BYTES:
        raise ValueError("Native ASR descriptor is too large")
    try:
        value = json.loads(raw)
    except (ValueError, RecursionError) as error:
        raise ValueError("Native ASR descriptor is invalid") from error
    if type(value) is not dict:
        raise ValueError("Native ASR descriptor must be an object")
    pending = [value]
    while pending:
        item = pending.pop()
        if type(item) is dict:
            if _REDIRECT_KEYS.intersection(item):
                raise ValueError("Native ASR descriptor redirects are unsupported")
            if "auto_map" in item:
                mapping = item["auto_map"]
                if (
                    item is not value
                    or type(mapping) is not dict
                    or not mapping
                    or any(
                        key not in _MAP_ROLES.get(file.name, ())
                        or type(alias) is not str
                        or alias != _ORIGINAL_AUTO_MAP.get(key)
                        for key, alias in mapping.items()
                    )
                ):
                    raise ValueError("Native ASR descriptor redirects are unsupported")
            # TokenizersBackend also accepts serialized constructor overrides.
            if any(type(item.get(key)) is str for key in ("vocab", "merges")):
                raise ValueError("Native ASR descriptor redirects are unsupported")
            for key, selected in item.items():
                if key in _NATIVE_CLASSES:
                    if type(selected) is not str or selected not in _NATIVE_CLASSES[key]:
                        raise ValueError("Native ASR descriptor class is unsupported")
                elif key.endswith("_class") or key.endswith("_processor_type"):
                    raise ValueError("Native ASR descriptor class is unsupported")
            pending.extend(item.values())
        elif type(item) is list:
            pending.extend(item)
    return value


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
    start = getattr(config, "decoder_start_token_id", None)
    if start is None:
        start = getattr(config, "bos_token_id", None)
    start = _tokens([start])[0]
    prefix = inputs.get("decoder_input_ids")
    if prefix is None:
        prefix = [start]
    else:
        if hasattr(prefix, "tolist"):
            prefix = prefix.tolist()
        if type(prefix) is not list or len(prefix) != 1:
            raise SpeechRuntimeUnsupported("Native decoder prompt evidence is unsupported")
        prefix = _tokens(prefix[0])
        # v5.4 GenerationMixin prepends the decoder start (or BOS fallback)
        # for this single-sample encoder-decoder when the prompt lacks it.
        if prefix[0] != start:
            prefix = [start, *prefix]
    return prefix, eos


def _single_chunk(inputs):
    # The native v5.4 processor returns this non-model metadata. Forwarding it
    # to GenerationMixin fails argument validation; ignoring multiple chunks
    # would discard part of the caller's recording.
    chunks = inputs.get("audio_chunk_index")
    if (
        type(chunks) is not list
        or len(chunks) != 1
        or type(chunks[0]) not in (list, tuple)
        or len(chunks[0]) != 2
        or type(chunks[0][0]) is not int
        or chunks[0][0] != 0
        or not (chunks[0][1] is None or (type(chunks[0][1]) is int and chunks[0][1] == 0))
    ):
        raise SpeechRuntimeUnsupported("Native single-sample chunk evidence is unsupported")


def _finite_features(inputs, torch):
    features = inputs.get("input_features")
    if (
        not torch.is_tensor(features)
        or not torch.is_floating_point(features)
        or len(features.shape) != 3
        or features.shape[0] != 1
        or any(size <= 0 for size in features.shape)
        or not torch.isfinite(features).all().item()
    ):
        raise SpeechRuntimeUnsupported("Native audio features are unsupported or nonfinite")


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
        _single_chunk(inputs)
        _finite_features(inputs, torch)
        inputs = inputs.to(model.device, dtype=model.dtype)
        _finite_features(inputs, torch)
        inputs = {key: value for key, value in inputs.items() if key != "audio_chunk_index"}
        contract = _generation_contract(model, inputs) if detailed else None
        _check_cancel(cancel)
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
