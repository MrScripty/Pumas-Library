#!/usr/bin/env python3
"""
Cross-platform file opener utility for launching paths in the system file manager.

Uses click.launch to avoid OS-specific command branches.
"""

from pathlib import Path
from typing import Optional, Union

import click


def resolve_target_path(
    path: Union[str, Path],
    base_dir: Optional[Path] = None,
) -> Path:
    """
    Resolve a target path, allowing relative paths with an optional base directory.
    """
    target = Path(path).expanduser()

    if not target.is_absolute():
        base = base_dir or Path.cwd()
        target = (base / target).resolve()
    else:
        target = target.resolve()

    return target


def open_in_file_manager(
    path: Union[str, Path],
    base_dir: Optional[Path] = None,
) -> dict:
    """
    Open a filesystem path in the user's file manager.

    Returns:
        Dict with success status and optional error message.
    """
    if path is None or not str(path).strip():
        return {"success": False, "error": "Path is required"}

    try:
        target_path = resolve_target_path(path, base_dir=base_dir)
    except Exception as exc:
        return {"success": False, "error": f"Invalid path: {exc}"}

    if not target_path.exists():
        return {"success": False, "error": f"Path does not exist: {target_path}"}

    try:
        # locate=True highlights files when supported; directories open normally.
        launched = click.launch(str(target_path), locate=target_path.is_file(), wait=False)
        if not launched:
            return {"success": False, "error": "Unable to open file manager"}
        return {"success": True, "path": str(target_path)}
    except Exception as exc:
        return {"success": False, "error": str(exc)}
