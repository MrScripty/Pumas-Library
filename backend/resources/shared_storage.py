#!/usr/bin/env python3
"""
Shared Storage Manager
Handles shared storage initialization.
"""

from pathlib import Path

from backend.utils import ensure_directory


class SharedStorageManager:
    """Manages shared storage structure."""

    def __init__(self, shared_dir: Path, versions_dir: Path, launcher_root: Path):
        self.shared_dir = Path(shared_dir)
        self.versions_dir = Path(versions_dir)
        self.launcher_root = Path(launcher_root)

        self.shared_models_dir = self.shared_dir / "models"
        self.shared_custom_nodes_cache_dir = self.shared_dir / "custom_nodes_cache"
        self.shared_user_dir = self.shared_dir / "user"
        self.shared_workflows_dir = self.shared_user_dir / "workflows"
        self.shared_settings_dir = self.shared_user_dir / "settings"

    def initialize_shared_storage(self) -> bool:
        """Create shared-resources directory structure."""
        directories = [
            self.shared_dir,
            self.shared_models_dir,
            self.shared_custom_nodes_cache_dir,
            self.shared_user_dir,
            self.shared_workflows_dir,
            self.shared_settings_dir,
        ]

        success = True
        for directory in directories:
            if not ensure_directory(directory):
                success = False

        return success
