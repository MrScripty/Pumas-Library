#!/usr/bin/env python3
"""
Symlink Manager
Handles symlink creation and validation for version installations
"""

from pathlib import Path
from typing import Dict

from backend.models import RepairReport
from backend.utils import is_broken_symlink, make_relative_symlink


class SymlinkManager:
    """Manages symlinks between version directories and shared storage"""

    def __init__(
        self,
        shared_models_dir: Path,
        shared_user_dir: Path,
        versions_dir: Path,
        launcher_root: Path,
    ):
        """
        Initialize symlink manager

        Args:
            shared_models_dir: Path to shared models directory
            shared_user_dir: Path to shared user directory
            versions_dir: Path to comfyui-versions directory
            launcher_root: Path to launcher root directory
        """
        self.shared_models_dir = Path(shared_models_dir)
        self.shared_user_dir = Path(shared_user_dir)
        self.versions_dir = Path(versions_dir)
        self.launcher_root = Path(launcher_root)

    def setup_version_symlinks(self, version_tag: str) -> bool:
        """
        Setup all symlinks for a version
        - Symlinks models directory
        - Symlinks user data (workflows, settings)
        - Does NOT symlink custom_nodes (real files per version)

        Args:
            version_tag: Version tag (e.g., "v0.2.0")

        Returns:
            True if successful
        """
        version_path = self.versions_dir / version_tag

        if not version_path.exists():
            print(f"Error: Version directory not found: {version_path}")
            return False

        success = True

        # 1. Symlink models directory
        models_link = version_path / "models"
        if not make_relative_symlink(self.shared_models_dir, models_link):
            print(f"Failed to create models symlink for {version_tag}")
            success = False
        else:
            print(f"Created models symlink: {version_tag}/models -> shared-resources/models")

        # 2. Symlink user directory
        user_link = version_path / "user"
        if not make_relative_symlink(self.shared_user_dir, user_link):
            print(f"Failed to create user symlink for {version_tag}")
            success = False
        else:
            print(f"Created user symlink: {version_tag}/user -> shared-resources/user")

        return success

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """
        Check for broken symlinks and attempt repair

        Args:
            version_tag: Version tag to check

        Returns:
            RepairReport with broken, repaired, and removed symlinks
        """
        version_path = self.versions_dir / version_tag

        report: RepairReport = {"broken": [], "repaired": [], "removed": []}

        if not version_path.exists():
            print(f"Error: Version directory not found: {version_path}")
            return report

        # Check key symlinks
        symlinks_to_check = {
            "models": self.shared_models_dir,
            "user": self.shared_user_dir,
        }

        for link_name, expected_target in symlinks_to_check.items():
            link_path = version_path / link_name

            if is_broken_symlink(link_path):
                report["broken"].append(str(link_path.relative_to(self.launcher_root)))

                # Try to repair
                if expected_target.exists():
                    if make_relative_symlink(expected_target, link_path):
                        report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                        print(f"Repaired symlink: {link_name}")
                    else:
                        print(f"Failed to repair symlink: {link_name}")
                else:
                    # Target doesn't exist, remove broken symlink
                    link_path.unlink()
                    report["removed"].append(str(link_path.relative_to(self.launcher_root)))
                    print(f"Removed broken symlink: {link_name}")

            elif not link_path.exists():
                # Symlink doesn't exist, create it
                if expected_target.exists():
                    if make_relative_symlink(expected_target, link_path):
                        report["repaired"].append(str(link_path.relative_to(self.launcher_root)))
                        print(f"Created missing symlink: {link_name}")

        return report
