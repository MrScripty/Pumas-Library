"""Naming utilities for model library assets."""

from __future__ import annotations

import re
from pathlib import Path

from backend.logging_config import get_logger

logger = get_logger(__name__)

_ALLOWED_PATTERN = re.compile(r"[^A-Za-z0-9_-]")


def normalize_name(value: str, max_length: int = 128, fallback: str = "model") -> str:
    """Normalize a model name to a filesystem-safe ASCII form."""
    cleaned = _ALLOWED_PATTERN.sub("", value.strip())
    if not cleaned:
        cleaned = fallback
    if len(cleaned) > max_length:
        cleaned = cleaned[:max_length]
    return cleaned


def normalize_filename(filename: str, max_length: int = 128) -> str:
    """Normalize a filename while preserving the extension."""
    path = Path(filename)
    suffix = path.suffix
    stem = path.stem

    cleaned_stem = normalize_name(stem, max_length=max_length, fallback="file")
    if suffix:
        remaining = max_length - len(suffix)
        if remaining < 1:
            logger.warning("Filename extension exceeds max length, truncating stem")
            remaining = 1
        cleaned_stem = cleaned_stem[:remaining]
    return f"{cleaned_stem}{suffix}"


def unique_path(base_path: Path) -> Path:
    """Return a unique path by suffixing with an incremented number."""
    if not base_path.exists():
        return base_path

    parent = base_path.parent
    stem = base_path.stem
    suffix = base_path.suffix
    counter = 2

    while True:
        candidate_name = f"{stem}-{counter}{suffix}"
        candidate = parent / candidate_name
        if not candidate.exists():
            return candidate
        counter += 1
