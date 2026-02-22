"""Sherry model loader.

Provides specialized loading for Sherry models that may use
quantization-aware training (QAT) or custom quantization formats.
"""

import logging
from pathlib import Path
from typing import Any

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

logger = logging.getLogger(__name__)


def load_sherry(
    model_path: Path,
    device: torch.device,
) -> tuple[Any, Any, str]:
    """Load a Sherry model.

    Sherry models may use QAT-specific quantization that requires
    custom loading parameters.

    Returns:
        (model, tokenizer, model_type)
    """
    path_str = str(model_path)

    logger.info("Loading Sherry model from %s onto %s", path_str, device)

    tokenizer = AutoTokenizer.from_pretrained(path_str, trust_remote_code=True)

    model = AutoModelForCausalLM.from_pretrained(
        path_str,
        torch_dtype="auto",
        device_map=str(device),
        trust_remote_code=True,
        low_cpu_mem_usage=True,
    )

    model.eval()

    logger.info("Sherry model loaded: %s", model_path.name)
    return model, tokenizer, "sherry"
