#!/usr/bin/env python3
"""
Version Manager for ComfyUI
Handles installation, switching, and launching of ComfyUI versions
"""

from __future__ import annotations

from pathlib import Path
from typing import Dict, List, Optional

from backend.github_integration import GitHubReleasesFetcher
from backend.installation_progress_tracker import InstallationProgressTracker
from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import GitHubRelease
from backend.resource_manager import ResourceManager
from backend.utils import ensure_directory
from backend.version_manager_components.constraints import ConstraintsMixin
from backend.version_manager_components.dependencies import DependenciesMixin
from backend.version_manager_components.installer import InstallationMixin
from backend.version_manager_components.launcher import LauncherMixin
from backend.version_manager_components.state import StateMixin

logger = get_logger(__name__)


class VersionManager(
    ConstraintsMixin, DependenciesMixin, InstallationMixin, LauncherMixin, StateMixin
):
    """Manages ComfyUI version installation, switching, and launching"""

    def __init__(
        self,
        launcher_root: Path,
        metadata_manager: MetadataManager,
        github_fetcher: GitHubReleasesFetcher,
        resource_manager: ResourceManager,
    ):
        """
        Initialize VersionManager

        Args:
            launcher_root: Root directory for the launcher
            metadata_manager: MetadataManager instance
            github_fetcher: GitHubReleasesFetcher instance
            resource_manager: ResourceManager instance
        """
        self.launcher_root = Path(launcher_root)
        self.metadata_manager = metadata_manager
        self.github_fetcher = github_fetcher
        self.resource_manager = resource_manager
        self.logs_dir = self.metadata_manager.launcher_data_dir / "logs"
        ensure_directory(self.logs_dir)
        self.constraints_dir = self.metadata_manager.cache_dir / "constraints"
        ensure_directory(self.constraints_dir)
        self._constraints_cache_file = self.metadata_manager.cache_dir / "constraints-cache.json"
        self._constraints_cache: Dict[str, Dict[str, str]] = self._load_constraints_cache()
        self._pypi_release_cache: Dict[str, Dict[str, datetime]] = {}

        # Track active version for this session so user selections are not overridden
        self._active_version: Optional[str] = None

        # Directories
        self.versions_dir = self.launcher_root / "comfyui-versions"
        self.active_version_file = self.launcher_root / ".active-version"

        # Ensure versions directory exists
        ensure_directory(self.versions_dir)
        # Shared pip cache directory (persists across installs)
        self.pip_cache_dir = self.metadata_manager.cache_dir / "pip"
        if ensure_directory(self.pip_cache_dir):
            logger.info(f"Using pip cache directory at {self.pip_cache_dir}")
        self.active_pip_cache_dir = self.pip_cache_dir

        # Initialize progress tracker (Phase 6.2.5b)
        cache_dir = metadata_manager.launcher_data_dir / "cache"
        self.progress_tracker = InstallationProgressTracker(cache_dir)

        # Cancellation flag (Phase 6.2.5d)
        self._cancel_installation = False
        self._installing_tag = None
        self._current_process = None  # Track active subprocess for immediate kill
        self._current_downloader = None  # Track active downloader for immediate cancel
        self._install_log_handle = None
        self._current_install_log_path: Optional[Path] = None

        # Establish startup active version using priority rules
        self._initialize_active_version()

    def get_available_releases(
        self, force_refresh: bool = False, collapse: bool = True, include_prerelease: bool = True
    ) -> List[GitHubRelease]:
        """
        Get available ComfyUI releases from GitHub

        Args:
            force_refresh: Force refresh from GitHub
            collapse: Collapse to latest patch per minor
            include_prerelease: Include prereleases in collapsed set

        Returns:
            List of GitHubRelease objects
        """
        releases = self.github_fetcher.get_releases(force_refresh)
        if collapse:
            releases = self.github_fetcher.collapse_latest_patch_per_minor(
                releases, include_prerelease=include_prerelease
            )
        return releases


if __name__ == "__main__":
    # For testing - demonstrate version manager
    from backend.utils import get_launcher_root

    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize components
    metadata_mgr = MetadataManager(launcher_data_dir)
    github_fetcher = GitHubReleasesFetcher(metadata_mgr)
    resource_mgr = ResourceManager(launcher_root, metadata_mgr)
    version_mgr = VersionManager(launcher_root, metadata_mgr, github_fetcher, resource_mgr)

    logger.info("=== ComfyUI Version Manager ===\n")

    # Get available releases
    logger.info("Fetching available releases...")
    releases = version_mgr.get_available_releases()
    logger.info(f"Found {len(releases)} releases\n")

    # Show installed versions
    installed = version_mgr.get_installed_versions()
    logger.info(f"Installed versions: {len(installed)}")
    for tag in installed:
        info = version_mgr.get_version_info(tag)
        logger.info(f"  - {tag} (installed: {info['installDate']})")

    # Show active version
    active = version_mgr.get_active_version()
    if active:
        logger.info(f"\nActive version: {active}")
    else:
        logger.info("\nNo active version")
