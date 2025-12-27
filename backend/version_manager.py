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
import urllib.request
from datetime import datetime, timezone
try:
    import psutil
except Exception:
    psutil = None
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional, List, Callable, Dict, Tuple
from packaging.utils import canonicalize_name
from packaging.version import Version
from packaging.specifiers import SpecifierSet
from backend.models import (
    VersionsMetadata, VersionInfo, VersionConfig, DependencyStatus,
    GitHubRelease, get_iso_timestamp
)
from backend.metadata_manager import MetadataManager
from backend.github_integration import GitHubReleasesFetcher, DownloadManager
from backend.resource_manager import ResourceManager
from backend.utils import (
    ensure_directory, run_command, get_directory_size,
    parse_requirements_file, safe_filename
)
from backend.installation_progress_tracker import (
    InstallationProgressTracker,
    InstallationStage
)
from backend.config import INSTALLATION
from backend.process_io_tracker import ProcessIOTracker


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
        self.logs_dir = self.metadata_manager.launcher_data_dir / "logs"
        ensure_directory(self.logs_dir)
        self.constraints_dir = self.metadata_manager.cache_dir / "constraints"
        ensure_directory(self.constraints_dir)
        self._constraints_cache_file = self.metadata_manager.cache_dir / "constraints-cache.json"
        self._constraints_cache: Dict[str, Dict[str, str]] = self._load_constraints_cache()
        self._pypi_release_cache: Dict[str, Dict[str, datetime]] = {}
        self.logs_dir = self.metadata_manager.launcher_data_dir / "logs"
        ensure_directory(self.logs_dir)
        self.constraints_dir = self.metadata_manager.cache_dir / "constraints"
        ensure_directory(self.constraints_dir)
        self._constraints_cache_file = self.metadata_manager.cache_dir / "constraints-cache.json"
        self._constraints_cache: Dict[str, Dict[str, str]] = self._load_constraints_cache()
        self._pypi_release_cache: Dict[str, Dict[str, list]] = {}

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
            print(f"Using pip cache directory at {self.pip_cache_dir}")
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
        except Exception as exc:
            print(f"Error writing active version file: {exc}")
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
            versions_metadata['lastSelectedVersion'] = tag
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
            versions_metadata.get('defaultVersion'),
            versions_metadata.get('lastSelectedVersion')
        ]

        for candidate in candidates:
            if candidate and candidate in installed_versions:
                self._set_active_version_state(candidate, update_last_selected=False)
                return candidate

        newest = sorted(installed_versions, reverse=True)[0]
        self._set_active_version_state(newest, update_last_selected=False)
        return newest

    def _load_constraints_cache(self) -> Dict[str, Dict[str, str]]:
        """Load cached per-tag constraints to avoid recomputation."""
        try:
            if self._constraints_cache_file.exists():
                with open(self._constraints_cache_file, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    return data if isinstance(data, dict) else {}
        except Exception as exc:
            print(f"Warning: unable to read constraints cache: {exc}")
        return {}

    def _save_constraints_cache(self):
        """Persist constraints cache safely."""
        try:
            tmp = self._constraints_cache_file.with_suffix('.tmp')
            with open(tmp, 'w', encoding='utf-8') as f:
                json.dump(self._constraints_cache, f, indent=2)
            tmp.replace(self._constraints_cache_file)
        except Exception as exc:
            print(f"Warning: unable to write constraints cache: {exc}")

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
        except Exception as exc:
            print(f"Warning: unable to open install log at {log_path}: {exc}")
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
            except Exception as exc:
                print(f"Warning: failed to write to install log: {exc}")

    def _get_process_io_bytes(self, pid: int, include_children: bool = True) -> Optional[int]:
        """
        Return total read+write bytes for a process (and optionally its children),
        using psutil as a proxy for download activity.
        """
        if not psutil or not pid:
            return None
        try:
            proc = psutil.Process(pid)
            procs = [proc]
            if include_children:
                procs += proc.children(recursive=True)
            total = 0
            for p in procs:
                try:
                    io = p.io_counters()
                    total += io.read_bytes + io.write_bytes
                except Exception:
                    continue
            return total
        except Exception:
            return None


    def _get_release_date(self, tag: str, release: Optional[GitHubRelease]) -> Optional[datetime]:
        """Return release date in UTC for a tag."""
        if not release:
            return None
        published_at = release.get("published_at")
        if not published_at:
            return None
        try:
            # GitHub timestamps are ISO with Z suffix
            if isinstance(published_at, str):
                ts = published_at.replace("Z", "+00:00")
                return datetime.fromisoformat(ts).astimezone(timezone.utc)
        except Exception as exc:
            print(f"Warning: could not parse release date for {tag}: {exc}")
        return None

    def _get_constraints_path(self, tag: str) -> Path:
        """Path to the cached constraints file for a tag."""
        safe_tag = safe_filename(tag) or "unknown"
        return self.constraints_dir / f"{safe_tag}.txt"

    def _fetch_pypi_versions(self, package: str) -> Dict[str, datetime]:
        """
        Fetch release versions and upload times for a package from PyPI.
        Returns mapping version->upload datetime (UTC).
        """
        canon = canonicalize_name(package)
        if canon in self._pypi_release_cache:
            return self._pypi_release_cache[canon]

        url = f"https://pypi.org/pypi/{package}/json"
        try:
            with urllib.request.urlopen(url, timeout=INSTALLATION.URL_FETCH_TIMEOUT_SEC) as resp:
                data = json.load(resp)
        except Exception as exc:
            print(f"Warning: failed to fetch PyPI data for {package}: {exc}")
            return {}

        releases = data.get("releases", {})
        result: Dict[str, datetime] = {}

        for version_str, files in releases.items():
            upload_times = []
            for file_entry in files or []:
                upload_time = file_entry.get("upload_time_iso_8601") or file_entry.get("upload_time")
                if upload_time:
                    try:
                        upload_times.append(datetime.fromisoformat(upload_time.replace("Z", "+00:00")).astimezone(timezone.utc))
                    except Exception:
                        continue
            if upload_times:
                result[version_str] = max(upload_times)

        self._pypi_release_cache[canon] = result
        return result

    def _select_version_for_date(self, package: str, spec: str, release_date: Optional[datetime]) -> Optional[str]:
        """
        Choose the newest version that satisfies the spec and is uploaded on/before the release date.
        If release_date is None, picks the newest version satisfying the spec.
        """
        releases = self._fetch_pypi_versions(package)
        if not releases:
            return None

        try:
            spec_set = SpecifierSet(spec) if spec else SpecifierSet()
        except Exception as exc:
            print(f"Warning: invalid specifier for {package} ({spec}): {exc}")
            spec_set = SpecifierSet()

        candidates = []
        for ver_str, uploaded_at in releases.items():
            try:
                ver = Version(ver_str)
            except Exception:
                continue
            if spec_set and ver not in spec_set:
                continue
            if release_date and uploaded_at and uploaded_at > release_date:
                continue
            candidates.append((ver, ver_str))

        if not candidates:
            return None

        candidates.sort()
        return candidates[-1][1]

    def _build_constraints_for_tag(self, tag: str, requirements_file: Path, release: Optional[GitHubRelease]) -> Optional[Path]:
        """
        Build a constraints file when requirements are not fully pinned.
        """
        constraints_path = self._get_constraints_path(tag)
        if constraints_path.exists():
            return constraints_path

        if not requirements_file.exists():
            return None

        requirements = parse_requirements_file(requirements_file)
        unpinned = {pkg: spec for pkg, spec in requirements.items() if not spec or not spec.startswith("==")}
        if not unpinned:
            return None  # already pinned

        release_date = self._get_release_date(tag, release)
        resolved: Dict[str, str] = {}

        for pkg, spec in unpinned.items():
            version_str = self._select_version_for_date(pkg, spec, release_date)
            if version_str:
                resolved[pkg] = f"=={version_str}"
            else:
                resolved[pkg] = spec or ""
                print(f"Warning: unable to resolve pinned version for {pkg} (spec: '{spec}')")

        combined: Dict[str, str] = {}
        for pkg, spec in requirements.items():
            combined[pkg] = resolved.get(pkg, spec if spec else "")

        try:
            with open(constraints_path, 'w', encoding='utf-8') as f:
                for pkg, spec in combined.items():
                    if spec:
                        f.write(f"{pkg}{spec}\n")
                    else:
                        f.write(f"{pkg}\n")
        except Exception as exc:
            print(f"Warning: failed to write constraints file for {tag}: {exc}")
            return None

        self._constraints_cache[tag] = combined
        self._save_constraints_cache()

        return constraints_path

    def _load_constraints_cache(self) -> Dict[str, Dict[str, str]]:
        """Load cached per-tag constraints to avoid recomputation."""
        try:
            if self._constraints_cache_file.exists():
                with open(self._constraints_cache_file, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    return data if isinstance(data, dict) else {}
        except Exception as exc:
            print(f"Warning: unable to read constraints cache: {exc}")
        return {}

    def _save_constraints_cache(self):
        """Persist constraints cache safely."""
        try:
            tmp = self._constraints_cache_file.with_suffix('.tmp')
            with open(tmp, 'w', encoding='utf-8') as f:
                json.dump(self._constraints_cache, f, indent=2)
            tmp.replace(self._constraints_cache_file)
        except Exception as exc:
            print(f"Warning: unable to write constraints cache: {exc}")

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
                            self._current_process.wait(timeout=INSTALLATION.SUBPROCESS_STOP_TIMEOUT_SEC)
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
                            self._current_process.wait(timeout=INSTALLATION.SUBPROCESS_KILL_TIMEOUT_SEC)
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
            print(f"Version {tag} is not installed")
            return False

        # Update active version state (persist as user choice)
        if not self._set_active_version_state(tag, update_last_selected=True):
            return False

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

    def _wait_for_server_ready(self, url: str, process: subprocess.Popen, log_file: Path, timeout: int = 90) -> Tuple[bool, Optional[str]]:
        """
        Poll the server URL until ready or process exits.

        Returns:
            (ready, error_message)
        """
        start = time.time()
        last_error = None
        while True:
            if process.poll() is not None:
                exit_code = process.returncode
                msg = f"ComfyUI process exited early with code {exit_code}"
                print(msg)
                return False, msg

            try:
                with urllib.request.urlopen(url, timeout=INSTALLATION.URL_QUICK_CHECK_TIMEOUT_SEC) as resp:
                    if resp.status == 200:
                        return True, None
            except Exception as exc:
                last_error = str(exc)

            if time.time() - start > timeout:
                return False, last_error or "Timed out waiting for server"

            time.sleep(0.5)

    def _tail_log(self, log_file: Path, lines: int = 20) -> List[str]:
        """Return the last N lines of a log file."""
        if not log_file.exists():
            return []
        try:
            content = log_file.read_text().splitlines()
            return content[-lines:]
        except Exception:
            return []

    def _open_frontend(self, url: str, slug: str):
        """Open the ComfyUI frontend in a browser profile (prefers Brave)."""
        profile_dir = self.metadata_manager.launcher_data_dir / "profiles" / slug
        profile_dir.mkdir(parents=True, exist_ok=True)
        try:
            if shutil.which("brave-browser"):
                subprocess.Popen(
                    [
                        "brave-browser",
                        f"--app={url}",
                        "--new-window",
                        f"--user-data-dir={profile_dir}",
                        f"--class=ComfyUI-{slug}"
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL
                )
            else:
                subprocess.Popen(
                    ["xdg-open", url],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL
                )
        except Exception as exc:
            print(f"Warning: failed to open frontend: {exc}")

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

            download_url = release.get('zipball_url') or release.get('tarball_url')
            if not download_url:
                error_msg = "No download URL found in release"
                print(error_msg)
                self._log_install(error_msg)
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
                    self._log_install(error_msg)
                    self.progress_tracker.set_error(error_msg)
                    return False
            finally:
                self._current_downloader = None  # Clear reference

            # Get archive size
            archive_size = archive_path.stat().st_size
            self.progress_tracker.update_download_progress(archive_size, archive_size)
            self.progress_tracker.add_completed_item(archive_path.name, 'archive', archive_size)
            self._log_install(f"Downloaded archive to {archive_path} ({archive_size} bytes)")

            # Check for cancellation after download
            if self._cancel_installation:
                raise InterruptedError("Installation cancelled after download")

            # Step 2: Extract archive
            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 0, "Extracting archive")
            if progress_callback:
                progress_callback("Extracting archive...", 2, 5)

            print(f"Extracting {archive_path.name}...")
            self._log_install(f"Extracting archive {archive_path.name}")
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

            # Step 3: Create venv with python3
            self.progress_tracker.update_stage(InstallationStage.VENV, 0, "Creating virtual environment")
            if progress_callback:
                progress_callback("Creating virtual environment...", 3, 5)

            if not self._create_venv(version_path):
                error_msg = "Failed to create virtual environment"
                print(error_msg)
                self._log_install(error_msg)
                self.progress_tracker.set_error(error_msg)
                shutil.rmtree(version_path)
                return False

            self.progress_tracker.update_stage(InstallationStage.VENV, 100, "Virtual environment created")
            self._log_install("Virtual environment created successfully")

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
                self._log_install("Dependency installation failed; aborting setup")
                self.progress_tracker.set_error("Dependency installation failed")
                # Clean up failed install
                if version_path.exists():
                    try:
                        shutil.rmtree(version_path)
                        print(f"✓ Removed incomplete installation directory")
                    except Exception as cleanup_error:
                        print(f"Warning: Failed to clean up directory: {cleanup_error}")
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
                self._log_install(f"✓ Successfully installed {tag}")
            else:
                print(f"Installation completed with dependency errors for {tag}")
                self._log_install(f"Installation completed with dependency errors for {tag}")
            return deps_success

        except InterruptedError as e:
            # Installation was cancelled by user
            error_msg = str(e)
            print(f"✓ {error_msg}")
            self._log_install(error_msg)
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
            self._log_install(error_msg)
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
            if self._install_log_handle:
                try:
                    self._install_log_handle.write(f"{'='*30} INSTALL END {tag} {'='*30}\n")
                    self._install_log_handle.close()
                except Exception:
                    pass
                self._install_log_handle = None
            # Preserve progress state so UI can read failure logs; it will be overwritten by the next install

    def _build_pip_env(self) -> Dict[str, str]:
        """
        Build environment variables for pip commands, ensuring cache is shared.
        """
        env = os.environ.copy()
        cache_dir = self.pip_cache_dir
        if ensure_directory(cache_dir):
            env['PIP_CACHE_DIR'] = str(cache_dir)
            self.active_pip_cache_dir = cache_dir
        else:
            print(f"Warning: Unable to create pip cache directory at {cache_dir}")
            self.active_pip_cache_dir = self.pip_cache_dir
        return env

    def _create_space_safe_requirements(
        self,
        tag: str,
        requirements_file: Optional[Path],
        constraints_path: Optional[Path]
    ) -> tuple[Optional[Path], Optional[Path]]:
        """
        Some tools can stumble on paths with spaces; copy requirements/constraints to a cache dir without spaces.
        """
        if not requirements_file and not constraints_path:
            return None, None

        safe_dir = self.metadata_manager.cache_dir / "requirements-safe"
        try:
            safe_dir.mkdir(parents=True, exist_ok=True)
        except Exception as exc:
            print(f"Warning: could not create safe requirements dir: {exc}")
            return requirements_file, constraints_path

        safe_tag = safe_filename(tag) or "req"
        safe_req = None
        safe_constraints = None

        try:
            if requirements_file and requirements_file.exists():
                safe_req = safe_dir / f"{safe_tag}-requirements.txt"
                shutil.copyfile(requirements_file, safe_req)
        except Exception as exc:
            print(f"Warning: could not copy requirements.txt to safe path: {exc}")
            safe_req = requirements_file

        try:
            if constraints_path and constraints_path.exists():
                safe_constraints = safe_dir / f"{safe_tag}-constraints.txt"
                shutil.copyfile(constraints_path, safe_constraints)
        except Exception as exc:
            print(f"Warning: could not copy constraints to safe path: {exc}")
            safe_constraints = constraints_path

        return safe_req or requirements_file, safe_constraints or constraints_path

    def _get_global_required_packages(self) -> list[str]:
        """
        Packages that must be installed in every ComfyUI venv regardless of requirements.txt
        """
        return ["setproctitle"]

    def _create_venv(self, version_path: Path) -> bool:
        """
        Create virtual environment for a version using python3.

        Args:
            version_path: Path to version directory

        Returns:
            True if successful
        """
        venv_path = version_path / "venv"

        print("Creating virtual environment with python3...")
        pip_env = self._build_pip_env()
        success, stdout, stderr = run_command(
            ['python3', '-m', 'venv', str(venv_path)],
            timeout=INSTALLATION.VENV_CREATION_TIMEOUT_SEC,
            env=pip_env
        )

        if not success:
            print(f"Failed to create venv: {stderr}")
            return False

        venv_python = venv_path / "bin" / "python"
        if venv_python.exists():
            run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
            run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

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
            timeout=INSTALLATION.SUBPROCESS_QUICK_TIMEOUT_SEC
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

        pip_env = self._build_pip_env()
        pip_ok, _stdout, _stderr = run_command(
            [str(venv_python), "-m", "pip", "--version"],
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env
        )
        if not pip_ok:
            run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
            run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        # Check which packages are installed
        installed: list[str] = []
        missing: list[str] = []

        installed_names = self._get_installed_package_names(tag, venv_python)
        if installed_names is None:
            print(f"Warning: Could not inspect installed packages for {tag}, treating dependencies as missing")
            installed_names = set()

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
        pip_env = self._build_pip_env()
        errors: list[str] = []

        success, stdout, stderr = run_command(
            [str(venv_python), '-m', 'pip', 'list', '--format=json'],
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env
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
            timeout=INSTALLATION.SUBPROCESS_STANDARD_TIMEOUT_SEC,
            env=pip_env
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

        Note: This generates the script dynamically with version-specific paths.
        A reference template is available at: scripts/templates/comfyui_run.sh

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
SERVER_START_DELAY="${{SERVER_START_DELAY:-8}}"
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

if [[ "${{SKIP_BROWSER:-0}}" != "1" ]]; then
    log "Waiting $SERVER_START_DELAY seconds for server to start..."
    sleep "$SERVER_START_DELAY"
    open_app
else
    log "Browser auto-open skipped by SKIP_BROWSER"
fi

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
            print(f"Virtual environment not found for {tag}; creating...")
            if not self._create_venv(version_path):
                return False
            venv_python = version_path / "venv" / "bin" / "python"
            if not venv_python.exists():
                print(f"Virtual environment not found for {tag}")
                return False

        print(f"Installing dependencies for {tag}...")

        if progress_callback:
            progress_callback("Installing Python packages...")

        global_required = self._get_global_required_packages()
        constraints_path = None
        try:
            release = self.github_fetcher.get_release_by_tag(tag)
        except Exception:
            release = None
        if requirements_file.exists():
            constraints_path = self._build_constraints_for_tag(tag, requirements_file, release)
            if constraints_path:
                print(f"Using pinned constraints for {tag}: {constraints_path}")
                self._log_install(f"Using constraints file: {constraints_path}")

        pip_env = self._build_pip_env()
        safe_req, safe_constraints = self._create_space_safe_requirements(
            tag,
            requirements_file if requirements_file.exists() else None,
            constraints_path
        )

        run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
        run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        install_cmd = [
            str(venv_python),
            "-m",
            "pip",
            "install",
        ]
        if safe_req:
            install_cmd += ["-r", str(Path(safe_req))]
        if safe_constraints:
            install_cmd += ["-c", str(Path(safe_constraints))]
        install_cmd += global_required

        success, stdout, stderr = run_command(
            install_cmd,
            timeout=INSTALLATION.PIP_FALLBACK_TIMEOUT_SEC,
            env=pip_env
        )

        if success:
            print("✓ Dependencies installed successfully")
            if stdout:
                print(stdout)
                self._log_install(stdout)
            return True

        print(f"Dependency installation failed: {stderr}")
        self._log_install(f"pip dependency install failed: {stderr}")
        return False

    def _install_dependencies_with_progress(self, tag: str) -> bool:
        """
        Install Python dependencies with real-time progress tracking.

        Uses pip and tracks download speed via process I/O counters and
        cache directory growth. Supports cancellation via the
        _cancel_installation flag.

        Process:
        1. Parses requirements.txt and constraints
        2. Ensures venv + pip are present
        3. Installs with pip while monitoring download progress
        4. Updates progress_tracker with real-time metrics

        Args:
            tag: Version tag being installed

        Returns:
            True if all dependencies installed successfully, False otherwise

        Raises:
            InterruptedError: If installation is cancelled by user via cancel_installation()

        Side Effects:
            - Updates self.progress_tracker state continuously
            - May create constraints cache files
            - Writes to installation log
            - Sets self._current_process during execution
        """
        version_path = self.versions_dir / tag

        if not version_path.exists():
            print(f"Version {tag} not found")
            return False

        requirements_file = version_path / "requirements.txt"

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}; creating...")
            if not self._create_venv(version_path):
                return False
            venv_python = version_path / "venv" / "bin" / "python"
            if not venv_python.exists():
                print(f"Virtual environment not found for {tag}")
                return False

        # Parse requirements to get package list
        requirements = parse_requirements_file(requirements_file) if requirements_file.exists() else {}
        global_required = self._get_global_required_packages()
        constraints_path = None
        try:
            release = self.github_fetcher.get_release_by_tag(tag)
        except Exception:
            release = None
        if requirements_file.exists():
            constraints_path = self._build_constraints_for_tag(tag, requirements_file, release)
            if constraints_path:
                self._log_install(f"Using constraints for {tag}: {constraints_path}")

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

        # Use pip to install requirements
        # Use Popen to allow immediate cancellation
        print("Starting dependency installation...")
        pip_env = self._build_pip_env()
        safe_req, safe_constraints = self._create_space_safe_requirements(
            tag,
            requirements_file if requirements_file.exists() else None,
            constraints_path
        )

        run_command([str(venv_python), "-m", "ensurepip", "--upgrade"], env=pip_env)
        run_command([str(venv_python), "-m", "pip", "install", "--upgrade", "pip"], env=pip_env)

        cache_dir = self.active_pip_cache_dir or self.pip_cache_dir
        pip_stdout = ""
        pip_stderr = ""
        pip_cmd = [
            str(venv_python),
            "-m",
            "pip",
            "install",
        ]
        if safe_req:
            pip_cmd += ["-r", str(Path(safe_req))]
        if safe_constraints:
            pip_cmd += ["-c", str(Path(safe_constraints))]
        pip_cmd += extra_global

        self._current_process = subprocess.Popen(
            pip_cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            # Create new process group so we can kill the entire tree
            preexec_fn=os.setsid if hasattr(os, 'setsid') else None,
            env=pip_env
        )

        # Initialize process I/O tracker for monitoring download progress
        io_tracker = ProcessIOTracker(
            pid=self._current_process.pid if self._current_process else None,
            cache_dir=cache_dir,
            io_bytes_getter=self._get_process_io_bytes
        )

        try:
            # Wait for process to complete, checking cancellation flag
            # The cancel_installation() method will kill the process immediately
            while self._current_process.poll() is None:
                time.sleep(0.1)  # Check every 100ms

                # Update download progress metrics
                if io_tracker.should_update(min_interval_sec=0.75):
                    downloaded, speed = io_tracker.get_download_metrics()

                    if downloaded is not None:
                        self.progress_tracker.update_download_progress(
                            downloaded,
                            None,
                            speed if speed is not None else 0
                        )

                # Check if process was killed by cancel_installation()
                if self._cancel_installation:
                    raise InterruptedError("Installation cancelled during dependency installation")

            # Process completed, get output
            pip_stdout, pip_stderr = self._current_process.communicate()
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
            if pip_stdout:
                print(pip_stdout)
                self._log_install(pip_stdout)
            return True

        error_msg = f"Dependency installation failed via pip: {pip_stderr[:500]}"
        print(error_msg)
        self._log_install(error_msg)
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
    ) -> Tuple[bool, Optional[subprocess.Popen], Optional[str], Optional[str], Optional[bool]]:
        """
        Launch a ComfyUI version with readiness detection.

        Returns:
            (success, process, log_path, error_message, ready)
        """
        if tag not in self.get_installed_versions():
            print(f"Version {tag} is not installed")
            return (False, None, None, "Version not installed", None)

        # Set as active version
        if not self.set_active_version(tag):
            print("Failed to activate version")
            return (False, None, None, "Failed to activate version", None)

        # Check dependencies
        dep_status = self.check_dependencies(tag)
        if dep_status['missing']:
            print(f"Missing dependencies detected for {tag}: {len(dep_status['missing'])}")
            print("Attempting to install missing dependencies before launch...")
            if not self.install_dependencies(tag):
                print("Failed to install dependencies, aborting launch.")
                return (False, None, None, "Dependencies missing", None)
            # Re-check after install
            dep_status = self.check_dependencies(tag)
            if dep_status['missing']:
                print(f"Dependencies still missing after install: {dep_status['missing']}")
                return (False, None, None, "Dependencies still missing after install", None)

        # Validate symlinks
        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)
        if repair_report['broken']:
            print(f"Warning: Repaired {len(repair_report['repaired'])} broken symlinks")

        version_path = self.versions_dir / tag
        main_py = version_path / "main.py"

        if not main_py.exists():
            print(f"main.py not found in {tag}")
            return (False, None, None, "main.py missing", None)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            print(f"Virtual environment not found for {tag}")
            return (False, None, None, "Virtual environment missing", None)

        # Ensure run script exists so the UI opens consistently
        run_script = self._ensure_version_run_script(tag, version_path)
        slug = self._slugify_tag(tag)
        url = "http://127.0.0.1:8188"

        log_file = self.logs_dir / f"launch-{slug}-{int(time.time())}.log"
        log_handle = None
        try:
            log_handle = open(log_file, "a", encoding="utf-8")
        except Exception as e:
            print(f"Warning: could not open log file {log_file} for {tag}: {e}")

        cmd = ['bash', str(run_script)]
        if extra_args:
            cmd.extend(extra_args)

        print(f"Launching ComfyUI {tag}...")
        print(f"Command: {' '.join(cmd)}")

        env = os.environ.copy()
        env["SKIP_BROWSER"] = "1"

        process = None
        try:
            process = subprocess.Popen(
                cmd,
                cwd=str(version_path),
                stdout=log_handle or subprocess.DEVNULL,
                stderr=log_handle or subprocess.DEVNULL,
                start_new_session=True,
                env=env
            )

            ready, ready_error = self._wait_for_server_ready(url, process, log_file)

            if ready:
                print(f"✓ ComfyUI {tag} reported ready (PID: {process.pid})")
                self._open_frontend(url, slug)
                return (True, process, str(log_file), None, True)

            # If process crashed or timeout
            tail = self._tail_log(log_file)
            if tail:
                print("Launch log tail:")
                for line in tail:
                    print(line)
            return (False, process if process and process.poll() is None else None, str(log_file), ready_error, False)

        except Exception as e:
            print(f"Error launching ComfyUI: {e}")
            return (False, None, str(log_file), str(e), None)
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
