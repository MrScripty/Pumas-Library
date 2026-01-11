"""HuggingFace API client wrapper for model library."""

from __future__ import annotations

import os
from typing import TYPE_CHECKING

from backend.logging_config import get_logger

if TYPE_CHECKING:
    from huggingface_hub import HfApi

logger = get_logger(__name__)


class HfClient:
    """Wrapper for HuggingFace Hub API client with authentication."""

    def __init__(self, token: str | None = None) -> None:
        """Initialize the HuggingFace client.

        Args:
            token: Optional HuggingFace API token. If not provided, will check HF_TOKEN env var.
        """
        self.hf_token = token or os.getenv("HF_TOKEN")
        self._api: HfApi | None = None

    def get_api(self) -> HfApi:
        """Get or create the HuggingFace API instance.

        Returns:
            Initialized HfApi instance

        Raises:
            RuntimeError: If huggingface_hub is not installed
        """
        if self._api:
            return self._api

        try:
            from huggingface_hub import HfApi, login
        except ImportError as exc:
            raise RuntimeError("huggingface_hub is not installed") from exc

        if self.hf_token:
            login(self.hf_token)

        self._api = HfApi()
        return self._api
