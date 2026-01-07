#!/usr/bin/env python3
"""
Symlink Manager
Handles user data symlinks for version installations
"""

from pathlib import Path

from backend.logging_config import get_logger
from backend.models import RepairReport
from backend.utils import is_broken_symlink, make_relative_symlink

logger = get_logger(__name__)


class SymlinkManager:
    """Manages symlinks between version directories and shared user data."""

    def __init__(self, shared_user_dir: Path, versions_dir: Path, launcher_root: Path):
        self.shared_user_dir = Path(shared_user_dir)
        self.versions_dir = Path(versions_dir)
        self.launcher_root = Path(launcher_root)

    def setup_version_symlinks(self, version_tag: str) -> bool:
        """Symlink shared user directory into the version."""
        version_path = self.versions_dir / version_tag
        if not version_path.exists():
            logger.error("Error: Version directory not found: %s", version_path)
            return False

        user_link = version_path / "user"
        if not make_relative_symlink(self.shared_user_dir, user_link):
            logger.error("Failed to create user symlink for %s", version_tag)
            return False

        logger.info("Created user symlink: %s/user -> shared-resources/user", version_tag)
        return True

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """Check for broken user symlinks and attempt repair."""
        version_path = self.versions_dir / version_tag
        report: RepairReport = {"broken": [], "repaired": [], "removed": []}

        if not version_path.exists():
            logger.error("Error: Version directory not found: %s", version_path)
            return report

        link_path = version_path / "user"
        expected_target = self.shared_user_dir

        if is_broken_symlink(link_path):
            report["broken"].append(str(link_path.relative_to(self.launcher_root)))
            if expected_target.exists():
                if make_relative_symlink(expected_target, link_path):
                    report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                    logger.info("Repaired symlink: user")
                else:
                    logger.error("Failed to repair symlink: user")
            else:
                link_path.unlink()
                report["removed"].append(str(link_path.relative_to(self.launcher_root)))
                logger.info("Removed broken symlink: user")
        elif not link_path.exists():
            if expected_target.exists():
                if make_relative_symlink(expected_target, link_path):
                    report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                    logger.info("Created missing symlink: user")

        return report
