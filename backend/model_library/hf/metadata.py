"""Metadata utilities for HuggingFace models."""

from __future__ import annotations

from typing import Iterable

from backend.logging_config import get_logger

logger = get_logger(__name__)

KIND_TAG_MAPPING: dict[str, list[str]] = {
    "text-to-image": ["text-to-image", "text2img", "text-to-img"],
    "image-to-image": ["image-to-image", "img2img", "image-to-img"],
    "text-to-video": ["text-to-video", "text2video"],
    "text-to-audio": ["text-to-audio", "text-to-speech", "tts"],
    "audio-to-text": [
        "audio-to-text",
        "speech-recognition",
        "automatic-speech-recognition",
        "asr",
    ],
    "text-to-3d": ["text-to-3d", "text2shape"],
    "image-to-3d": ["image-to-3d", "img2shape", "image-to-shape"],
}


def infer_kind_from_tags(tags: Iterable[str]) -> str:
    """Infer model kind/task from tags.

    Args:
        tags: Collection of tags to analyze

    Returns:
        Inferred model kind or 'unknown' if not determinable
    """
    normalized = [tag.lower() for tag in tags]

    for kind, needles in KIND_TAG_MAPPING.items():
        for tag in normalized:
            if any(needle in tag for needle in needles):
                return kind

    for tag in normalized:
        if "video" in tag:
            return "video"
        if "audio" in tag:
            return "audio"
        if "image" in tag:
            return "image"
        if "text" in tag:
            return "text"
        if "3d" in tag:
            return "3d"

    return "unknown"


def coerce_int(value: object) -> int:
    """Coerce a value to integer, returning 0 on failure.

    Args:
        value: Value to coerce

    Returns:
        Integer value or 0 if conversion fails
    """
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        return int(value)
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError as exc:
            logger.debug("Failed to coerce string to int: %r - %s", value, exc)
            return 0
    return 0


def collect_paths_with_sizes(siblings: Iterable[object]) -> list[tuple[str, int]]:
    """Collect file paths and sizes from HuggingFace sibling objects.

    Args:
        siblings: HuggingFace sibling objects with rfilename and size/lfs attributes

    Returns:
        List of (path, size_bytes) tuples for files with positive size
    """
    results: list[tuple[str, int]] = []

    for sibling in siblings:
        path = getattr(sibling, "rfilename", "") or ""
        if not path:
            continue

        size = getattr(sibling, "size", None)
        if size is None:
            lfs = getattr(sibling, "lfs", None)
            if isinstance(lfs, dict):
                size = lfs.get("size")

        try:
            size_value = int(size) if size is not None else 0
        except TypeError as exc:
            logger.debug("Invalid size type %r: %s", size, exc)
            size_value = 0
        except ValueError as exc:
            logger.debug("Invalid size value %r: %s", size, exc)
            size_value = 0

        if size_value > 0:
            results.append((path, size_value))

    return results
