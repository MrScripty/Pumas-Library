#!/usr/bin/env python3
"""
Version Manager for ComfyUI
Handles installation, switching, and launching of ComfyUI versions
"""

import json
import os
import shutil
import tarfile
import zipfile
import subprocess
import time
import re
from pathlib import Path
from typing import Optional, List, Callable, Dict, Tuple
from packaging.utils import canonicalize_name
from backend.models import (
    VersionsMetadata, VersionInfo, VersionConfig, DependencyStatus,
    GitHubRelease, get_iso_timestamp
)
from backend.metadata_manager import MetadataManager
from backend.github_integration import GitHubReleasesFetcher, DownloadManager
from backend.resource_manager import ResourceManager
from backend.utils import (
    ensure_directory, run_command, check_command_exists, get_directory_size,
    parse_requirements_file
)
from backend.installation_progress_tracker import (
    InstallationProgressTracker,
    InstallationStage
)


class VersionManager:
    """Manages ComfyUI version installation, switching, and launching"""

    def __init__(
        self,
        launcher_root: Path,
        metadata_manager: MetadataManager,
        github_fetcher: GitHubReleasesFetcher,
        resource_manager: ResourceManager
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

        # Directories
        self.versions_dir = self.launcher_root / "comfyui-versions"
        self.active_version_file = self.launcher_root / ".active-version"

        # Ensure versions directory exists
        ensure_directory(self.versions_dir)
        # Shared UV cache directory (persists across installs)
        self.uv_cache_dir = self.resource_manager.shared_dir / "uv"
        if ensure_directory(self.uv_cache_dir):
            print(f"Using UV cache directory at {self.uv_cache_dir}")

        # Initialize progress tracker (Phase 6.2.5b)
        cache_dir = metadata_manager.launcher_data_dir / "cache"
        self.progress_tracker = InstallationProgressTracker(cache_dir)

        # Cancellation flag (Phase 6.2.5d)
        self._cancel_installation = False
        self._installing_tag = None
        self._current_process = None  # Track active subprocess for immediate kill
        self._current_downloader = None  # Track active downloader for immediate cancel

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
            print("\n" + "=" * 60)
            print(f"⚠️  CANCELLATION REQUESTED for {self._installing_tag}")
            print("=" * 60)
            self._cancel_installation = True

            # Immediately cancel any active download
            if self._current_downloader:
                try:
                    print("→ Cancelling active download...")
                    self._current_downloader.cancel()
                    print("✓ Download cancelled")
                except Exception as e:
                    print(f"✗ Error cancelling download: {e}")

            # Immediately kill any running process and all its children
            if self._current_process:
                try:
                    pid = self._current_process.pid
                    print(f"→ Terminating subprocess (PID: {pid}) and all child processes...")

                    # Kill the entire process group to ensure all children are terminated
                    try:
                        if hasattr(os, 'killpg'):
                            # Send SIGTERM first for graceful shutdown
                            os.killpg(os.getpgid(pid), 15)
                            # Wait a moment
                            time.sleep(0.5)
                            # Check if still alive - need to check if process object is still valid
                            try:
                                still_alive = self._current_process.poll() is None
                            except Exception:
                                # Process object might be invalid, assume it's dead
                                still_alive = False

                            if still_alive:
                                # Force kill with SIGKILL
                                os.killpg(os.getpgid(pid), 9)
                        else:
                            self._current_process.kill()

                        # Wait for process to die
                        try:
                            self._current_process.wait(timeout=2)
                        except Exception:
                            pass  # Process might already be gone
                        print("✓ All processes terminated")
                    except ProcessLookupError:
                        print("✓ Process already terminated")
                    except Exception as kill_error:
                        print(f"✗ Error killing process group: {kill_error}")
                        # Fallback: try killing just the main process
                        try:
                            self._current_process.kill()
                            self._current_process.wait(timeout=1)
                            print("✓ Main process killed")
                        except Exception:
                            pass
                except Exception as e:
                    print(f"✗ Error terminating subprocess: {e}")

            self.progress_tracker.set_error("Installation cancelled by user")
            print("=" * 60)
            print("✓ INSTALLATION CANCELLED")
            print("=" * 60 + "\n")
            return True
        return False

    def get_available_releases(
        self,
        force_refresh: bool = False,
        collapse: bool = True,
        include_prerelease: bool = True
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
                releases,
                include_prerelease=include_prerelease
            )
        return releases

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed version tags (validated against actual directories)

        Returns:
            List of version tags that are both in metadata and have valid directories
        """
        versions_metadata = self.metadata_manager.load_versions()
        metadata_versions = set(versions_metadata.get('installed', {}).keys())

        # Verify each version actually exists on disk
        validated_versions = []
        needs_cleanup = False

        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                # Version is in metadata but incomplete/missing on disk
                print(f"Warning: Version {tag} is incomplete or missing, removing from metadata")
                needs_cleanup = True

        # Clean up metadata if we found incomplete versions
        # NOTE: This ONLY modifies the 'installed' dict in versions.json
        # It does NOT touch the GitHub releases cache or any other cache files
        if needs_cleanup:
            for tag in metadata_versions:
                if tag not in validated_versions:
                    del versions_metadata['installed'][tag]
            self.metadata_manager.save_versions(versions_metadata)
            print(f"✓ Cleaned up metadata - removed {len(metadata_versions) - len(validated_versions)} incomplete version(s)")

        return validated_versions

    def validate_installations(self) -> Dict[str, any]:
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
        metadata_versions = set(versions_metadata.get('installed', {}).keys())

        validated_versions = []
        removed_versions = []

        # Check 1: Validate versions in metadata
        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                removed_versions.append(tag)
                print(f"Warning: Version {tag} in metadata but directory incomplete/missing")

        # Check 2: Look for orphaned directories (no metadata = incomplete install)
        if self.versions_dir.exists():
            for version_dir in self.versions_dir.iterdir():
                if version_dir.is_dir():
                    tag = version_dir.name
                    # If directory exists but NOT in metadata, it's an incomplete install
                    if tag not in metadata_versions:
                        removed_versions.append(tag)
                        print(f"Warning: Found incomplete installation directory: {tag} (not in metadata)")
                        # Remove the orphaned directory
                        try:
                            shutil.rmtree(version_dir)
                            print(f"✓ Removed incomplete installation directory: {tag}")
                        except Exception as e:
                            print(f"Error removing {tag}: {e}")

        # Clean up metadata if we found incomplete versions in metadata
        if any(tag in metadata_versions for tag in removed_versions):
            for tag in removed_versions:
                if tag in versions_metadata['installed']:
                    del versions_metadata['installed'][tag]
            self.metadata_manager.save_versions(versions_metadata)
            print(f"✓ Cleaned up {len(removed_versions)} incomplete installation(s): {', '.join(removed_versions)}")

        return {
            'had_invalid': len(removed_versions) > 0,
            'removed': removed_versions,
            'valid': validated_versions
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
            version_path / "venv",     # Virtual environment
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
        return versions_metadata.get('installed', {}).get(tag)

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
        Get currently active version tag

        Priority order:
        1. defaultVersion (if set and installed)
        2. lastSelectedVersion (if installed)
        3. newest installed version
        4. None (nothing installed)

        Returns:
            Active version tag or None
        """
        versions_metadata = self.metadata_manager.load_versions()
        installed_versions = self.get_installed_versions()

        # If no versions installed, return None
        if not installed_versions:
            return None

        # Priority 1: Use default version if set and installed
        default_version = versions_metadata.get('defaultVersion')
        if default_version and default_version in installed_versions:
            return default_version

        # Priority 2: Use last selected version if installed
        last_selected = versions_metadata.get('lastSelectedVersion')
        if last_selected and last_selected in installed_versions:
            return last_selected

        # Priority 3: Use newest installed version
        # Versions are typically in format v1.2.3 or similar
        # Sort in reverse to get newest first
        sorted_versions = sorted(installed_versions, reverse=True)
        return sorted_versions[0] if sorted_versions else None

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
            print(f"Version {tag} is not installed")
            return False

        # Validate symlinks
        print(f"Validating symlinks for {tag}...")
        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)

        if repair_report['broken']:
            print(f"Warning: Found {len(repair_report['broken'])} broken symlinks")
            print(f"Repaired: {len(repair_report['repaired'])}, Removed: {len(repair_report['removed'])}")

        # Update active version file
        try:
            self.active_version_file.write_text(tag)
        except Exception as e:
            print(f"Error writing active version file: {e}")
            return False

        # Update metadata
        versions_metadata = self.metadata_manager.load_versions()
        versions_metadata['lastSelectedVersion'] = tag

        self.metadata_manager.save_versions(versions_metadata)

        print(f"✓ Activated version: {tag}")
        return True

    def get_default_version(self) -> Optional[str]:
        """
        Get the default version set in metadata.
        """
        versions_metadata = self.metadata_manager.load_versions()
        return versions_metadata.get('defaultVersion')

    def set_default_version(self, tag: Optional[str]) -> bool:
        """
        Set a version as default (or clear if tag is None).
        """
        versions_metadata = self.metadata_manager.load_versions()
        installed = versions_metadata.get('installed', {})

        if tag is not None and tag not in installed:
            print(f"Cannot set default to {tag}: not installed")
            return False

        versions_metadata['defaultVersion'] = tag
        self.metadata_manager.save_versions(versions_metadata)
        print(f"✓ Default version set to: {tag}")
        return True

    def install_version(
        self,
        tag: str,
        progress_callback: Optional[Callable[[str, int, int], None]] = None
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
            print(f"Version {tag} is already installed")
            return False

        # Get release info
        release = self.github_fetcher.get_release_by_tag(tag)
        if not release:
            print(f"Release {tag} not found")
            return False

        print(f"Installing ComfyUI {tag}...")

        version_path = self.versions_dir / tag
        if version_path.exists():
            print(f"Version directory already exists: {version_path}")
            return False

        try:
            # Reset cancellation flag and set installing tag
            self._cancel_installation = False
            self._installing_tag = tag

            # Initialize progress tracking
            self.progress_tracker.start_installation(tag)

            # Step 1: Download release
            self.progress_tracker.update_stage(InstallationStage.DOWNLOAD, 0, f"Downloading {tag}")
            if progress_callback:
                progress_callback("Downloading release...", 1, 5)

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            download_url = release.get('zipball_url') or release.get('tarball_url')
            if not download_url:
                error_msg = "No download URL found in release"
                print(error_msg)
                self.progress_tracker.set_error(error_msg)
                return False

            # Determine archive type
            is_zip = 'zipball' in download_url

            # Download to temporary file
            download_dir = self.launcher_root / "temp"
            ensure_directory(download_dir)

            archive_ext = '.zip' if is_zip else '.tar.gz'
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
                    downloaded,
                    total_bytes,
                    effective_speed
                )

            try:
                success = downloader.download_with_retry(
                    download_url,
                    archive_path,
                    progress_callback=on_download_progress
                )

                if not success:
                    error_msg = "Download failed"
                    print(error_msg)
                    self.progress_tracker.set_error(error_msg)
                    return False
            finally:
                self._current_downloader = None  # Clear reference

            # Get archive size
            archive_size = archive_path.stat().st_size
            self.progress_tracker.update_download_progress(archive_size, archive_size)
            self.progress_tracker.add_completed_item(archive_path.name, 'archive', archive_size)

            # Check for cancellation after download
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled after download")

            # Step 2: Extract archive
            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 0, "Extracting archive")
            if progress_callback:
                progress_callback("Extracting archive...", 2, 5)

            print(f"Extracting {archive_path.name}...")
            temp_extract_dir = download_dir / f"extract-{tag}"
            ensure_directory(temp_extract_dir)

            if is_zip:
                with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                    zip_ref.extractall(temp_extract_dir)
            else:
                with tarfile.open(archive_path, 'r:gz') as tar_ref:
                    tar_ref.extractall(temp_extract_dir)

            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 100, "Extraction complete")

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

            # Step 3: Create venv with UV
            self.progress_tracker.update_stage(InstallationStage.VENV, 0, "Creating virtual environment")
            if progress_callback:
                progress_callback("Creating virtual environment...", 3, 5)

            if not self._create_venv(version_path):
                error_msg = "Failed to create virtual environment"
                print(error_msg)
                self.progress_tracker.set_error(error_msg)
                shutil.rmtree(version_path)
                return False

            self.progress_tracker.update_stage(InstallationStage.VENV, 100, "Virtual environment created")

            # Check for cancellation
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled by user")

            # Step 4: Install dependencies with progress tracking
            self.progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 0, "Installing dependencies")
            if progress_callback:
                progress_callback("Installing dependencies...", 4, 5)

            deps_success = self._install_dependencies_with_progress(tag)
            if not deps_success:
                print("Warning: Dependency installation had errors")
                self.progress_tracker.set_error("Dependency installation failed")
                # Continue to finish setup, but installation will be marked as failed

            # Step 5: Setup symlinks
            self.progress_tracker.update_stage(InstallationStage.SETUP, 0, "Setting up symlinks")
            if progress_callback:
                progress_callback("Setting up symlinks...", 5, 5)

            self.resource_manager.setup_version_symlinks(tag)
            self.progress_tracker.update_stage(InstallationStage.SETUP, 100, "Setup complete")

            # Update metadata
            version_info: VersionInfo = {
                'path': str(version_path.relative_to(self.launcher_root)),
                'installedDate': get_iso_timestamp(),
                'pythonVersion': self._get_python_version(version_path),
                'releaseTag': tag
            }

            versions_metadata = self.metadata_manager.load_versions()
            if 'installed' not in versions_metadata:
                versions_metadata['installed'] = {}

            versions_metadata['installed'][tag] = version_info
            self.metadata_manager.save_versions(versions_metadata)

            # Mark installation as complete
            self.progress_tracker.complete_installation(deps_success)

            if deps_success:
                print(f"✓ Successfully installed {tag}")
            else:
                print(f"Installation completed with dependency errors for {tag}")
            return deps_success

        except InterruptedError as e:
            # Installation was cancelled by user
            error_msg = str(e)
            print(f"✓ {error_msg}")
            self.progress_tracker.set_error(error_msg)
            self.progress_tracker.complete_installation(False)

            # Clean up cancelled installation
            if version_path.exists():
                print(f"Cleaning up cancelled installation: {version_path}")
                try:
                    shutil.rmtree(version_path)
                    print(f"✓ Removed incomplete installation directory")
                except Exception as cleanup_error:
                    print(f"Warning: Failed to clean up directory: {cleanup_error}")
            return False
        except Exception as e:
            error_msg = f"Error installing version {tag}: {e}"
            print(error_msg)
            self.progress_tracker.set_error(error_msg)
            self.progress_tracker.complete_installation(False)

            # Clean up on failure
            if version_path.exists():
                print(f"Cleaning up failed installation: {version_path}")
                try:
                    shutil.rmtree(version_path)
                    print(f"✓ Removed incomplete installation directory")
                except Exception as cleanup_error:
                    print(f"Warning: Failed to clean up directory: {cleanup_error}")
            return False
        finally:
            # Reset installation state
            self._installing_tag = None
            self._cancel_installation = False

            # Clear progress state after a short delay (allow UI to read final state)
            time.sleep(2)
            self.progress_tracker.clear_state()

    def _build_uv_env(self) -> Dict[str, str]:
        """
        Build environment variables for UV commands, ensuring cache is shared
        """
        env = os.environ.copy()
        if ensure_directory(self.uv_cache_dir):
            env['UV_CACHE_DIR'] = str(self.uv_cache_dir)
            # Also point pip to a persistent cache alongside UV's cache
            pip_cache = self.uv_cache_dir / "pip-cache"
            try:
                pip_cache.mkdir(parents=True, exist_ok=True)
                env['PIP_CACHE_DIR'] = str(pip_cache)
            except Exception:
                pass
        else:
            print(f"Warning: Unable to create UV cache directory at {self.uv_cache_dir}")
        env['UV_LINK_MODE'] = env.get('UV_LINK_MODE', 'copy')
        return env

    def _get_global_required_packages(self) -> list[str]:
        """
        Packages that must be installed in every ComfyUI venv regardless of requirements.txt
        """
        return ["setproctitle"]

    def _create_venv(self, version_path: Path) -> bool:
        """
        Create virtual environment for a version using UV

        Args:
            version_path: Path to version directory

        Returns:
            True if successful
        """
        # Check if UV is installed
        if not check_command_exists('uv'):
            print("UV package manager not found. Attempting to install...")
            # Try to install UV
            success, stdout, stderr = run_command(
                ['pip', 'install', 'uv'],
                timeout=60
            )
            if not success:
                print("Failed to install UV. Please install it manually:")
                print("  pip install uv")
                return False

        venv_path = version_path / "venv"

        print(f"Creating virtual environment with UV...")
        uv_env = self._build_uv_env()
        success, stdout, stderr = run_command(
            ['uv', 'venv', str(venv_path)],
            timeout=120,
            env=uv_env
        )

        if not success:
            print(f"Failed to create venv: {stderr}")
            return False

        print("✓ Virtual environment created")
        return True

    def _get_python_version(self, version_path: Path) -> str:
        """
        Get Python version for a version's venv

        Args:
            version_path: Path to version directory

        Returns:
            Python version string or "unknown"
        """
        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            return "unknown"

        success, stdout, stderr = run_command(
            [str(venv_python), '--version'],
            timeout=5
        )

        if success:
            # Output is like "Python 3.11.7"
            return stdout.strip()

        return "unknown"

    def check_dependencies(self, tag: str) -> DependencyStatus:
        """
        Check dependency installation status for a version

        Args:
            tag: Version tag

        Returns:
            DependencyStatus with installed/missing packages
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            return {
                'installed': [],
                'missing': [],
                'requirementsFile': None
            }

        requirements_file = version_path / "requirements.txt"
        requirements_file_rel = str(requirements_file.relative_to(self.launcher_root)) if requirements_file.exists() else None

        requirements = parse_requirements_file(requirements_file) if requirements_file.exists() else {}

        # Parse requirements (split required vs optional)
        optional_requirements: set[str] = set()
        if requirements_file.exists():
            try:
                optional_mode = False
                with open(requirements_file, 'r') as f:
                    for line in f:
                        raw = line.strip()
                        if not raw:
                            continue
                        if raw.startswith('#'):
                            if raw.lower().startswith("#non essential dependencies"):
                                optional_mode = True
                            continue
                        if raw.startswith('-'):
                            continue
                        if optional_mode:
                            pkg = raw.split('==')[0].split('>=')[0].split('<=')[0].split('<')[0].split('>')[0].split('@')[0].strip()
                            if pkg:
                                optional_requirements.add(canonicalize_name(pkg))
            except Exception as e:
                print(f"Warning: could not parse optional dependencies in {requirements_file}: {e}")

        # Add global required packages (e.g., setproctitle for process naming)
        global_required = self._get_global_required_packages()
        existing_canon = {canonicalize_name(pkg) for pkg in requirements}
        for pkg in global_required:
            canon = canonicalize_name(pkg)
            if canon not in existing_canon:
                requirements[pkg] = ""
                existing_canon.add(canon)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            # No venv, all packages are missing
            return {
                'installed': [],
                'missing': list(requirements.keys()),
                'requirementsFile': requirements_file_rel
            }

        # Check which packages are installed
        installed: list[str] = []
        missing: list[str] = []

        installed_names = self._get_installed_package_names(tag, venv_python)
        if installed_names is None:
            # If inspection fails, don't block launch; assume satisfied
            print(f"Warning: Could not inspect installed packages for {tag}, assuming dependencies are present")
            return {
                'installed': list(requirements.keys()),
                'missing': [],
                'requirementsFile': requirements_file_rel
            }

        for package in requirements.keys():
            canon = canonicalize_name(package)
            if canon in optional_requirements:
                continue  # optional; ignore for blocking
            if canon in installed_names:
                installed.append(package)
            else:
                missing.append(package)

        return {
            'installed': installed,
            'missing': missing,
            'requirementsFile': requirements_file_rel
        }

    def _get_installed_package_names(self, tag: str, venv_python: Path) -> Optional[set[str]]:
        """
        Inspect installed packages in the version venv.

        Returns:
            Set of canonicalized package names, or None if inspection failed.
        """
        installed_names: set[str] = set()
        uv_env = self._build_uv_env()
        errors: list[str] = []

        # Prefer uv pip list JSON (works even if pip isn't in the venv)
        success, stdout, stderr = run_command(
            ['uv', 'pip', 'list', '--format=json', '--python', str(venv_python)],
            timeout=30,
            env=uv_env
        )

        if success:
            try:
                import json as _json
                parsed = _json.loads(stdout)
                installed_names = {
                    canonicalize_name(pkg.get('name', ''))
                    for pkg in parsed
                    if pkg.get('name')
                }
                return installed_names
            except Exception as e:
                errors.append(f"uv json parse: {e}")
                print(f"Error parsing uv pip list JSON for {tag}: {e}")
        else:
            errors.append(f"uv json: {stderr}")

        # Fallback to freeze format
        success, stdout, stderr = run_command(
            ['uv', 'pip', 'list', '--format=freeze', '--python', str(venv_python)],
            timeout=30,
            env=uv_env
        )
        if success:
            for line in stdout.splitlines():
                line = line.strip()
                if not line:
                    continue
                pkg = line.split('==')[0].split('@')[0].strip()
                if pkg:
                    installed_names.add(canonicalize_name(pkg))
            return installed_names
        else:
            errors.append(f"uv freeze: {stderr}")

        # Last resort: try venv's pip directly if available
        success, stdout, stderr = run_command(
            [str(venv_python), '-m', 'pip', 'list', '--format=json'],
            timeout=30
        )

        if success:
            try:
                import json as _json
                parsed = _json.loads(stdout)
                installed_names = {
                    canonicalize_name(pkg.get('name', ''))
                    for pkg in parsed
                    if pkg.get('name')
                }
                return installed_names
            except Exception as e:
                errors.append(f"pip json parse: {e}")
                print(f"Error parsing pip list JSON for {tag}: {e}")
        else:
            errors.append(f"pip json: {stderr}")

        success, stdout, stderr = run_command(
            [str(venv_python), '-m', 'pip', 'list', '--format=freeze'],
            timeout=30
        )
        if success:
            for line in stdout.splitlines():
                line = line.strip()
                if not line:
                    continue
                pkg = line.split('==')[0].split('@')[0].strip()
                if pkg:
                    installed_names.add(canonicalize_name(pkg))
            return installed_names
        else:
            errors.append(f"pip freeze: {stderr}")

        error_msg = '; '.join([e for e in errors if e]) or "unknown error"
        print(f"Warning: dependency inspection failed for {tag}: {error_msg}")
        return None

    def _slugify_tag(self, tag: str) -> str:
        """Safe slug for filenames"""
        if not tag:
            return "comfyui"
        safe = ''.join(c if c.isalnum() or c in ('-', '_') else '-' for c in tag.strip().lower())
        # Drop a leading 'v' (v0.5.1 -> 0-5-1) to match requested naming
        if safe.startswith('v') and len(safe) > 1:
            safe = safe[1:]
        safe = re.sub(r'-+', '-', safe).strip('-_')
        return safe or "comfyui"

    def _ensure_version_run_script(self, tag: str, version_path: Path) -> Path:
        """
        Ensure a version-specific run.sh exists that also opens the UI.

        Returns:
            Path to the run script.
        """
        slug = self._slugify_tag(tag)
        script_path = version_path / f"run_{slug}.sh"
        profile_dir = self.metadata_manager.launcher_data_dir / "profiles" / slug
        profile_dir.mkdir(parents=True, exist_ok=True)

        content = f"""#!/bin/bash
set -euo pipefail

VERSION_DIR="{version_path}"
VENV_PATH="$VERSION_DIR/venv"
MAIN_PY="$VERSION_DIR/main.py"
PID_FILE="$VERSION_DIR/comfyui.pid"
URL="http://127.0.0.1:8188"
WINDOW_CLASS="ComfyUI-{slug}"
PROFILE_DIR="{profile_dir}"
SERVER_START_DELAY=8
SERVER_PID=""

log() {{
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}}

stop_previous_instance() {{
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || echo "")
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            log "Stopping previous server (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi
}}

close_existing_app_window() {{
    if command -v wmctrl >/dev/null 2>&1; then
        local wins
        wins=$(wmctrl -l -x 2>/dev/null | grep -i "$WINDOW_CLASS" | awk '{{print $1}}' || true)
        if [[ -n "$wins" ]]; then
            for win_id in $wins; do
                wmctrl -i -c "$win_id" || true
            done
            sleep 1
        fi
    fi
}}

start_comfyui() {{
    if [[ ! -x "$VENV_PATH/bin/python" ]]; then
        echo "Missing virtual environment for {tag}"
        exit 1
    fi

    cd "$VERSION_DIR"
    log "Starting ComfyUI {tag}..."
    "$VENV_PATH/bin/python" "$MAIN_PY" --enable-manager &
    SERVER_PID=$!
    echo "$SERVER_PID" > "$PID_FILE"
}}

open_app() {{
    if command -v brave-browser >/dev/null 2>&1; then
        mkdir -p "$PROFILE_DIR"
        log "Opening Brave window for {tag}..."
        brave-browser --app="$URL" --new-window --user-data-dir="$PROFILE_DIR" --class="$WINDOW_CLASS" >/dev/null 2>&1 &
    else
        log "Opening default browser..."
        xdg-open "$URL" >/dev/null 2>&1 &
    fi
}}

cleanup() {{
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
}}

trap cleanup EXIT

stop_previous_instance
close_existing_app_window
start_comfyui

log "Waiting $SERVER_START_DELAY seconds for server to start..."
sleep "$SERVER_START_DELAY"
open_app

wait $SERVER_PID
"""
        try:
            script_path.write_text(content)
            script_path.chmod(0o755)
        except Exception as e:
            print(f"Warning: could not write run.sh for {tag}: {e}")
        return script_path

    def install_dependencies(
        self,
        tag: str,
        progress_callback: Optional[Callable[[str], None]] = None
    ) -> bool:
        """
        Install dependencies for a version

        Args:
            tag: Version tag
            progress_callback: Optional callback for progress messages

        Returns:
            True if successful
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            print(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        if not requirements_file.exists():
            print(f"No requirements.txt found for {tag} (will still install global dependencies)")

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return False

        print(f"Installing dependencies for {tag}...")

        if progress_callback:
            progress_callback("Installing Python packages...")

        global_required = self._get_global_required_packages()
        uv_env = self._build_uv_env()
        # Use UV to install requirements plus global packages
        install_cmd = [
            'uv', 'pip', 'install',
            '--python', str(venv_python)
        ]
        if requirements_file.exists():
            install_cmd += ['-r', str(requirements_file)]
        install_cmd += global_required

        success, stdout, stderr = run_command(install_cmd, timeout=600, env=uv_env)

        if success:
            print("✓ Dependencies installed successfully")
            if stdout:
                print(stdout)
            return True

        print(f"Error installing dependencies with uv: {stderr}")
        print("Attempting pip fallback...")

        pip_cmd = [
            str(venv_python),
            "-m",
            "pip",
            "install",
        ]
        if requirements_file.exists():
            pip_cmd += ["-r", str(requirements_file)]
        pip_cmd += global_required

        success, stdout, stderr = run_command(pip_cmd, timeout=900, env=uv_env)

        if success:
            print("✓ Dependencies installed successfully via pip fallback")
            if stdout:
                print(stdout)
            return True

        print(f"Dependency installation failed via uv and pip: {stderr}")
        return False

    def _install_dependencies_with_progress(self, tag: str) -> bool:
        """
        Install dependencies with detailed progress tracking (Phase 6.2.5b)

        Args:
            tag: Version tag

        Returns:
            True if successful
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            print(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return False

        # Parse requirements to get package list
        requirements = parse_requirements_file(requirements_file) if requirements_file.exists() else {}
        global_required = self._get_global_required_packages()
        # Add global packages that are not already in requirements
        existing_canon = {canonicalize_name(pkg) for pkg in requirements}
        extra_global = []
        for pkg in global_required:
            canon = canonicalize_name(pkg)
            if canon not in existing_canon:
                extra_global.append(pkg)
                existing_canon.add(canon)

        package_entries = list(requirements.items()) + [(pkg, "") for pkg in extra_global]
        package_count = len(package_entries)

        print(f"Installing {package_count} dependencies for {tag}...")

        # Update progress tracker with dependency count
        current_state = self.progress_tracker.get_current_state()
        if current_state:
            current_state['dependency_count'] = package_count
            self.progress_tracker.update_dependency_progress(
                "Preparing...",
                0,
                package_count
        )

        # Use UV to install requirements
        # Use Popen to allow immediate cancellation
        print("Starting dependency installation...")
        uv_env = self._build_uv_env()
        cache_start_size = get_directory_size(self.uv_cache_dir) if self.uv_cache_dir.exists() else 0
        last_cache_size = cache_start_size
        last_sample_time = time.time()
        uv_stdout = ""
        uv_stderr = ""
        uv_cmd = [
            'uv', 'pip', 'install',
            '--python', str(venv_python)
        ]
        if requirements_file.exists():
            uv_cmd += ['-r', str(requirements_file)]
        uv_cmd += extra_global

        self._current_process = subprocess.Popen(
            uv_cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            # Create new process group so we can kill the entire tree
            preexec_fn=os.setsid if hasattr(os, 'setsid') else None,
            env=uv_env
        )

        try:
            # Wait for process to complete, checking cancellation flag
            # The cancel_installation() method will kill the process immediately
            while self._current_process.poll() is None:
                time.sleep(0.1)  # Check every 100ms

                # Update approximate download speed based on UV cache growth
                now = time.time()
                if now - last_sample_time >= 0.75:
                    current_cache_size = get_directory_size(self.uv_cache_dir) if self.uv_cache_dir.exists() else 0
                    bytes_since_last = current_cache_size - last_cache_size
                    elapsed = now - last_sample_time
                    speed = bytes_since_last / elapsed if elapsed > 0 else None

                    total_downloaded = max(current_cache_size - cache_start_size, 0)
                    self.progress_tracker.update_download_progress(
                        total_downloaded,
                        None,
                        speed
                    )

                    last_cache_size = current_cache_size
                    last_sample_time = now

                # Check if process was killed by cancel_installation()
                if self._cancel_installation:
                    raise InterruptedError("Installation cancelled during dependency installation")

            # Process completed, get output
            uv_stdout, uv_stderr = self._current_process.communicate()
            success = self._current_process.returncode == 0

        except InterruptedError:
            raise  # Re-raise to be caught by install_version
        except Exception as e:
            print(f"Error during dependency installation: {e}")
            if self._current_process:
                # Kill entire process group
                try:
                    if hasattr(os, 'killpg'):
                        os.killpg(os.getpgid(self._current_process.pid), 9)
                    else:
                        self._current_process.kill()
                except Exception:
                    pass
            return False
        finally:
            self._current_process = None  # Clear process reference

        if success:
            # Mark all dependencies as completed
            for i, (package, version_spec) in enumerate(package_entries, 1):
                self.progress_tracker.update_dependency_progress(
                    f"{package}{version_spec}",
                    i,
                    package_count
                )
                self.progress_tracker.add_completed_item(package, 'package')

            print("✓ Dependencies installed successfully")
            if uv_stdout:
                print(uv_stdout)
            return True
        else:
            print(f"Error installing dependencies with uv: {uv_stderr}")
            self.progress_tracker.set_error("Dependency installation failed with uv, attempting pip fallback")

            # Fallback: use venv's pip directly (still reuses cache via env)
            print("Attempting fallback install with pip...")
            pip_cmd = [
                str(venv_python),
                "-m",
                "pip",
                "install",
            ]
            if requirements_file.exists():
                pip_cmd += ["-r", str(requirements_file)]
            pip_cmd += extra_global

            success, stdout, stderr = run_command(pip_cmd, timeout=900, env=uv_env)

            if success:
                for i, (package, version_spec) in enumerate(package_entries, 1):
                    self.progress_tracker.update_dependency_progress(
                        f"{package}{version_spec}",
                        i,
                        package_count
                    )
                    self.progress_tracker.add_completed_item(package, 'package')
                print("✓ Dependencies installed successfully via pip fallback")
                if stdout:
                    print(stdout)
                return True

            error_msg = f"Dependency installation failed via uv and pip. uv stderr: {uv_stderr[:500]} pip stderr: {stderr[:500]}"
            print(error_msg)
            self.progress_tracker.set_error(error_msg)
            return False

    def remove_version(self, tag: str) -> bool:
        """
        Remove an installed version

        Args:
            tag: Version tag to remove

        Returns:
            True if successful
        """
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return False

        # Check if it's the active version
        if self.get_active_version() == tag:
            print(f"Cannot remove active version {tag}")
            print("Please switch to a different version first")
            return False

        version_path = self.versions_dir / tag

        try:
            # Remove directory
            print(f"Removing {tag}...")
            shutil.rmtree(version_path)

            # Update metadata
            versions_metadata = self.metadata_manager.load_versions()

            if tag in versions_metadata.get('installed', {}):
                del versions_metadata['installed'][tag]

            if versions_metadata.get('defaultVersion') == tag:
                versions_metadata['defaultVersion'] = None

            self.metadata_manager.save_versions(versions_metadata)

            print(f"✓ Removed version {tag}")
            return True

        except Exception as e:
            print(f"Error removing version {tag}: {e}")
            return False

    def launch_version(
        self,
        tag: str,
        extra_args: Optional[List[str]] = None
    ) -> Tuple[bool, Optional[subprocess.Popen]]:
        """
        Launch a ComfyUI version

        Args:
            tag: Version tag to launch
            extra_args: Optional extra arguments for main.py

        Returns:
            Tuple of (success, process) - process is None if launch failed
        """
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return (False, None)

        # Set as active version
        if not self.set_active_version(tag):
            print("Failed to activate version")
            return (False, None)

        # Check dependencies
        dep_status = self.check_dependencies(tag)
        if dep_status['missing']:
            print(f"Missing dependencies detected for {tag}: {len(dep_status['missing'])}")
            print("Attempting to install missing dependencies before launch...")
            if not self.install_dependencies(tag):
                print("Failed to install dependencies, aborting launch.")
                return (False, None)
            # Re-check after install
            dep_status = self.check_dependencies(tag)
            if dep_status['missing']:
                print(f"Dependencies still missing after install: {dep_status['missing']}")
                return (False, None)

        # Validate symlinks
        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)
        if repair_report['broken']:
            print(f"Warning: Repaired {len(repair_report['repaired'])} broken symlinks")

        version_path = self.versions_dir / tag
        main_py = version_path / "main.py"

        if not main_py.exists():
            print(f"main.py not found in {tag}")
            return (False, None)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return (False, None)

        # Ensure run script exists so the UI opens consistently
        run_script = self._ensure_version_run_script(tag, version_path)
        log_file = version_path / "launcher-run.log"
        # Start fresh each launch so logs reflect the current run only
        try:
            log_file.unlink(missing_ok=True)
        except Exception as e:
            print(f"Warning: could not clear old log file {log_file} for {tag}: {e}")
        log_handle = None
        try:
            log_handle = open(log_file, "a")
        except Exception as e:
            print(f"Warning: could not open log file {log_file} for {tag}: {e}")

        cmd = ['bash', str(run_script)]
        if extra_args:
            cmd.extend(extra_args)

        print(f"Launching ComfyUI {tag}...")
        print(f"Command: {' '.join(cmd)}")

        try:
            # Launch as subprocess (non-blocking)
            process = subprocess.Popen(
                cmd,
                cwd=str(version_path),
                stdout=log_handle or subprocess.DEVNULL,
                stderr=log_handle or subprocess.DEVNULL,
                start_new_session=True
            )

            if log_handle:
                try:
                    log_handle.flush()
                except Exception:
                    pass

            # Give the script a moment to start and detect early failure
            time.sleep(1.0)
            if process.poll() is not None:
                exit_code = process.returncode
                print(f"ComfyUI {tag} launch script exited immediately with code {exit_code}")
                try:
                    if log_file.exists():
                        tail = log_file.read_text().splitlines()[-20:]
                        print("Launcher run log (last lines):")
                        for line in tail:
                            print(line)
                except Exception as log_err:
                    print(f"Unable to read launcher log for {tag}: {log_err}")
                return (False, None)

            print(f"✓ ComfyUI {tag} started (PID: {process.pid})")
            print("Use the returned process object to monitor or terminate")

            return (True, process)

        except Exception as e:
            print(f"Error launching ComfyUI: {e}")
            return (False, None)
        finally:
            if log_handle:
                try:
                    log_handle.close()
                except Exception:
                    pass

    def get_version_status(self) -> Dict[str, any]:
        """
        Get comprehensive status of all versions

        Returns:
            Dict with version status information
        """
        installed = self.get_installed_versions()
        active = self.get_active_version()

        status = {
            'installedCount': len(installed),
            'activeVersion': active,
            'defaultVersion': self.get_default_version(),
            'versions': {}
        }

        for tag in installed:
            version_info = self.get_version_info(tag)
            dep_status = self.check_dependencies(tag)

            status['versions'][tag] = {
                'info': version_info,
                'dependencies': dep_status,
                'isActive': tag == active
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

    print("=== ComfyUI Version Manager ===\n")

    # Get available releases
    print("Fetching available releases...")
    releases = version_mgr.get_available_releases()
    print(f"Found {len(releases)} releases\n")

    # Show installed versions
    installed = version_mgr.get_installed_versions()
    print(f"Installed versions: {len(installed)}")
    for tag in installed:
        info = version_mgr.get_version_info(tag)
        print(f"  - {tag} (installed: {info['installDate']})")

    # Show active version
    active = version_mgr.get_active_version()
    if active:
        print(f"\nActive version: {active}")
    else:
        print("\nNo active version")
