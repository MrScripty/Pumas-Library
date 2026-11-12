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

    _require_safetensors_weights(path)
    _refuse_unexecuted_custom_code(path)
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


def _auto_map_modules(auto_map: dict) -> list[str]:
    """Code files an ``auto_map`` names: ``module.Class`` or ``org/repo--module.Class``."""
    references = []
    for value in auto_map.values():
        values = value if isinstance(value, list) else [value]
        references.extend(reference for reference in values if isinstance(reference, str))
    modules = set()
    for reference in references:
        local = reference.split("--", 1)[-1]
        module = local.rsplit(".", 1)[0]
        if module and module != local:
            modules.add(f"{module.replace('.', '/')}.py")
    return sorted(modules)


def _refuse_unexecuted_custom_code(path: Path) -> None:
    """Say so when a model needs repository code that Pumas never executes.

    Pumas stores a model's ``*.py`` files for inspection only, and loaders keep
    ``trust_remote_code=False``. A config whose ``auto_map`` names code for a
    ``model_type`` this transformers build does not implement cannot load, so
    the caller gets that reason instead of an opaque loader failure. Models whose
    architecture transformers implements natively still load: transformers
    ignores ``auto_map`` when remote code is not trusted.
    """
    from validation import ModelLoadRefusal

    config_path = path / "config.json" if path.is_dir() else path.parent / "config.json"
    try:
        with open(config_path) as f:
            config = json.load(f)
    except (OSError, json.JSONDecodeError):
        return
    auto_map = config.get("auto_map") if isinstance(config, dict) else None
    if not isinstance(auto_map, dict) or not auto_map:
        return
    model_type = config.get("model_type")
    try:
        from transformers import CONFIG_MAPPING
    except ImportError:
        return
    if isinstance(model_type, str) and model_type in CONFIG_MAPPING:
        return
    modules = ", ".join(_auto_map_modules(auto_map)) or "its auto_map code"
    raise ModelLoadRefusal(
        f"This model's config.json auto_map requires custom code ({modules}) for "
        f"model_type {model_type!r}, which this transformers build does not implement. "
        "Pumas stores that code with the model for inspection but never executes it."
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
