"""Filesystem validation utilities for model library operations.

Provides validation for import sources and mapping targets, including:
- NTFS dirty bit detection (prevents data corruption)
- Read-only mount detection
- Filesystem writability checks
- Path existence and type validation
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from enum import Enum
from pathlib import Path

from backend.logging_config import get_logger

logger = get_logger(__name__)


class ValidationSeverity(Enum):
    """Severity levels for validation issues."""

    INFO = 1
    WARNING = 2
    ERROR = 3


@dataclass
class ValidationIssue:
    """A single validation issue found during filesystem validation."""

    severity: ValidationSeverity
    message: str
    path: Path


class ValidationResult:
    """Result of filesystem validation containing all issues found."""

    def __init__(self, path: Path, issues: list[ValidationIssue] | None = None):
        """Initialize validation result.

        Args:
            path: Path that was validated
            issues: List of validation issues found (default: empty list)
        """
        self.path = path
        self.issues = issues if issues is not None else []

    def is_valid(self) -> bool:
        """Check if validation passed (no ERROR-level issues).

        Returns:
            True if no ERROR-level issues were found
        """
        return not self.has_errors()

    def has_warnings(self) -> bool:
        """Check if any WARNING-level issues were found.

        Returns:
            True if at least one WARNING issue exists
        """
        return any(issue.severity == ValidationSeverity.WARNING for issue in self.issues)

    def has_errors(self) -> bool:
        """Check if any ERROR-level issues were found.

        Returns:
            True if at least one ERROR issue exists
        """
        return any(issue.severity == ValidationSeverity.ERROR for issue in self.issues)


def _get_filesystem_type(path: Path) -> str:
    """Get the filesystem type for a given path.

    Args:
        path: Path to check

    Returns:
        Filesystem type string (e.g., "ext4", "ntfs", "btrfs") or empty string if unknown
    """
    try:
        # Use df to get filesystem type
        result = subprocess.run(
            ["df", "--output=fstype", str(path)],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
        if result.returncode == 0:
            lines = result.stdout.strip().split("\n")
            if len(lines) >= 2:
                return lines[1].strip().lower()
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as e:  # noqa: multi-exception
        logger.debug("Failed to get filesystem type for %s: %s", path, e)

    return ""


def is_ntfs_dirty(path: Path) -> bool:
    """Check if an NTFS filesystem has the dirty bit set.

    The NTFS dirty bit indicates the filesystem was not cleanly unmounted
    and may have inconsistencies. Importing from or mapping to such a
    filesystem risks data corruption.

    Args:
        path: Path on the NTFS filesystem to check

    Returns:
        True if the filesystem is NTFS and has the dirty bit set,
        False otherwise (including for non-NTFS filesystems)
    """
    # Only check NTFS filesystems
    fs_type = _get_filesystem_type(path)
    if fs_type not in ("ntfs", "ntfs3"):
        return False

    try:
        # Find the device for this path
        result = subprocess.run(
            ["df", "--output=source", str(path)],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
        if result.returncode != 0:
            return False

        lines = result.stdout.strip().split("\n")
        if len(lines) < 2:
            return False

        device = lines[1].strip()

        # Use ntfsinfo to check dirty bit (requires ntfs-3g package)
        result = subprocess.run(
            ["ntfsinfo", "-m", device],
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
        )

        if result.returncode == 0:
            # Look for dirty bit indicator in output
            output = result.stdout.lower()
            if "dirty" in output or "state: 1" in output:
                return True

    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as e:  # noqa: multi-exception
        logger.debug("Failed to check NTFS dirty bit for %s: %s", path, e)

    return False


def is_path_on_readonly_mount(path: Path) -> bool:
    """Check if a path is on a read-only mounted filesystem.

    Args:
        path: Path to check

    Returns:
        True if path is on a read-only mount, False otherwise
    """
    try:
        # Get the closest existing ancestor
        check_path = path
        while not check_path.exists() and check_path != check_path.parent:
            check_path = check_path.parent

        if not check_path.exists():
            return False

        # Check mount flags using statvfs
        stat = os.statvfs(str(check_path))
        # ST_RDONLY = 1
        is_readonly = bool(stat.f_flag & 1)
        return is_readonly

    except OSError as e:
        logger.debug("Failed to check readonly status for %s: %s", path, e)
        return False


def is_filesystem_writable(path: Path) -> bool:
    """Check if a filesystem path is writable.

    For directories, checks if we have write permission.
    For files, checks if the parent directory is writable.
    For nonexistent paths, checks the closest existing parent.

    Args:
        path: Path to check

    Returns:
        True if the path (or its parent) is writable
    """
    try:
        # Find the closest existing path to check
        check_path = path
        while not check_path.exists() and check_path != check_path.parent:
            check_path = check_path.parent

        if not check_path.exists():
            return False

        # For files, check the parent directory
        if check_path.is_file():
            check_path = check_path.parent

        # Check write permission
        return os.access(str(check_path), os.W_OK)

    except OSError as e:
        logger.debug("Failed to check writability for %s: %s", path, e)
        return False


def validate_import_source(path: Path) -> ValidationResult:
    """Validate a path as a model import source.

    Checks:
    - Path exists
    - If directory, not empty
    - Not on read-only mount (warning)
    - NTFS filesystem not dirty (error)

    Args:
        path: Path to validate as import source

    Returns:
        ValidationResult with any issues found
    """
    issues: list[ValidationIssue] = []

    # Check path exists
    if not path.exists():
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message=f"Import source does not exist: {path}",
                path=path,
            )
        )
        return ValidationResult(path=path, issues=issues)

    # Check if directory is empty
    if path.is_dir():
        try:
            if not any(path.iterdir()):
                issues.append(
                    ValidationIssue(
                        severity=ValidationSeverity.WARNING,
                        message="Import source directory is empty",
                        path=path,
                    )
                )
        except OSError as e:  # noqa: no-except-logging
            issues.append(
                ValidationIssue(
                    severity=ValidationSeverity.ERROR,
                    message=f"Cannot read directory: {e}",
                    path=path,
                )
            )

    # Check for read-only mount (warning - can still import)
    if is_path_on_readonly_mount(path):
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.WARNING,
                message="Import source is on a read-only mount",
                path=path,
            )
        )

    # Check NTFS dirty bit (error - risk of corruption)
    if is_ntfs_dirty(path):
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message=(
                    "NTFS filesystem has dirty bit set. "
                    "Please run chkdsk/ntfsfix before importing."
                ),
                path=path,
            )
        )

    return ValidationResult(path=path, issues=issues)


def validate_mapping_target(path: Path) -> ValidationResult:
    """Validate a path as a model mapping target.

    Mapping targets must be writable directories where symlinks can be created.

    Checks:
    - If exists, must be a directory (not a file)
    - Filesystem is writable
    - Not on read-only mount (error for targets)
    - NTFS filesystem not dirty (error)

    Args:
        path: Path to validate as mapping target

    Returns:
        ValidationResult with any issues found
    """
    issues: list[ValidationIssue] = []

    # If path exists, must be a directory
    if path.exists() and not path.is_dir():
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message="Mapping target must be a directory, not a file",
                path=path,
            )
        )
        return ValidationResult(path=path, issues=issues)

    # Check if filesystem is writable
    if not is_filesystem_writable(path):
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message="Mapping target is not writable",
                path=path,
            )
        )

    # Check for read-only mount (error for targets, unlike sources)
    if is_path_on_readonly_mount(path):
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message="Mapping target is on a read-only mount",
                path=path,
            )
        )

    # Check NTFS dirty bit (error - risk of corruption)
    if is_ntfs_dirty(path):
        issues.append(
            ValidationIssue(
                severity=ValidationSeverity.ERROR,
                message=(
                    "NTFS filesystem has dirty bit set. "
                    "Please run chkdsk/ntfsfix before creating mappings."
                ),
                path=path,
            )
        )

    return ValidationResult(path=path, issues=issues)
