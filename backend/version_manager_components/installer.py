"""Installation helpers for VersionManager."""

from __future__ import annotations

import os
import shutil
import subprocess
import tarfile
import time
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, Optional

from backend.config import INSTALLATION
from backend.exceptions import DependencyError, InstallationError, NetworkError, ResourceError
from backend.github_integration import DownloadManager
from backend.installation_progress_tracker import InstallationStage
from backend.logging_config import get_logger
from backend.models import VersionInfo, get_iso_timestamp
from backend.utils import ensure_directory, safe_filename

logger = get_logger(__name__)


class InstallationMixin:
    """Mix-in for installation and removal workflows."""

    def _open_install_log(self, prefix: str) -> Path:
        """Create/open an install log file for the current attempt."""
        timestamp = int(time.time())
        filename = f"{prefix}-{timestamp}.log"
        log_path = self.logs_dir / filename
        try:
            self._install_log_handle = open(log_path, "a", encoding="utf-8")
            self._current_install_log_path = log_path
            header = (
                f"{'='*30} INSTALL START {prefix} @ "
                f"{datetime.now(timezone.utc).isoformat()} {'='*30}\n"
            )
            self._install_log_handle.write(header)
            self._install_log_handle.flush()
        except (IOError, OSError) as exc:
            logger.warning(f"Unable to open install log at {log_path}: {exc}")
            self._install_log_handle = None
            self._current_install_log_path = log_path
        return log_path

    def _log_install(self, message: str) -> None:
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
                        logger.info("✓ Removed incomplete installation directory")
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
                    logger.info("✓ Removed incomplete installation directory")
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
                    logger.info("✓ Removed incomplete installation directory")
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
