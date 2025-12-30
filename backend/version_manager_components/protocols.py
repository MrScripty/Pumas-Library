"""Protocols defining VersionManager attributes required by mixins."""

from __future__ import annotations

import subprocess
from datetime import datetime
from pathlib import Path
from typing import IO, TYPE_CHECKING, Any, Dict, List, Optional, Protocol

if TYPE_CHECKING:
    from backend.github_integration import GitHubReleasesFetcher
    from backend.installation_progress_tracker import InstallationProgressTracker
    from backend.metadata_manager import MetadataManager
    from backend.models import GitHubRelease
    from backend.resource_manager import ResourceManager


class MixinBase:
    """Concrete base class to avoid Protocol-only inheritance in mixins."""

    pass


class ConstraintsContext(Protocol):
    constraints_dir: Path
    _constraints_cache_file: Path
    _constraints_cache: Dict[str, Dict[str, str]]
    _pypi_release_cache: Dict[str, Dict[str, datetime]]


class StateContext(Protocol):
    metadata_manager: "MetadataManager"
    versions_dir: Path
    active_version_file: Path
    _active_version: Optional[str]

    def _is_version_complete(self, version_path: Path) -> bool: ...
    def check_dependencies(self, tag: str) -> Any: ...


class LauncherContext(Protocol):
    metadata_manager: "MetadataManager"
    resource_manager: "ResourceManager"
    versions_dir: Path
    logs_dir: Path

    def get_installed_versions(self) -> List[str]: ...
    def set_active_version(self, tag: str) -> bool: ...
    def check_dependencies(self, tag: str) -> Any: ...
    def install_dependencies(self, tag: str) -> bool: ...


class DependenciesContext(Protocol):
    metadata_manager: "MetadataManager"
    launcher_root: Path
    versions_dir: Path
    github_fetcher: "GitHubReleasesFetcher"
    pip_cache_dir: Path
    active_pip_cache_dir: Path
    progress_tracker: "InstallationProgressTracker"
    _cancel_installation: bool
    _current_process: Optional[subprocess.Popen[str]]

    def _log_install(self, message: str) -> None: ...
    def _build_constraints_for_tag(
        self, tag: str, requirements_file: Path, release: Optional["GitHubRelease"]
    ) -> Optional[Path]: ...


class InstallationContext(Protocol):
    logs_dir: Path
    launcher_root: Path
    versions_dir: Path
    metadata_manager: "MetadataManager"
    resource_manager: "ResourceManager"
    github_fetcher: "GitHubReleasesFetcher"
    progress_tracker: "InstallationProgressTracker"
    _cancel_installation: bool
    _installing_tag: Optional[str]
    _current_process: Optional[subprocess.Popen[str]]
    _current_downloader: Optional[Any]
    _install_log_handle: Optional[IO[str]]
    _current_install_log_path: Optional[Path]

    def _log_install(self, message: str) -> None: ...
    def _create_venv(self, version_path: Path) -> bool: ...
    def _get_python_version(self, version_path: Path) -> str: ...
    def _install_dependencies_with_progress(self, tag: str) -> bool: ...
    def get_installed_versions(self) -> List[str]: ...
    def get_active_version(self) -> Optional[str]: ...
    def set_active_version(self, tag: str) -> bool: ...
