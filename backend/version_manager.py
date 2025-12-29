#!/usr/bin/env python3
"""
Version Manager for ComfyUI
Handles installation, switching, and launching of ComfyUI versions
"""

import os
import shutil
import subprocess
import tarfile
import time
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from backend.config import INSTALLATION
from backend.exceptions import DependencyError, InstallationError, NetworkError, ResourceError
from backend.github_integration import DownloadManager, GitHubReleasesFetcher
from backend.installation_progress_tracker import InstallationProgressTracker, InstallationStage
from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import GitHubRelease, VersionInfo, get_iso_timestamp
from backend.resource_manager import ResourceManager
from backend.utils import ensure_directory, safe_filename
from backend.version_manager_components.constraints import ConstraintsMixin
from backend.version_manager_components.dependencies import DependenciesMixin
from backend.version_manager_components.launcher import LauncherMixin

logger = get_logger(__name__)


class VersionManager(ConstraintsMixin, DependenciesMixin, LauncherMixin):
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

    def _write_active_version_file(self, tag: Optional[str]) -> bool:
        """Persist active version tag to file or clear it when None."""
        try:
            if tag:
                self.active_version_file.write_text(tag)
            else:
                if self.active_version_file.exists():
                    self.active_version_file.unlink()
            return True
        except (IOError, OSError) as exc:
            logger.error(f"Error writing active version file: {exc}", exc_info=True)
            return False

    def _set_active_version_state(self, tag: Optional[str], update_last_selected: bool) -> bool:
        """
        Update in-memory and on-disk active version state.

        Args:
            tag: Version tag to mark active, or None
            update_last_selected: When True, persist as lastSelectedVersion (user choice)
        """
        self._active_version = tag
        success = self._write_active_version_file(tag)

        if update_last_selected:
            versions_metadata = self.metadata_manager.load_versions()
            versions_metadata["lastSelectedVersion"] = tag
            success = self.metadata_manager.save_versions(versions_metadata) and success

        return success

    def _initialize_active_version(self) -> Optional[str]:
        """
        Set the startup active version using priority:
        1) defaultVersion
        2) lastSelectedVersion
        3) newest installed
        """
        installed_versions = self.get_installed_versions()
        if not installed_versions:
            self._set_active_version_state(None, update_last_selected=False)
            return None

        versions_metadata = self.metadata_manager.load_versions()
        candidates = [
            versions_metadata.get("defaultVersion"),
            versions_metadata.get("lastSelectedVersion"),
        ]

        for candidate in candidates:
            if candidate and candidate in installed_versions:
                self._set_active_version_state(candidate, update_last_selected=False)
                return candidate

        newest = sorted(installed_versions, reverse=True)[0]
        self._set_active_version_state(newest, update_last_selected=False)
        return newest

    def _open_install_log(self, prefix: str) -> Path:
        """Create/open an install log file for the current attempt."""
        timestamp = int(time.time())
        filename = f"{prefix}-{timestamp}.log"
        log_path = self.logs_dir / filename
        try:
            self._install_log_handle = open(log_path, "a", encoding="utf-8")
            self._current_install_log_path = log_path
            header = f"{'='*30} INSTALL START {prefix} @ {datetime.now(timezone.utc).isoformat()} {'='*30}\n"
            self._install_log_handle.write(header)
            self._install_log_handle.flush()
        except (IOError, OSError) as exc:
            logger.warning(f"Unable to open install log at {log_path}: {exc}")
            self._install_log_handle = None
            self._current_install_log_path = log_path
        return log_path

    def _log_install(self, message: str):
        """Append a line to the current install log."""
        if not message:
            return
        if self._install_log_handle:
            try:
                self._install_log_handle.write(message.rstrip() + "\n")
                self._install_log_handle.flush()
            except (IOError, OSError) as exc:
                logger.warning(f"Failed to write to install log: {exc}")

    def get_installation_progress(self) -> Optional[Dict]:
        """
        Get current installation progress (Phase 6.2.5b)

        Returns:
            Progress state dict or None if no installation in progress
        """
        return self.progress_tracker.get_current_state()

    def cancel_installation(self) -> bool:
        """
        Cancel the currently running installation (Phase 6.2.5d)

        Immediately kills any running subprocess and cancels downloads.

        Returns:
            True if cancellation was requested
        """
        if self._installing_tag:
            logger.info("\n" + "=" * 60)
            logger.info(f"⚠️  CANCELLATION REQUESTED for {self._installing_tag}")
            logger.info("=" * 60)
            self._cancel_installation = True

            # Immediately cancel any active download
            if self._current_downloader:
                try:
                    logger.info("→ Cancelling active download...")
                    self._current_downloader.cancel()
                    logger.info("✓ Download cancelled")
                except (AttributeError, RuntimeError) as e:
                    # Downloader may not support cancellation or already finished
                    logger.error(f"✗ Error cancelling download: {e}", exc_info=True)

            # Immediately kill any running process and all its children
            if self._current_process:
                try:
                    pid = self._current_process.pid
                    logger.info(f"→ Terminating subprocess (PID: {pid}) and all child processes...")

                    # Kill the entire process group to ensure all children are terminated
                    try:
                        if hasattr(os, "killpg"):
                            # Send SIGTERM first for graceful shutdown
                            os.killpg(os.getpgid(pid), 15)
                            # Wait a moment
                            time.sleep(0.5)
                            # Check if still alive - need to check if process object is still valid
                            try:
                                still_alive = self._current_process.poll() is None
                            except (OSError, ValueError):
                                # Process object might be invalid, assume it's dead
                                still_alive = False

                            if still_alive:
                                # Force kill with SIGKILL
                                os.killpg(os.getpgid(pid), 9)
                        else:
                            self._current_process.kill()

                        # Wait for process to die
                        try:
                            self._current_process.wait(
                                timeout=INSTALLATION.SUBPROCESS_STOP_TIMEOUT_SEC
                            )
                        except (subprocess.TimeoutExpired, OSError):
                            pass  # Process might already be gone or timeout
                        logger.info("✓ All processes terminated")
                    except ProcessLookupError:
                        logger.info("✓ Process already terminated")
                    except (OSError, PermissionError) as kill_error:
                        logger.error(f"✗ Error killing process group: {kill_error}", exc_info=True)
                        # Fallback: try killing just the main process
                        try:
                            self._current_process.kill()
                            self._current_process.wait(
                                timeout=INSTALLATION.SUBPROCESS_KILL_TIMEOUT_SEC
                            )
                            logger.info("✓ Main process killed")
                        except (subprocess.TimeoutExpired, OSError):
                            pass
                except (OSError, PermissionError) as e:
                    logger.error(f"✗ Error terminating subprocess: {e}", exc_info=True)

            self.progress_tracker.set_error("Installation cancelled by user")
            logger.info("=" * 60)
            logger.info("✓ INSTALLATION CANCELLED")
            logger.info("=" * 60 + "\n")
            return True
        return False

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

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed version tags (validated against actual directories)

        Returns:
            List of version tags that are both in metadata and have valid directories
        """
        versions_metadata = self.metadata_manager.load_versions()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())

        # Verify each version actually exists on disk
        validated_versions = []
        needs_cleanup = False

        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                # Version is in metadata but incomplete/missing on disk
                logger.warning(f"Version {tag} is incomplete or missing, removing from metadata")
                needs_cleanup = True

        # Clean up metadata if we found incomplete versions
        # NOTE: This ONLY modifies the 'installed' dict in versions.json
        # It does NOT touch the GitHub releases cache or any other cache files
        if needs_cleanup:
            for tag in metadata_versions:
                if tag not in validated_versions:
                    del versions_metadata["installed"][tag]
            self.metadata_manager.save_versions(versions_metadata)
            logger.info(
                f"✓ Cleaned up metadata - removed {len(metadata_versions) - len(validated_versions)} incomplete version(s)"
            )

        return validated_versions

    def validate_installations(self) -> Dict[str, Any]:
        """
        Validate all installations and return cleanup report

        This is meant to be called at startup to detect and clean up
        any incomplete installations, and report back to the frontend
        so it can refresh the UI if needed.

        Checks two scenarios:
        1. Metadata says installed, but directory is incomplete/missing
        2. Directory exists, but no metadata (cancelled/interrupted install)

        Returns:
            Dict with:
                - had_invalid: bool - whether any invalid installations were found
                - removed: List[str] - tags of removed versions
                - valid: List[str] - tags of valid installed versions
        """
        versions_metadata = self.metadata_manager.load_versions()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())

        validated_versions = []
        removed_versions = []

        # Check 1: Validate versions in metadata
        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                removed_versions.append(tag)
                logger.warning(f"Version {tag} in metadata but directory incomplete/missing")

        # Check 2: Look for orphaned directories (no metadata = incomplete install)
        if self.versions_dir.exists():
            for version_dir in self.versions_dir.iterdir():
                if version_dir.is_dir():
                    tag = version_dir.name
                    # If directory exists but NOT in metadata, it's an incomplete install
                    if tag not in metadata_versions:
                        removed_versions.append(tag)
                        logger.warning(
                            f"Found incomplete installation directory: {tag} (not in metadata)"
                        )
                        # Remove the orphaned directory
                        try:
                            shutil.rmtree(version_dir)
                            logger.info(f"✓ Removed incomplete installation directory: {tag}")
                        except (OSError, PermissionError) as e:
                            logger.error(f"Error removing {tag}: {e}", exc_info=True)

        # Clean up metadata if we found incomplete versions in metadata
        if any(tag in metadata_versions for tag in removed_versions):
            for tag in removed_versions:
                if tag in versions_metadata["installed"]:
                    del versions_metadata["installed"][tag]
            self.metadata_manager.save_versions(versions_metadata)
            logger.info(
                f"✓ Cleaned up {len(removed_versions)} incomplete installation(s): {', '.join(removed_versions)}"
            )

        return {
            "had_invalid": len(removed_versions) > 0,
            "removed": removed_versions,
            "valid": validated_versions,
        }

    def _is_version_complete(self, version_path: Path) -> bool:
        """
        Check if a version installation is complete

        Args:
            version_path: Path to version directory

        Returns:
            True if version appears complete
        """
        if not version_path.exists():
            return False

        # Check for essential files/directories
        required_paths = [
            version_path / "main.py",  # Core ComfyUI file
            version_path / "venv",  # Virtual environment
            version_path / "venv" / "bin" / "python",  # Python in venv
        ]

        for path in required_paths:
            if not path.exists():
                return False

        return True

    def get_version_info(self, tag: str) -> Optional[VersionInfo]:
        """
        Get info about an installed version

        Args:
            tag: Version tag

        Returns:
            VersionInfo or None if not installed
        """
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get("installed", {}).get(tag)

    def get_version_path(self, tag: str) -> Optional[Path]:
        """
        Get filesystem path for an installed version.

        Args:
            tag: Version tag

        Returns:
            Path to version directory or None if missing/incomplete
        """
        version_path = self.versions_dir / tag
        if not version_path.exists():
            return None

        if not self._is_version_complete(version_path):
            return None

        return version_path

    def get_active_version(self) -> Optional[str]:
        """
        Get currently active version tag for this session.

        If the session has no active version (or it points to a missing install),
        re-evaluate using startup priority: defaultVersion → lastSelectedVersion
        → newest installed.

        Returns:
            Active version tag or None
        """
        installed_versions = self.get_installed_versions()

        # If no versions installed, return None
        if not installed_versions:
            self._active_version = None
            return None

        # Honor current session selection if still valid
        if self._active_version in installed_versions:
            return self._active_version

        # Re-evaluate using startup priority when session state is missing/stale
        return self._initialize_active_version()

    def get_active_version_path(self) -> Optional[Path]:
        """
        Get filesystem path for the active version.

        Returns:
            Path or None if no active version or incomplete installation
        """
        active_tag = self.get_active_version()
        if not active_tag:
            return None

        return self.get_version_path(active_tag)

    def set_active_version(self, tag: str) -> bool:
        """
        Set a version as active

        Args:
            tag: Version tag to activate

        Returns:
            True if successful
        """
        # Verify version is installed
        if tag not in self.get_installed_versions():
            logger.warning(f"Version {tag} is not installed")
            return False

        # Update active version state (persist as user choice)
        if not self._set_active_version_state(tag, update_last_selected=True):
            return False

        logger.info(f"✓ Activated version: {tag}")
        return True

    def get_default_version(self) -> Optional[str]:
        """
        Get the default version set in metadata.
        """
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get("defaultVersion")

    def set_default_version(self, tag: Optional[str]) -> bool:
        """
        Set a version as default (or clear if tag is None).
        """
        versions_metadata = self.metadata_manager.load_versions()
        installed = versions_metadata.get("installed", {})

        if tag is not None and tag not in installed:
            logger.warning(f"Cannot set default to {tag}: not installed")
            return False

        versions_metadata["defaultVersion"] = tag
        self.metadata_manager.save_versions(versions_metadata)
        logger.info(f"✓ Default version set to: {tag}")
        return True

    def install_version(
        self, tag: str, progress_callback: Optional[Callable[[str, int, int], None]] = None
    ) -> bool:
        """
        Install a ComfyUI version (Enhanced with Phase 6.2.5b progress tracking)

        Args:
            tag: Version tag to install
            progress_callback: Optional callback(message, current, total)

        Returns:
            True if successful
        """
        # Check if already installed
        if tag in self.get_installed_versions():
            logger.info(f"Version {tag} is already installed")
            return False

        # Get release info
        release = self.github_fetcher.get_release_by_tag(tag)
        if not release:
            logger.error(f"Release {tag} not found")
            return False

        logger.info(f"Installing ComfyUI {tag}...")

        version_path = self.versions_dir / tag
        if version_path.exists():
            logger.warning(f"Version directory already exists: {version_path}")
            return False

        install_log_path = self._open_install_log(f"install-{safe_filename(tag)}")
        self._log_install(f"Starting install for {tag}")

        try:
            # Reset cancellation flag and set installing tag
            self._cancel_installation = False
            self._installing_tag = tag

            # Initialize progress tracking
            self.progress_tracker.start_installation(tag, log_path=str(install_log_path))

            # Step 1: Download release
            self.progress_tracker.update_stage(InstallationStage.DOWNLOAD, 0, f"Downloading {tag}")
            if progress_callback:
                progress_callback("Downloading release...", 1, 5)
            self._log_install(f"Downloading release from GitHub for {tag}")

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            download_url = release.get("zipball_url") or release.get("tarball_url")
            if not download_url:
                error_msg = "No download URL found in release"
                logger.error(error_msg)
                self._log_install(error_msg)
                self.progress_tracker.set_error(error_msg)
                return False

            # Determine archive type
            is_zip = "zipball" in download_url

            # Download to temporary file
            download_dir = self.launcher_root / "temp"
            ensure_directory(download_dir)

            archive_ext = ".zip" if is_zip else ".tar.gz"
            archive_path = download_dir / f"{tag}{archive_ext}"

            # Track download progress and speed for UI feedback
            downloader = DownloadManager()
            self._current_downloader = downloader  # Track for cancellation

            download_start_time = time.time()

            def on_download_progress(downloaded: int, total: int, speed: Optional[float] = None):
                # Prefer instantaneous speed from downloader; fall back to average if missing
                effective_speed = speed
                if effective_speed is None and downloaded > 0:
                    elapsed = time.time() - download_start_time
                    if elapsed > 0:
                        effective_speed = downloaded / elapsed

                total_bytes = total if total and total > 0 else None
                self.progress_tracker.update_download_progress(
                    downloaded, total_bytes, effective_speed
                )

            try:
                success = downloader.download_with_retry(
                    download_url, archive_path, progress_callback=on_download_progress
                )

                if not success:
                    error_msg = "Download failed"
                    logger.error(error_msg)
                    self._log_install(error_msg)
                    self.progress_tracker.set_error(error_msg)
                    return False
            finally:
                self._current_downloader = None  # Clear reference

            # Get archive size
            archive_size = archive_path.stat().st_size
            self.progress_tracker.update_download_progress(archive_size, archive_size)
            self.progress_tracker.add_completed_item(archive_path.name, "archive", archive_size)
            self._log_install(f"Downloaded archive to {archive_path} ({archive_size} bytes)")

            # Check for cancellation after download
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled after download")

            # Step 2: Extract archive
            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 0, "Extracting archive")
            if progress_callback:
                progress_callback("Extracting archive...", 2, 5)

            logger.info(f"Extracting {archive_path.name}...")
            self._log_install(f"Extracting archive {archive_path.name}")
            temp_extract_dir = download_dir / f"extract-{tag}"
            ensure_directory(temp_extract_dir)

            if is_zip:
                with zipfile.ZipFile(archive_path, "r") as zip_ref:
                    zip_ref.extractall(temp_extract_dir)
            else:
                with tarfile.open(archive_path, "r:gz") as tar_ref:
                    tar_ref.extractall(temp_extract_dir)

            self.progress_tracker.update_stage(
                InstallationStage.EXTRACT, 100, "Extraction complete"
            )

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            # GitHub archives extract to a subdirectory, find it
            extracted_contents = list(temp_extract_dir.iterdir())
            if len(extracted_contents) == 1 and extracted_contents[0].is_dir():
                actual_dir = extracted_contents[0]
            else:
                actual_dir = temp_extract_dir

            # Move to final location
            ensure_directory(self.versions_dir)
            shutil.move(str(actual_dir), str(version_path))

            # Clean up
            archive_path.unlink()
            if temp_extract_dir.exists():
                shutil.rmtree(temp_extract_dir)

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            # Step 3: Create venv with python3
            self.progress_tracker.update_stage(
                InstallationStage.VENV, 0, "Creating virtual environment"
            )
            if progress_callback:
                progress_callback("Creating virtual environment...", 3, 5)

            if not self._create_venv(version_path):
                error_msg = "Failed to create virtual environment"
                logger.error(error_msg)
                self._log_install(error_msg)
                self.progress_tracker.set_error(error_msg)
                shutil.rmtree(version_path)
                return False

            self.progress_tracker.update_stage(
                InstallationStage.VENV, 100, "Virtual environment created"
            )
            self._log_install("Virtual environment created successfully")

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            # Step 4: Install dependencies with progress tracking
            self.progress_tracker.update_stage(
                InstallationStage.DEPENDENCIES, 0, "Installing dependencies"
            )
            if progress_callback:
                progress_callback("Installing dependencies...", 4, 5)

            deps_success = self._install_dependencies_with_progress(tag)
            if not deps_success:
                logger.warning("Dependency installation had errors")
                self._log_install("Dependency installation failed; aborting setup")
                self.progress_tracker.set_error("Dependency installation failed")
                # Clean up failed install
                if version_path.exists():
                    try:
                        shutil.rmtree(version_path)
                        logger.info(f"✓ Removed incomplete installation directory")
                    except (OSError, PermissionError) as cleanup_error:
                        logger.warning(f"Failed to clean up directory: {cleanup_error}")
                self.progress_tracker.complete_installation(False)
                return False
            else:
                self._log_install("Dependencies installed successfully")

            # Step 5: Setup symlinks
            self.progress_tracker.update_stage(InstallationStage.SETUP, 0, "Setting up symlinks")
            if progress_callback:
                progress_callback("Setting up symlinks...", 5, 5)

            self.resource_manager.setup_version_symlinks(tag)
            self.progress_tracker.update_stage(InstallationStage.SETUP, 100, "Setup complete")

            # Update metadata
            version_info: VersionInfo = {
                "path": str(version_path.relative_to(self.launcher_root)),
                "installedDate": get_iso_timestamp(),
                "pythonVersion": self._get_python_version(version_path),
                "releaseTag": tag,
            }

            versions_metadata = self.metadata_manager.load_versions()
            if "installed" not in versions_metadata:
                versions_metadata["installed"] = {}

            versions_metadata["installed"][tag] = version_info
            self.metadata_manager.save_versions(versions_metadata)

            # Mark installation as complete
            self.progress_tracker.complete_installation(deps_success)

            if deps_success:
                logger.info(f"✓ Successfully installed {tag}")
                self._log_install(f"✓ Successfully installed {tag}")
            else:
                logger.warning(f"Installation completed with dependency errors for {tag}")
                self._log_install(f"Installation completed with dependency errors for {tag}")
            return deps_success

        except InterruptedError as e:
            # Installation was cancelled by user
            error_msg = str(e)
            logger.info(f"✓ {error_msg}")
            self._log_install(error_msg)
            self.progress_tracker.set_error(error_msg)
            self.progress_tracker.complete_installation(False)

            # Clean up cancelled installation
            if version_path.exists():
                logger.info(f"Cleaning up cancelled installation: {version_path}")
                try:
                    shutil.rmtree(version_path)
                    logger.info(f"✓ Removed incomplete installation directory")
                except (OSError, PermissionError) as cleanup_error:
                    logger.warning(f"Failed to clean up directory: {cleanup_error}")
            return False
        except (
            InstallationError,
            DependencyError,
            NetworkError,
            ResourceError,
            tarfile.TarError,
            zipfile.BadZipFile,
        ) as e:
            error_msg = f"Error installing version {tag}: {e}"
            logger.error(error_msg, exc_info=True)
            self._log_install(error_msg)
            self.progress_tracker.set_error(error_msg)
            self.progress_tracker.complete_installation(False)

            # Clean up on failure
            if version_path.exists():
                logger.info(f"Cleaning up failed installation: {version_path}")
                try:
                    shutil.rmtree(version_path)
                    logger.info(f"✓ Removed incomplete installation directory")
                except (OSError, PermissionError) as cleanup_error:
                    logger.warning(f"Failed to clean up directory: {cleanup_error}")
            return False
        finally:
            # Reset installation state
            self._installing_tag = None
            self._cancel_installation = False
            if self._install_log_handle:
                try:
                    self._install_log_handle.write(f"{'='*30} INSTALL END {tag} {'='*30}\n")
                    self._install_log_handle.close()
                except (IOError, OSError):
                    pass
                self._install_log_handle = None
            # Preserve progress state so UI can read failure logs; it will be overwritten by the next install

    def remove_version(self, tag: str) -> bool:
        """
        Remove an installed version

        Args:
            tag: Version tag to remove

        Returns:
            True if successful
        """
        if tag not in self.get_installed_versions():
            logger.warning(f"Version {tag} is not installed")
            return False

        # Check if it's the active version
        if self.get_active_version() == tag:
            logger.warning(f"Cannot remove active version {tag}")
            logger.warning("Please switch to a different version first")
            return False

        version_path = self.versions_dir / tag

        try:
            # Remove directory
            logger.info(f"Removing {tag}...")
            shutil.rmtree(version_path)

            # Update metadata
            versions_metadata = self.metadata_manager.load_versions()

            if tag in versions_metadata.get("installed", {}):
                del versions_metadata["installed"][tag]

            if versions_metadata.get("defaultVersion") == tag:
                versions_metadata["defaultVersion"] = None

            self.metadata_manager.save_versions(versions_metadata)

            logger.info(f"✓ Removed version {tag}")
            return True

        except (OSError, PermissionError) as e:
            logger.error(f"Error removing version {tag}: {e}", exc_info=True)
            return False

    def get_version_status(self) -> Dict[str, Any]:
        """
        Get comprehensive status of all versions

        Returns:
            Dict with version status information
        """
        installed = self.get_installed_versions()
        active = self.get_active_version()

        status = {
            "installedCount": len(installed),
            "activeVersion": active,
            "defaultVersion": self.get_default_version(),
            "versions": {},
        }

        for tag in installed:
            version_info = self.get_version_info(tag)
            dep_status = self.check_dependencies(tag)

            status["versions"][tag] = {
                "info": version_info,
                "dependencies": dep_status,
                "isActive": tag == active,
            }

        return status


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
