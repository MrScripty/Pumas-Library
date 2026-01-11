"""Model format detection utilities."""

from __future__ import annotations

from typing import Iterable

KNOWN_FORMATS: frozenset[str] = frozenset(
    {
        "safetensors",
        "gguf",
        "ckpt",
        "pt",
        "pth",
        "bin",
        "onnx",
        "tflite",
        "mlmodel",
        "pb",
    }
)


def extract_formats_from_paths(paths: Iterable[str], tags: Iterable[str]) -> list[str]:
    """Extract model formats from file paths and tags.

    Args:
        paths: File paths to scan for format extensions
        tags: Tags to scan for format keywords

    Returns:
        Sorted list of detected format names
    """
    formats: set[str] = set()

    for path in paths:
        lower = path.lower()
        for ext in KNOWN_FORMATS:
            token = f".{ext}"
            if lower.endswith(token) or token in lower:
                formats.add(ext)

    for tag in tags:
        lower = tag.lower()
        for ext in KNOWN_FORMATS:
            if ext in lower:
                formats.add(ext)

    return sorted(formats)


def extract_formats(siblings: Iterable[object], tags: Iterable[str]) -> list[str]:
    """Extract model formats from HuggingFace sibling objects and tags.

    Args:
        siblings: HuggingFace sibling objects with rfilename attribute
        tags: Tags to scan for format keywords

    Returns:
        Sorted list of detected format names
    """
    paths = [getattr(sibling, "rfilename", "") or "" for sibling in siblings]
    return extract_formats_from_paths(paths, tags)
