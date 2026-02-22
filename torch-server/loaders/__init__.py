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

    if model_type == "dllm":
        from .dllm_loader import load_dllm
        return load_dllm(path, device)
    elif model_type == "sherry":
        from .sherry_loader import load_sherry
        return load_sherry(path, device)
    else:
        from .safetensors_loader import load_safetensors
        return load_safetensors(path, device)


def _detect_model_type(path: Path) -> str:
    """Detect model type from config.json or directory contents."""
    config_path = path / "config.json" if path.is_dir() else path.parent / "config.json"

    if config_path.exists():
        try:
            with open(config_path) as f:
                config = json.load(f)

            architectures = config.get("architectures", [])
            model_type_field = config.get("model_type", "")

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
