"""Version information for the launcher"""

import subprocess
from pathlib import Path
from typing import Optional

from backend.logging_config import get_logger

logger = get_logger(__name__)


def get_current_commit() -> str:
    """Get current git commit SHA (short format)"""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short=7", "HEAD"],
            cwd=Path(__file__).parent.parent,
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except OSError as exc:
        logger.debug("Unable to read git commit: %s", exc)
    return "unknown"


def get_current_branch() -> str:
    """Get current git branch"""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            cwd=Path(__file__).parent.parent,
            capture_output=True,
            text=True,
            timeout=5,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except OSError as exc:
        logger.debug("Unable to read git branch: %s", exc)
    return "main"


def is_git_repo() -> bool:
    """Check if we're in a git repository"""
    git_dir = Path(__file__).parent.parent / ".git"
    return git_dir.exists()


__version__ = get_current_commit()
__branch__ = get_current_branch()
