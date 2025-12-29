"""Input validation helpers for ComfyUI Launcher."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Union
from urllib.parse import urlparse

from backend.exceptions import ValidationError

_VERSION_TAG_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9.-]*$")
_PACKAGE_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


def validate_version_tag(tag: str) -> bool:
    """Return True if a version tag is safe for filesystem use."""
    if not isinstance(tag, str):
        return False
    candidate = tag.strip()
    if not candidate:
        return False
    return bool(_VERSION_TAG_RE.fullmatch(candidate))


def validate_url(url: str) -> bool:
    """Return True if the URL uses http/https and has a host."""
    if not isinstance(url, str):
        return False
    candidate = url.strip()
    if not candidate:
        return False
    parsed = urlparse(candidate)
    return parsed.scheme in ("http", "https") and bool(parsed.netloc)


def sanitize_path(path: Union[str, Path], base_dir: Path) -> Path:
    """
    Resolve a path against a base directory and prevent traversal outside the base.

    Raises ValidationError when the resolved path escapes base_dir.
    """
    if not base_dir:
        raise ValidationError("Base directory is required", field_name="base_dir")

    base = Path(base_dir).expanduser().resolve()
    target = Path(path).expanduser()
    if not target.is_absolute():
        target = (base / target).resolve()
    else:
        target = target.resolve()

    try:
        target.relative_to(base)
    except ValueError as exc:
        raise ValidationError(
            "Path escapes base directory", field_name="path", invalid_value=str(path)
        ) from exc

    return target


def validate_package_name(name: str) -> bool:
    """Return True if a package name is in a safe PEP 508-ish format."""
    if not isinstance(name, str):
        return False
    candidate = name.strip()
    if not candidate:
        return False
    return bool(_PACKAGE_NAME_RE.fullmatch(candidate))
