#!/usr/bin/env python3
"""
Ollama Version Manager
Handles installation and switching for Ollama releases.
"""

from __future__ import annotations

import os
import platform
import shutil
import tarfile
import threading
import time
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import IO, Any, Dict, List, Optional

from backend.github_integration import DownloadManager, GitHubReleasesFetcher
from backend.installation_progress_tracker import InstallationProgressTracker, InstallationStage
from backend.logging_config import get_logger
from backend.metadata_manager import MetadataManager
from backend.models import GitHubRelease, VersionInfo
from backend.utils import ensure_directory, safe_filename
from backend.validators import validate_version_tag

logger = get_logger(__name__)


class OllamaVersionManager:
    """Manages Ollama version installation, switching, and metadata."""

    def __init__(
        self,
        launcher_root: Path,
        metadata_manager: MetadataManager,
        github_fetcher: GitHubReleasesFetcher,
    ) -> None:
        self.app_id = "ollama"
        self.launcher_root = Path(launcher_root)
        self.metadata_manager = metadata_manager
        self.github_fetcher = github_fetcher
        self.logs_dir = self.metadata_manager.launcher_data_dir / "logs"
        ensure_directory(self.logs_dir)

        self.versions_dir = self.launcher_root / "ollama-versions"
        self.active_version_file = self.launcher_root / ".active-version-ollama"
        ensure_directory(self.versions_dir)

        cache_dir = self.metadata_manager.launcher_data_dir / "cache"
        self.progress_tracker = InstallationProgressTracker(
            cache_dir, state_filename="installation-state-ollama.json"
        )

        self._active_version: Optional[str] = None
        self._installing_tag: Optional[str] = None
        self._cancel_installation = False
        self._current_downloader: Optional[DownloadManager] = None
        self._install_log_handle: Optional[IO[str]] = None
        self._current_install_log_path: Optional[Path] = None
        self._install_thread: Optional[threading.Thread] = None

        self._initialize_active_version()

    def _open_install_log(self, prefix: str) -> Path:
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
        except OSError as exc:
            logger.warning("Unable to open install log at %s: %s", log_path, exc)
            self._install_log_handle = None
            self._current_install_log_path = log_path
        return log_path

    def _log_install(self, message: str) -> None:
        if not message:
            return
        if self._install_log_handle:
            try:
                self._install_log_handle.write(message.rstrip() + "\n")
                self._install_log_handle.flush()
            except OSError as exc:
                logger.warning("Failed to write to install log: %s", exc)

    def _load_versions_metadata(self) -> Dict[str, Any]:
        return self.metadata_manager.load_versions_for_app(self.app_id)

    def _save_versions_metadata(self, data: Dict[str, Any]) -> bool:
        return self.metadata_manager.save_versions_for_app(self.app_id, data)

    def _write_active_version_file(self, tag: Optional[str]) -> bool:
        try:
            if tag:
                self.active_version_file.write_text(tag)
            elif self.active_version_file.exists():
                self.active_version_file.unlink()
            return True
        except OSError as exc:
            logger.error("Error writing active version file: %s", exc, exc_info=True)
            return False

    def _set_active_version_state(self, tag: Optional[str], update_last_selected: bool) -> bool:
        self._active_version = tag
        success = self._write_active_version_file(tag)
        if update_last_selected:
            versions_metadata = self._load_versions_metadata()
            versions_metadata["lastSelectedVersion"] = tag
            success = self._save_versions_metadata(versions_metadata) and success
        return success

    def _initialize_active_version(self) -> Optional[str]:
        installed_versions = self.get_installed_versions()
        if not installed_versions:
            self._set_active_version_state(None, update_last_selected=False)
            return None

        versions_metadata = self._load_versions_metadata()
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

    def _binary_name(self) -> str:
        return "ollama.exe" if platform.system().lower() == "windows" else "ollama"

    def _is_version_complete(self, version_path: Path) -> bool:
        if not version_path.exists():
            return False
        binary_path = version_path / self._binary_name()
        return binary_path.exists()

    def _select_asset(self, release: GitHubRelease) -> Optional[Dict[str, Any]]:
        assets = release.get("assets") or []
        if not assets:
            return None

        system = platform.system().lower()
        arch = platform.machine().lower()
        arch_map = {
            "x86_64": "amd64",
            "amd64": "amd64",
            "aarch64": "arm64",
            "arm64": "arm64",
        }
        desired_arch = arch_map.get(arch, arch)
        desired_os = "windows" if system.startswith("win") else system

        scored: List[tuple[int, Dict[str, Any]]] = []
        for asset in assets:
            name = str(asset.get("name", "")).lower()
            score = 0
            if desired_os and desired_os in name:
                score += 2
            if desired_arch and desired_arch in name:
                score += 2
            if desired_os == "windows" and name.endswith(".exe"):
                score += 1
            if name.endswith((".zip", ".tar.gz", ".tgz")):
                score += 1
            if score > 0:
                scored.append((score, asset))

        if scored:
            scored.sort(key=lambda item: item[0], reverse=True)
            return scored[0][1]

        return assets[0] if assets else None

    def _extract_archive(self, archive_path: Path, extract_dir: Path) -> Path:
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path, "r") as zip_ref:
                zip_ref.extractall(extract_dir)
        else:
            with tarfile.open(archive_path, "r:*") as tar_ref:
                tar_ref.extractall(extract_dir)

        extracted_contents = list(extract_dir.iterdir())
        if len(extracted_contents) == 1 and extracted_contents[0].is_dir():
            return extracted_contents[0]
        return extract_dir

    def _find_binary(self, root: Path) -> Optional[Path]:
        target = self._binary_name()
        for path in root.rglob(target):
            if path.is_file():
                return path
        return None

    def get_available_releases(
        self,
        force_refresh: bool = False,
        collapse: bool = True,
        include_prerelease: bool = True,
    ) -> List[GitHubRelease]:
        releases = self.github_fetcher.get_releases(force_refresh)
        if collapse:
            releases = self.github_fetcher.collapse_latest_patch_per_minor(
                releases, include_prerelease=include_prerelease
            )
        return releases

    def get_available_versions(self, force_refresh: bool = False) -> List[Dict[str, Any]]:
        releases = self.get_available_releases(force_refresh)
        installing_tag = None
        active_progress = self.get_installation_progress()
        if active_progress and not active_progress.get("completed_at"):
            installing_tag = active_progress.get("tag")

        enriched: List[Dict[str, Any]] = []
        for release in releases:
            tag = release.get("tag_name", "")
            release_with_size = dict(release)
            if not release_with_size.get("html_url") and tag:
                release_with_size["html_url"] = (
                    f"https://github.com/ollama/ollama/releases/tag/{tag}"
                )

            asset = self._select_asset(release)
            asset_size = asset.get("size") if asset else None
            release_with_size["archive_size"] = asset_size
            release_with_size["dependencies_size"] = 0
            release_with_size["total_size"] = asset_size
            release_with_size["installing"] = bool(installing_tag and tag == installing_tag)
            enriched.append(release_with_size)

        return enriched

    def get_release_size_info(self, tag: str) -> Optional[Dict[str, Any]]:
        release = self.github_fetcher.get_release_by_tag(tag)
        if not release:
            return None
        asset = self._select_asset(release)
        if not asset:
            return None
        size = asset.get("size")
        if size is None:
            return None
        return {
            "tag": tag,
            "total_size": size,
            "archive_size": size,
            "dependencies_size": 0,
        }

    def get_installed_versions(self) -> List[str]:
        versions_metadata = self._load_versions_metadata()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())
        validated_versions: List[str] = []

        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
        return validated_versions

    def validate_installations(self) -> Dict[str, Any]:
        versions_metadata = self._load_versions_metadata()
        metadata_versions = set(versions_metadata.get("installed", {}).keys())
        validated_versions: List[str] = []
        removed_versions: List[str] = []

        for tag in metadata_versions:
            version_path = self.versions_dir / tag
            if self._is_version_complete(version_path):
                validated_versions.append(tag)
            else:
                removed_versions.append(tag)

        if self.versions_dir.exists():
            for version_dir in self.versions_dir.iterdir():
                if not version_dir.is_dir():
                    continue
                tag = version_dir.name
                if tag not in metadata_versions:
                    removed_versions.append(tag)
                    try:
                        shutil.rmtree(version_dir)
                    except OSError as exc:
                        logger.debug(
                            "Failed to remove orphaned version dir %s: %s", version_dir, exc
                        )

        if removed_versions:
            for tag in removed_versions:
                if tag in versions_metadata.get("installed", {}):
                    del versions_metadata["installed"][tag]
            self._save_versions_metadata(versions_metadata)

        return {
            "had_invalid": len(removed_versions) > 0,
            "removed": removed_versions,
            "valid": validated_versions,
        }

    def get_installation_progress(self) -> Optional[Dict[str, Any]]:
        return self.progress_tracker.get_current_state()

    def cancel_installation(self) -> bool:
        if self._installing_tag:
            self._cancel_installation = True
            if self._current_downloader:
                try:
                    self._current_downloader.cancel()
                except AttributeError as exc:
                    logger.error("Error cancelling download: %s", exc, exc_info=True)
                except RuntimeError as exc:
                    logger.error("Error cancelling download: %s", exc, exc_info=True)
            self.progress_tracker.set_error("Installation cancelled by user")
            return True
        return False

    def _install_version_blocking(self, tag: str, asset: Dict[str, Any], download_url: str) -> bool:
        version_path = self.versions_dir / tag
        install_log_path = self._open_install_log(f"install-{safe_filename(tag)}")
        self._log_install(f"Starting install for {tag}")

        def _handle_install_failure(message: str) -> bool:
            self._log_install(message)
            self.progress_tracker.set_error(message)
            self.progress_tracker.complete_installation(False)
            if version_path.exists():
                shutil.rmtree(version_path, ignore_errors=True)
            return False

        try:
            total_size = asset.get("size") if isinstance(asset, dict) else None
            self.progress_tracker.start_installation(
                tag, total_size=total_size, log_path=str(install_log_path)
            )
            self.progress_tracker.update_stage(InstallationStage.DOWNLOAD, 0, f"Downloading {tag}")

            download_dir = self.launcher_root / "temp"
            ensure_directory(download_dir)

            asset_name = asset.get("name") or f"{tag}.bin"
            archive_path = download_dir / asset_name

            downloader = DownloadManager()
            self._current_downloader = downloader
            download_start_time = time.time()

            def on_download_progress(downloaded: int, total: int, speed: Optional[float] = None):
                effective_speed = speed
                if effective_speed is None and downloaded > 0:
                    elapsed = time.time() - download_start_time
                    if elapsed > 0:
                        effective_speed = downloaded / elapsed
                total_bytes = total if total and total > 0 else None
                self.progress_tracker.update_download_progress(
                    downloaded, total_bytes, effective_speed
                )

            success = downloader.download_with_retry(
                download_url, archive_path, progress_callback=on_download_progress
            )
            if not success:
                if downloader.was_cancelled():
                    return _handle_install_failure("Installation cancelled by user")
                return _handle_install_failure("Download failed")

            archive_size = archive_path.stat().st_size
            self.progress_tracker.update_download_progress(archive_size, archive_size)
            self.progress_tracker.add_completed_item(archive_path.name, "archive", archive_size)

            if self._cancel_installation:
                return _handle_install_failure("Installation cancelled by user")

            self.progress_tracker.update_stage(InstallationStage.EXTRACT, 0, "Preparing files")
            ensure_directory(version_path)

            asset_lower = archive_path.name.lower()
            if asset_lower.endswith((".zip", ".tar.gz", ".tgz")):
                temp_extract_dir = download_dir / f"extract-{tag}"
                ensure_directory(temp_extract_dir)
                extracted_dir = self._extract_archive(archive_path, temp_extract_dir)
                binary_path = self._find_binary(extracted_dir)
                if not binary_path:
                    return _handle_install_failure("Binary not found in archive")
                shutil.move(str(binary_path), str(version_path / self._binary_name()))
                shutil.rmtree(temp_extract_dir, ignore_errors=True)
            else:
                shutil.move(str(archive_path), str(version_path / self._binary_name()))

            if archive_path.exists():
                archive_path.unlink()

            binary_dest = version_path / self._binary_name()
            try:
                binary_dest.chmod(0o755)
            except OSError as exc:
                logger.warning("Failed to mark binary executable: %s", exc)

            self.progress_tracker.update_stage(InstallationStage.SETUP, 100, "Setup complete")

            version_info: VersionInfo = {
                "path": str(version_path.relative_to(self.launcher_root)),
                "installedDate": datetime.now(timezone.utc).isoformat(),
                "releaseTag": tag,
                "downloadUrl": download_url,
                "size": archive_size,
            }
            versions_metadata = self._load_versions_metadata()
            if "installed" not in versions_metadata:
                versions_metadata["installed"] = {}
            versions_metadata["installed"][tag] = version_info
            self._save_versions_metadata(versions_metadata)

            self.progress_tracker.complete_installation(True)
            self._log_install(f"✓ Successfully installed {tag}")
            return True
        except OSError as exc:
            logger.error("Error installing version %s: %s", tag, exc, exc_info=True)
            return _handle_install_failure(str(exc))
        finally:
            self._installing_tag = None
            self._cancel_installation = False
            self._current_downloader = None
            if self._install_log_handle:
                try:
                    self._install_log_handle.write(f"{'='*30} INSTALL END {tag} {'='*30}\n")
                    self._install_log_handle.close()
                except OSError as exc:
                    logger.debug("Failed to close install log: %s", exc)
                self._install_log_handle = None
            self._install_thread = None

    def install_version(self, tag: str) -> bool:
        if not validate_version_tag(tag):
            logger.error("Invalid version tag: %r", tag)
            return False
        if self._installing_tag:
            logger.warning("Installation already in progress for %s", self._installing_tag)
            return False
        if tag in self.get_installed_versions():
            logger.info("Version %s is already installed", tag)
            return False

        release = self.github_fetcher.get_release_by_tag(tag)
        if not release:
            logger.error("Release %s not found", tag)
            return False

        asset = self._select_asset(release)
        if not asset:
            logger.error("No suitable release asset found for %s", tag)
            return False

        download_url = asset.get("browser_download_url")
        if not download_url:
            logger.error("No download URL found for %s", tag)
            return False

        version_path = self.versions_dir / tag
        if version_path.exists():
            logger.warning("Version directory already exists: %s", version_path)
            return False

        self._cancel_installation = False
        self._installing_tag = tag

        def _worker() -> None:
            self._install_version_blocking(tag, asset, download_url)

        self._install_thread = threading.Thread(target=_worker, daemon=True)
        self._install_thread.start()
        return True

    def remove_version(self, tag: str) -> bool:
        if not validate_version_tag(tag):
            logger.error("Invalid version tag for removal: %r", tag)
            return False
        if tag not in self.get_installed_versions():
            logger.warning("Version %s is not installed", tag)
            return False
        if self.get_active_version() == tag:
            logger.warning("Cannot remove active version %s", tag)
            return False

        version_path = self.versions_dir / tag
        try:
            shutil.rmtree(version_path)
            versions_metadata = self._load_versions_metadata()
            if tag in versions_metadata.get("installed", {}):
                del versions_metadata["installed"][tag]
            if versions_metadata.get("defaultVersion") == tag:
                versions_metadata["defaultVersion"] = None
            self._save_versions_metadata(versions_metadata)
            logger.info("✓ Removed version %s", tag)
            return True
        except OSError as exc:
            logger.error("Error removing version %s: %s", tag, exc, exc_info=True)
            return False

    def get_version_info(self, tag: str) -> Optional[VersionInfo]:
        if not validate_version_tag(tag):
            logger.warning("Invalid version tag for info lookup: %r", tag)
            return None
        versions_metadata = self._load_versions_metadata()
        return versions_metadata.get("installed", {}).get(tag)

    def get_version_path(self, tag: str) -> Optional[Path]:
        if not validate_version_tag(tag):
            logger.warning("Invalid version tag for path lookup: %r", tag)
            return None
        version_path = self.versions_dir / tag
        if not self._is_version_complete(version_path):
            return None
        return version_path

    def get_active_version(self) -> Optional[str]:
        installed_versions = self.get_installed_versions()
        if not installed_versions:
            self._active_version = None
            return None
        if self._active_version in installed_versions:
            return self._active_version
        return self._initialize_active_version()

    def get_active_version_path(self) -> Optional[Path]:
        active_tag = self.get_active_version()
        if not active_tag:
            return None
        return self.get_version_path(active_tag)

    def set_active_version(self, tag: str) -> bool:
        if not validate_version_tag(tag):
            logger.warning("Invalid version tag for activation: %r", tag)
            return False
        if tag not in self.get_installed_versions():
            logger.warning("Cannot activate uninstalled version %s", tag)
            return False
        return self._set_active_version_state(tag, update_last_selected=True)

    def get_default_version(self) -> Optional[str]:
        versions_metadata = self._load_versions_metadata()
        return versions_metadata.get("defaultVersion")

    def set_default_version(self, tag: Optional[str]) -> bool:
        if tag is not None and not validate_version_tag(tag):
            logger.warning("Invalid version tag for default selection: %r", tag)
            return False
        versions_metadata = self._load_versions_metadata()
        versions_metadata["defaultVersion"] = tag
        return self._save_versions_metadata(versions_metadata)

    def get_version_status(self) -> Dict[str, Any]:
        installed = self.get_installed_versions()
        active = self.get_active_version()
        versions_status: Dict[str, Any] = {}
        for tag in installed:
            versions_status[tag] = {
                "info": self.get_version_info(tag),
                "dependencies": {"installed": [], "missing": [], "requirementsFile": None},
                "isActive": tag == active,
            }
        return {
            "installedCount": len(installed),
            "activeVersion": active,
            "defaultVersion": self.get_default_version(),
            "versions": versions_status,
        }
