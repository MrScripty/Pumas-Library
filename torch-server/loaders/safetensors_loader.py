"""Standard HuggingFace model loader using AutoModelForCausalLM."""

import logging
from pathlib import Path
from typing import Any

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

logger = logging.getLogger(__name__)


def load_safetensors(
    model_path: Path,
    device: torch.device,
) -> tuple[Any, Any, str]:
    """Load a standard HuggingFace model from safetensors format.

    Returns:
        (model, tokenizer, model_type)
    """
    path_str = str(model_path)

    logger.info("Loading model from %s onto %s", path_str, device)

    tokenizer = AutoTokenizer.from_pretrained(path_str, trust_remote_code=True)

    model = AutoModelForCausalLM.from_pretrained(
        path_str,
        torch_dtype="auto",
        device_map=str(device),
        trust_remote_code=True,
    )

    model.eval()

    logger.info("Model loaded: %s", model_path.name)
    return model, tokenizer, "text-generation"
