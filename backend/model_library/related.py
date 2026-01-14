"""Utilities for resolving related HuggingFace models from metadata."""

from __future__ import annotations

from typing import Any, Iterable
from urllib.parse import urlparse

from backend.model_library.naming import normalize_name


def _iter_candidates(value: Any) -> Iterable[Any]:
    """Yield possible repo id candidates from nested metadata values."""
    if value is None:
        return []
    if isinstance(value, list):
        return value
    return [value]


def _parse_repo_id_from_string(value: str) -> str | None:
    cleaned = value.strip()
    if not cleaned:
        return None

    if "huggingface.co" in cleaned:
        parsed = urlparse(cleaned)
        path = parsed.path.strip("/")
        if path:
            parts = path.split("/")
            if len(parts) >= 2:
                return f"{parts[0]}/{parts[1]}"

    if "/" in cleaned:
        parts = cleaned.strip("/").split("/")
        if len(parts) >= 2:
            return f"{parts[0]}/{parts[1]}"
    return None


def extract_repo_id(value: Any) -> str | None:
    """Extract a repo id from a base model value or URL-like payload."""
    for candidate in _iter_candidates(value):
        if isinstance(candidate, str):
            repo_id = _parse_repo_id_from_string(candidate)
            if repo_id:
                return repo_id
        elif isinstance(candidate, dict):
            for key in ("repo_id", "repoId", "model_id", "modelId", "id", "name", "model"):
                nested = candidate.get(key)
                repo_id = extract_repo_id(nested)
                if repo_id:
                    return repo_id
    return None


def extract_base_model_repo_id(metadata: dict[str, Any]) -> str | None:
    """Extract a base model repo id from model metadata or fallback URL."""
    base_model = metadata.get("base_model")
    if not base_model:
        card_data = metadata.get("model_card")
        if isinstance(card_data, dict):
            base_model = card_data.get("base_model")
    repo_id = extract_repo_id(base_model)
    if repo_id:
        return repo_id
    return extract_repo_id(metadata.get("download_url"))


def normalize_family_token(family: str) -> str:
    """Normalize family to a searchable token."""
    token = normalize_name(family).lower()
    if token in ("", "unknown", "imported"):
        return ""
    return token


def has_related_metadata(metadata: dict[str, Any]) -> bool:
    """Return True when metadata can support related-model lookup."""
    family_token = normalize_family_token(metadata.get("family", ""))
    if not family_token:
        return False
    return bool(extract_base_model_repo_id(metadata))
