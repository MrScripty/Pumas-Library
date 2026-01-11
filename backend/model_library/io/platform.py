"""Platform abstraction for link creation.

Provides cross-platform utilities for creating symlinks, hardlinks,
and other file linking strategies. Currently supports Linux with
symlinks as the default, with Windows copy fallback planned.
"""

from __future__ import annotations

import os
import shutil
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from backend.logging_config import get_logger

logger = get_logger(__name__)


class LinkStrategy(Enum):
    """Available strategies for linking files."""

    SYMLINK = "symlink"
    HARDLINK = "hardlink"
    COPY = "copy"
    REFLINK = "reflink"  # CoW copy, not widely supported


@dataclass
class LinkResult:
    """Result of a link creation operation.

    Attributes:
        success: Whether the operation succeeded
        source: Source path
        target: Target path (link location)
        strategy: Strategy that was used
        error: Error message if operation failed
    """

    success: bool
    source: Path
    target: Path
    strategy: LinkStrategy
    error: str | None = None


def get_default_strategy() -> LinkStrategy:
    """Get the default linking strategy for the current platform.

    Returns:
        SYMLINK for Linux/macOS, COPY for Windows
    """
    if sys.platform in ("linux", "darwin"):
        return LinkStrategy.SYMLINK
    else:
        # Windows symlinks require admin privileges
        return LinkStrategy.COPY


def get_available_strategies() -> list[LinkStrategy]:
    """Get list of available linking strategies for current platform.

    Returns:
        List of available strategies in order of preference
    """
    strategies = []

    if sys.platform in ("linux", "darwin"):
        strategies.append(LinkStrategy.SYMLINK)
        strategies.append(LinkStrategy.HARDLINK)

    # COPY is always available
    strategies.append(LinkStrategy.COPY)

    return strategies


def is_cross_filesystem(source: Path, target: Path) -> bool:
    """Check if two paths are on different filesystems.

    This matters for hardlinks which cannot cross filesystem boundaries.

    Args:
        source: Source path
        target: Target path (can be nonexistent, will check parent)

    Returns:
        True if paths are on different filesystems
    """
    try:
        # Get device IDs
        source_path = source if source.exists() else source.parent
        target_path = target if target.exists() else target.parent

        # Walk up to find existing ancestor
        while not source_path.exists() and source_path != source_path.parent:
            source_path = source_path.parent
        while not target_path.exists() and target_path != target_path.parent:
            target_path = target_path.parent

        if not source_path.exists() or not target_path.exists():
            return False  # Can't determine, assume same FS

        source_dev = source_path.stat().st_dev
        target_dev = target_path.stat().st_dev

        return source_dev != target_dev

    except OSError as e:
        logger.debug("Failed to check cross-filesystem: %s", e)
        return False


def _compute_relative_path(source: Path, target: Path) -> Path:
    """Compute relative path from target to source for symlinks.

    Args:
        source: The actual file location
        target: Where the symlink will be created

    Returns:
        Relative path from target's parent to source
    """
    try:
        return Path(os.path.relpath(source, target.parent))
    except ValueError:  # noqa: no-except-logging
        # On Windows, relpath fails for paths on different drives
        return source.resolve()


def create_link(
    source: Path,
    target: Path,
    strategy: LinkStrategy,
    relative: bool = True,
    overwrite: bool = False,
) -> LinkResult:
    """Create a link from target to source using specified strategy.

    Args:
        source: The actual file to link to
        target: Where the link will be created
        strategy: Linking strategy to use
        relative: For symlinks, use relative paths (default: True)
        overwrite: Replace existing files/links (default: False)

    Returns:
        LinkResult with success status and any error message
    """
    # Validate source exists
    if not source.exists():
        return LinkResult(
            success=False,
            source=source,
            target=target,
            strategy=strategy,
            error=f"Source file not found: {source}",
        )

    # Check target existence
    if target.exists() or target.is_symlink():
        if not overwrite:
            return LinkResult(
                success=False,
                source=source,
                target=target,
                strategy=strategy,
                error=f"Target already exists: {target}",
            )
        # Remove existing
        try:
            target.unlink()
        except OSError as e:  # noqa: no-except-logging
            return LinkResult(
                success=False,
                source=source,
                target=target,
                strategy=strategy,
                error=f"Failed to remove existing target: {e}",
            )

    # Create parent directories
    try:
        target.parent.mkdir(parents=True, exist_ok=True)
    except OSError as e:  # noqa: no-except-logging
        return LinkResult(
            success=False,
            source=source,
            target=target,
            strategy=strategy,
            error=f"Failed to create parent directory: {e}",
        )

    # Execute strategy
    try:
        if strategy == LinkStrategy.SYMLINK:
            if relative:
                link_target = _compute_relative_path(source, target)
            else:
                link_target = source.resolve()
            target.symlink_to(link_target)

        elif strategy == LinkStrategy.HARDLINK:
            target.hardlink_to(source)

        elif strategy == LinkStrategy.COPY:
            shutil.copy2(str(source), str(target))

        elif strategy == LinkStrategy.REFLINK:
            # Reflink (CoW copy) - fall back to regular copy
            # A true reflink would use: cp --reflink=auto
            shutil.copy2(str(source), str(target))

        else:
            return LinkResult(
                success=False,
                source=source,
                target=target,
                strategy=strategy,
                error=f"Unknown strategy: {strategy}",
            )

        logger.debug("Created %s link: %s -> %s", strategy.value, target, source)
        return LinkResult(
            success=True,
            source=source,
            target=target,
            strategy=strategy,
        )

    except OSError as e:  # noqa: no-except-logging
        return LinkResult(
            success=False,
            source=source,
            target=target,
            strategy=strategy,
            error=str(e),
        )


def verify_link(target: Path) -> tuple[bool, str | None]:
    """Verify that a link is valid and points to an existing file.

    Args:
        target: Path to the link

    Returns:
        Tuple of (is_valid, error_message)
    """
    if not target.exists() and not target.is_symlink():
        return False, f"Link not found: {target}"

    if target.is_symlink():
        # Check if symlink target exists
        try:
            target.resolve(strict=True)
            return True, None
        except OSError:  # noqa: no-except-logging
            return False, f"Broken symlink: {target} -> {os.readlink(target)}"

    # Regular file or hardlink - just check it exists
    if target.exists():
        return True, None

    return False, f"Link target does not exist: {target}"


def remove_link(target: Path, force: bool = False) -> bool:
    """Remove a link (symlink or hardlink).

    For safety, only removes symlinks by default. Set force=True
    to remove regular files as well.

    Args:
        target: Path to the link to remove
        force: If True, also remove regular files

    Returns:
        True if link was removed or didn't exist
    """
    if not target.exists() and not target.is_symlink():
        return True  # Already gone

    if target.is_symlink():
        try:
            target.unlink()
            logger.debug("Removed symlink: %s", target)
            return True
        except OSError as e:
            logger.error("Failed to remove symlink %s: %s", target, e)
            return False

    # Not a symlink
    if not force:
        logger.warning("Refusing to remove non-symlink without force: %s", target)
        return False

    try:
        target.unlink()
        logger.debug("Removed file: %s", target)
        return True
    except OSError as e:
        logger.error("Failed to remove file %s: %s", target, e)
        return False
