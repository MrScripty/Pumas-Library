"""Quantization token utilities for model files."""

from __future__ import annotations

import re
from typing import Iterable

QUANT_TOKENS: tuple[str, ...] = (
    "iq1",
    "iq1_s",
    "iq1_m",
    "iq2_xxs",
    "iq2_xs",
    "iq2_s",
    "iq2_k_s",
    "iq2_k",
    "iq2_m",
    "iq3_xxs",
    "iq3_xs",
    "iq3_s",
    "iq3_k_s",
    "iq3_m",
    "iq3_k_m",
    "iq3_k_l",
    "iq4_xxs",
    "iq4_xs",
    "iq4_s",
    "iq4_m",
    "iq4_k_s",
    "iq4_k_m",
    "iq4_k_l",
    "q2",
    "q2_k",
    "q2_k_s",
    "q2_k_m",
    "q3",
    "q3_k",
    "q3_k_s",
    "q3_k_m",
    "q3_k_l",
    "q4",
    "q4_0",
    "q4_1",
    "q4_k_s",
    "q4_k_m",
    "q5",
    "q5_0",
    "q5_1",
    "q5_k_s",
    "q5_k_m",
    "q6",
    "q6_k",
    "q8",
    "q8_0",
    "int4",
    "int8",
    "fp16",
    "fp32",
    "bf16",
    "f16",
    "f32",
)


def normalize_quant_source(value: str) -> str:
    """Normalize a string for quant token matching.

    Args:
        value: Input string (filename, path, or tag)

    Returns:
        Lowercase string with non-alphanumeric chars replaced by underscores
    """
    normalized = re.sub(r"[^a-z0-9]+", "_", value.lower())
    return normalized.strip("_")


def token_in_normalized(normalized: str, token: str) -> bool:
    """Check if a quant token appears in a normalized string.

    Args:
        normalized: Pre-normalized string to search in
        token: Quant token to find

    Returns:
        True if the token appears as a complete segment match
    """
    if not normalized or not token:
        return False
    segments = normalized.split("_")
    token_segments = token.split("_")
    if not token_segments or len(token_segments) > len(segments):
        return False
    for index in range(len(segments) - len(token_segments) + 1):
        if segments[index : index + len(token_segments)] == token_segments:
            return True
    return False


def sorted_quants(quants: Iterable[str]) -> list[str]:
    """Sort quantization tokens by standard ordering.

    Args:
        quants: Collection of quant tokens to sort

    Returns:
        Sorted list with duplicates and empty strings removed
    """
    order = {token: index for index, token in enumerate(QUANT_TOKENS)}
    unique = {quant for quant in quants if quant}
    return sorted(unique, key=lambda token: (order.get(token, len(order)), token))


def extract_quants_from_paths(paths: Iterable[str], tags: Iterable[str]) -> list[str]:
    """Extract quantization tokens from file paths and tags.

    Args:
        paths: File paths to scan for quant tokens
        tags: Tags to scan for quant tokens

    Returns:
        Sorted list of detected quant tokens
    """
    quants: set[str] = set()
    quant_tokens = sorted(QUANT_TOKENS, key=len, reverse=True)

    for path in paths:
        normalized = normalize_quant_source(path)
        for token in quant_tokens:
            if token_in_normalized(normalized, token):
                quants.add(token)
                break

    for tag in tags:
        normalized = normalize_quant_source(tag)
        for token in quant_tokens:
            if token_in_normalized(normalized, token):
                quants.add(token)
                break

    return sorted_quants(quants)


def quant_sizes_from_paths(paths_with_sizes: Iterable[tuple[str, int]]) -> dict[str, int]:
    """Calculate total size per quantization from file paths.

    Args:
        paths_with_sizes: Iterable of (path, size_bytes) tuples

    Returns:
        Mapping of quant token to total bytes for that quant
    """
    quant_sizes: dict[str, int] = {}
    tokens = sorted(QUANT_TOKENS, key=len, reverse=True)
    shared_size = 0
    shared_exts = {".json", ".yml", ".yaml", ".txt", ".md"}

    for path, size in paths_with_sizes:
        normalized = normalize_quant_source(path)
        lower = path.lower()
        matched = None
        for token in tokens:
            if token_in_normalized(normalized, token):
                matched = token
                break
        if matched:
            quant_sizes[matched] = quant_sizes.get(matched, 0) + size
        else:
            if any(lower.endswith(ext) for ext in shared_exts):
                shared_size += size

    if shared_size and quant_sizes:
        for token in list(quant_sizes.keys()):
            quant_sizes[token] += shared_size

    return quant_sizes
