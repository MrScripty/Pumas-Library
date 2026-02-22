"""DLLM (Dynamic Large Language Model) loader.

Provides specialized loading for DLLM architectures that may use
dynamic routing or mixture-of-experts patterns.
"""

import logging
from pathlib import Path
from typing import Any

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

logger = logging.getLogger(__name__)


def load_dllm(
    model_path: Path,
    device: torch.device,
) -> tuple[Any, Any, str]:
    """Load a DLLM model.

    DLLM models use standard HuggingFace format but may require
    specific loading parameters for dynamic architecture support.

    Returns:
        (model, tokenizer, model_type)
    """
    path_str = str(model_path)

    logger.info("Loading DLLM model from %s onto %s", path_str, device)

    tokenizer = AutoTokenizer.from_pretrained(path_str, trust_remote_code=True)

    model = AutoModelForCausalLM.from_pretrained(
        path_str,
        torch_dtype="auto",
        device_map=str(device),
        trust_remote_code=True,
        low_cpu_mem_usage=True,
    )

    model.eval()

    logger.info("DLLM model loaded: %s", model_path.name)
    return model, tokenizer, "dllm"
