"""File utility helpers for atomic, validated writes."""

from __future__ import annotations

import json
import os
import shutil
import threading
from pathlib import Path
from typing import Any, Optional


def _with_suffix(path: Path, suffix: str) -> Path:
    """Return a path with suffix appended, preserving files without suffixes."""
    if path.suffix:
        return path.with_suffix(path.suffix + suffix)
    return path.with_name(f"{path.name}{suffix}")


def atomic_write_json(
    path: Path, data: Any, lock: Optional[threading.Lock] = None, keep_backup: bool = True
) -> None:
    """Write JSON atomically, validating the payload and keeping an optional backup."""
    temp_path = _with_suffix(path, ".tmp")
    backup_path = _with_suffix(path, ".bak")
    if lock:
        lock.acquire()
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        serialized = json.dumps(data, indent=2, ensure_ascii=False)
        json.loads(serialized)

        if keep_backup and path.exists():
            shutil.copy2(path, backup_path)

        with open(temp_path, "w", encoding="utf-8") as f:
            f.write(serialized)
            f.flush()
            os.fsync(f.fileno())

        os.replace(temp_path, path)
    finally:
        if temp_path.exists():
            temp_path.unlink()
        if lock:
            lock.release()
