"""Model loaders for the Torch inference server.

Supports standard HuggingFace models, DLLM, and Sherry formats.
"""

import json
import logging
from pathlib import Path
from typing import Any, Optional

import torch

logger = logging.getLogger(__name__)


def load_model(
    model_path: str,
    device: torch.device,
    model_type: Optional[str] = None,
    declarations: Optional[dict] = None,
) -> tuple[Any, Any, str]:
    """Load a model from a path, auto-detecting the format.

    Returns:
        (model, tokenizer, detected_type)
    """
    path = Path(model_path)

    # Detect model type from config if not specified
    if model_type is None:
        model_type = _detect_model_type(path)

    if model_type == "cohere-asr":
        from .cohere_asr_loader import load_cohere_asr

        return load_cohere_asr(path, device)

    from .text_encoder_loader import TEXT_ENCODER_TYPES, load_text_encoder

    if model_type in TEXT_ENCODER_TYPES:
        return load_text_encoder(path, device, model_type, declarations)
    if declarations is not None:
        from validation import ModelLoadRefusal

        raise ModelLoadRefusal("declarations apply only to text embedding and rerank models")

    _require_safetensors_weights(path)
    if model_type == "dllm":
        from .dllm_loader import load_dllm

        return load_dllm(path, device)
    elif model_type == "sherry":
        from .sherry_loader import load_sherry

        return load_sherry(path, device)
    else:
        from .safetensors_loader import load_safetensors

        return load_safetensors(path, device)


def _require_safetensors_weights(path: Path) -> None:
    """Text loaders read safetensors only; say so before transformers fails opaquely."""
    # Imported here: the owned audio worker imports this package with a code
    # root that does not ship the HTTP sidecar's validation module.
    from validation import ModelLoadRefusal

    weights = [path] if path.is_file() else list(path.glob("*.safetensors"))
    if not any(candidate.suffix == ".safetensors" for candidate in weights):
        raise ModelLoadRefusal(
            "Torch text models must provide .safetensors weights; "
            "pickle checkpoints (.bin, .pt) are not loaded"
        )


def _detect_model_type(path: Path) -> str:
    """Detect model type from config.json or directory contents."""
    config_path = path / "config.json" if path.is_dir() else path.parent / "config.json"

    if config_path.exists():
        try:
            with open(config_path) as f:
                config = json.load(f)

            architectures = config.get("architectures", [])
            model_type_field = config.get("model_type", "")

            if model_type_field == "cohere_asr":
                return "cohere-asr"

            # Check for DLLM markers
            if any("dllm" in arch.lower() for arch in architectures):
                return "dllm"
            if "dllm" in model_type_field.lower():
                return "dllm"

            # Check for Sherry markers
            if any("sherry" in arch.lower() for arch in architectures):
                return "sherry"
            if "sherry" in model_type_field.lower():
                return "sherry"

        except (json.JSONDecodeError, OSError) as e:
            logger.warning("Failed to read config.json: %s", e)

    return "text-generation"
