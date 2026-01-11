"""Naming utilities for model library assets.

Provides cross-platform filename sanitization with special handling
for NTFS filesystem restrictions on Windows.
"""

from __future__ import annotations

import re
from pathlib import Path

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Characters allowed in normalized names (alphanumeric, underscore, hyphen)
_ALLOWED_PATTERN = re.compile(r"[^A-Za-z0-9_-]")

# NTFS reserved characters: < > : " / \ | ? *
_NTFS_RESERVED_CHARS = re.compile(r'[<>:"/\\|?*]')

# NTFS reserved device names (case-insensitive)
_NTFS_RESERVED_NAMES = frozenset(
    [
        "CON",
        "PRN",
        "AUX",
        "NUL",
        "COM1",
        "COM2",
        "COM3",
        "COM4",
        "COM5",
        "COM6",
        "COM7",
        "COM8",
        "COM9",
        "LPT1",
        "LPT2",
        "LPT3",
        "LPT4",
        "LPT5",
        "LPT6",
        "LPT7",
        "LPT8",
        "LPT9",
    ]
)


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


def is_ntfs_safe(filename: str) -> bool:
    """Check if a filename is safe for NTFS filesystems.

    Checks for:
    - Reserved characters: < > : " / \\ | ? *
    - Reserved device names: CON, PRN, AUX, NUL, COM1-9, LPT1-9
    - Trailing dots and spaces

    Args:
        filename: Filename to check (without directory path)

    Returns:
        True if filename is safe for NTFS
    """
    if not filename:
        return False

    # Check for reserved characters
    if _NTFS_RESERVED_CHARS.search(filename):
        return False

    # Check for reserved device names (check stem without extension)
    stem = Path(filename).stem.upper()
    if stem in _NTFS_RESERVED_NAMES:
        return False

    # Check for trailing dots or spaces
    if filename.endswith(".") or filename.endswith(" "):
        return False

    return True


def sanitize_for_ntfs(filename: str, replacement: str = "_") -> str:
    """Sanitize a filename to be safe for NTFS filesystems.

    Replaces reserved characters and handles reserved device names.

    Args:
        filename: Filename to sanitize (without directory path)
        replacement: Character to replace reserved chars with

    Returns:
        NTFS-safe filename
    """
    if not filename:
        return "file"

    # Replace reserved characters
    result = _NTFS_RESERVED_CHARS.sub(replacement, filename)

    # Remove trailing dots and spaces
    result = result.rstrip(". ")

    # Handle reserved device names
    stem = Path(result).stem
    suffix = Path(result).suffix

    if stem.upper() in _NTFS_RESERVED_NAMES:
        # Prefix with underscore to avoid conflict
        result = f"_{stem}{suffix}"

    # Ensure we have something
    if not result or result == suffix:
        result = f"file{suffix}"

    return result


def normalize_filename_ntfs(filename: str, max_length: int = 128) -> str:
    """Normalize a filename for cross-platform compatibility including NTFS.

    Combines normalize_filename with NTFS sanitization for maximum
    compatibility across Windows and Unix filesystems.

    Args:
        filename: Filename to normalize
        max_length: Maximum length for the result

    Returns:
        Normalized, NTFS-safe filename
    """
    # First apply NTFS sanitization
    safe_name = sanitize_for_ntfs(filename)

    # Then apply standard normalization
    return normalize_filename(safe_name, max_length=max_length)
