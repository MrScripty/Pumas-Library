#!/usr/bin/env python3
"""
ComfyUI Setup API - Core Module
Main API class that coordinates all setup operations
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional

from backend.api.dependency_manager import DependencyManager
from backend.api.patch_manager import PatchManager
from backend.api.process_manager import ProcessManager
from backend.api.shortcut_manager import ShortcutManager
from backend.api.size_calculator import SizeCalculator
from backend.api.system_utils import SystemUtils
from backend.api.version_info import VersionInfoManager
from backend.logging_config import get_logger
from backend.model_library.io import detect_sandbox_environment
from backend.models import DependencyStatus, GitHubRelease, ModelOverrides, ScanResult, VersionInfo
from backend.rate_limiter import RateLimiter
from backend.validators import validate_package_name, validate_url, validate_version_tag

if TYPE_CHECKING:
    from backend.github_integration import GitHubReleasesFetcher
    from backend.launcher_updater import LauncherUpdater
    from backend.metadata_manager import MetadataManager
    from backend.release_size_calculator import ReleaseSizeCalculator
    from backend.resources.resource_manager import ResourceManager
    from backend.version_manager import VersionManager

logger = get_logger(__name__)

InstallProgressCallback = Optional[Callable[[str, int, int], None]]
DependencyProgressCallback = Optional[Callable[[str], None]]
ReleaseSizeProgressCallback = Optional[Callable[[int, int, str], None]]


class ComfyUISetupAPI:
    """Main API class for ComfyUI setup operations"""

    def __init__(self, enable_background_prefetch: bool = True) -> None:
        # Determine directories based on launcher location
        # Handle both development mode and PyInstaller bundled mode
        self._enable_background_prefetch = enable_background_prefetch
        if getattr(sys, "frozen", False):
            # Running as PyInstaller bundle
            # Search upward from executable location to find ComfyUI root
            self.comfyui_dir = self._find_comfyui_root(Path(sys.executable).parent)
            # Launcher directory is where run.sh and icon should be
            # Try common locations
            launcher_candidates = [
                *self._find_launcher_candidates(self.comfyui_dir),
                Path(sys.executable).parent.parent,  # dist/ parent
                Path(sys.executable).parent,  # same dir as executable
            ]
            script_dir: Optional[Path] = None
            for candidate in launcher_candidates:
                if self._looks_like_launcher_root(candidate):
                    script_dir = candidate
                    break
            if script_dir is None:
                for candidate in launcher_candidates:
                    if candidate.exists():
                        script_dir = candidate
                        break
            if script_dir is None:
                # Fallback to executable directory
                script_dir = Path(sys.executable).parent
            assert script_dir is not None
            self.script_dir = script_dir
        else:
            # Running in development mode
            self.script_dir = Path(__file__).parent.parent.parent.resolve()
            self.comfyui_dir = self.script_dir.parent

        self.main_py: Path = self.comfyui_dir / "main.py"
        self.launcher_data_dir: Path = self.script_dir / "launcher-data"
        self.icon_webp: Path = self.launcher_data_dir / "icons" / "comfyui_logo_2025.png"
        self.shortcut_scripts_dir = self.launcher_data_dir / "shortcuts"
        self.generated_icons_dir = self.launcher_data_dir / "icons"

        # Ensure directories used by shortcut tooling exist
        self.shortcut_scripts_dir.mkdir(parents=True, exist_ok=True)
        self.generated_icons_dir.mkdir(parents=True, exist_ok=True)

        # Initialize version management components (Phase 2-4)
        self.metadata_manager: Optional[MetadataManager] = None
        self.github_fetcher: Optional[GitHubReleasesFetcher] = None
        self.resource_manager: Optional[ResourceManager] = None
        self.version_manager: Optional[VersionManager] = None
        self.release_size_calculator: Optional[ReleaseSizeCalculator] = None
        self.size_calc: Optional[SizeCalculator] = None
        self.launcher_updater: Optional["LauncherUpdater"] = None
        self._init_version_management()

        # Initialize specialized managers
        self._init_managers()

        # Rate limits for destructive actions
        self._rate_limiters = {
            "install": RateLimiter(max_calls=3, period_seconds=60),
            "remove": RateLimiter(max_calls=5, period_seconds=60),
            "cancel": RateLimiter(max_calls=10, period_seconds=60),
        }

    def _is_rate_limited(self, action: str, tag: Optional[str] = None) -> bool:
        limiter = self._rate_limiters.get(action)
        if not limiter:
            return False
        if limiter.is_allowed(action):
            return False
        target = f" for {tag}" if tag else ""
        logger.warning(f"Rate limit exceeded for {action}{target}")
        return True

    def _find_comfyui_root(self, start_path: Path) -> Path:
        """
        Search upward from start_path to find ComfyUI root directory.
        ComfyUI root is identified by the presence of main.py and pyproject.toml.
        """
        current = start_path.resolve()

        # Search up to 5 levels
        for _ in range(5):
            main_py = current / "main.py"
            pyproject = current / "pyproject.toml"

            # Check if both files exist
            if main_py.exists() and pyproject.exists():
                # Verify it's ComfyUI by checking pyproject.toml
                try:
                    with open(pyproject, "rb") as f:
                        data = tomllib.load(f)
                        if data.get("project", {}).get("name") == "ComfyUI":
                            return current
                except OSError as exc:
                    logger.debug("Failed to read %s: %s", pyproject, exc)
                except tomllib.TOMLDecodeError as exc:
                    logger.debug("Failed to parse %s: %s", pyproject, exc)

            # Move up one directory
            parent = current.parent
            if parent == current:
                # Reached filesystem root
                break
            current = parent

        # Fallback: return the parent of start_path
        return start_path.parent

    def _looks_like_launcher_root(self, candidate: Path) -> bool:
        return candidate.is_dir() and (candidate / "launcher-data").exists()

    def _find_launcher_candidates(self, comfyui_root: Path) -> List[Path]:
        parent = comfyui_root.parent
        if not parent.exists():
            return []
        candidates: List[Path] = []
        for entry in parent.iterdir():
            if self._looks_like_launcher_root(entry):
                candidates.append(entry)
        return candidates

    def _init_version_management(self) -> None:
        """Initialize version management components"""
        try:
            from backend.github_integration import GitHubReleasesFetcher
            from backend.metadata_manager import MetadataManager
            from backend.package_size_resolver import PackageSizeResolver
            from backend.release_data_fetcher import ReleaseDataFetcher
            from backend.release_size_calculator import ReleaseSizeCalculator
            from backend.resources.resource_manager import ResourceManager
            from backend.version_manager import VersionManager

            launcher_data_dir = self.script_dir / "launcher-data"
            cache_dir = launcher_data_dir / "cache"

            self.metadata_manager = MetadataManager(launcher_data_dir)
            self.github_fetcher = GitHubReleasesFetcher(self.metadata_manager)
            self.resource_manager = ResourceManager(self.script_dir, self.metadata_manager)
            self.version_manager = VersionManager(
                self.script_dir,
                self.metadata_manager,
                self.github_fetcher,
                self.resource_manager,
            )

            # Initialize size calculation components (Phase 6.2.5a)
            self.release_data_fetcher = ReleaseDataFetcher(cache_dir)
            self.package_size_resolver = PackageSizeResolver(cache_dir)
            self.release_size_calculator = ReleaseSizeCalculator(
                cache_dir,
                self.release_data_fetcher,
                self.package_size_resolver,
                cache_dir / "pip",
            )

            if self._enable_background_prefetch:
                self._prefetch_releases_if_needed()
        except ImportError as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
        except OSError as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
        except RuntimeError as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
        except TypeError as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
        except ValueError as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
            self.metadata_manager = None
            self.github_fetcher = None
            self.resource_manager = None
            self.version_manager = None
            self.release_size_calculator = None

    def _init_managers(self) -> None:
        """Initialize all specialized manager components"""
        # Dependency manager
        self.dependency_mgr = DependencyManager(self.script_dir)

        # Version info manager
        self.version_info_mgr = VersionInfoManager(self.comfyui_dir, self.github_fetcher)

        # Patch manager
        self.patch_mgr = PatchManager(self.comfyui_dir, self.main_py, self.version_manager)

        # Shortcut manager
        self.shortcut_mgr = ShortcutManager(
            self.script_dir,
            self.icon_webp,
            self.shortcut_scripts_dir,
            self.generated_icons_dir,
            self.version_manager,
            self.metadata_manager,
        )

        # Process manager
        self.process_mgr = ProcessManager(self.comfyui_dir, self.version_manager)

        # Size calculator
        self.size_calc = None
        if self.release_size_calculator and self.github_fetcher:
            self.size_calc = SizeCalculator(
                self.release_size_calculator, self.github_fetcher, self.version_manager
            )

        # System utilities
        self.system_utils = SystemUtils(
            self.script_dir,
            self.dependency_mgr,
            self.patch_mgr,
            self.shortcut_mgr,
            self.process_mgr,
            self.version_info_mgr,
            self.version_manager,
        )

    def _prefetch_releases_if_needed(self) -> None:
        """
        Smart background prefetch - never blocks startup

        Logic:
        - Valid cache → Skip prefetch (app starts instantly)
        - Stale/no cache → Prefetch in background (app still starts instantly)
        """
        try:
            if not self.github_fetcher or not self.metadata_manager:
                return

            # Quick check: do we have a valid cache?
            cache = self.metadata_manager.load_github_cache()
            cache_age = None

            if cache and cache.get("releases"):
                try:
                    from backend.models import get_iso_timestamp, parse_iso_timestamp

                    last_fetched = parse_iso_timestamp(cache["lastFetched"])
                    now = parse_iso_timestamp(get_iso_timestamp())
                    cache_age = (now - last_fetched).total_seconds()
                    ttl = cache.get("ttl", 3600)

                    if cache_age < ttl:
                        logger.debug(
                            f"GitHub cache is valid ({int(cache_age)}s old) - skipping prefetch"
                        )
                        return
                    else:
                        logger.info(f"GitHub cache is stale ({int(cache_age)}s old) - prefetching")
                except KeyError as e:
                    logger.warning(
                        f"Error checking cache validity: {e} - prefetching",
                        exc_info=True,
                    )
                except TypeError as e:
                    logger.warning(
                        f"Error checking cache validity: {e} - prefetching",
                        exc_info=True,
                    )
                except ValueError as e:
                    logger.warning(
                        f"Error checking cache validity: {e} - prefetching",
                        exc_info=True,
                    )
            else:
                logger.info("No GitHub cache found - prefetching in background")

            # Track completion for frontend polling
            self._background_fetch_completed = False

            def _background_fetch():
                try:
                    # Use force_refresh=True to actually fetch
                    # (blocking is OK in background thread)
                    releases = self.github_fetcher.get_releases(force_refresh=True)
                    if releases:
                        logger.info(f"Background prefetch complete: {len(releases)} releases")
                        # Mark completion so frontend can detect
                        self._background_fetch_completed = True
                    else:
                        logger.warning("Background prefetch returned empty (likely offline)")
                except OSError as exc:
                    logger.error(f"Background prefetch failed: {exc}", exc_info=True)
                    logger.info("App will continue using stale cache")
                except RuntimeError as exc:
                    logger.error(f"Background prefetch failed: {exc}", exc_info=True)
                    logger.info("App will continue using stale cache")
                except TypeError as exc:
                    logger.error(f"Background prefetch failed: {exc}", exc_info=True)
                    logger.info("App will continue using stale cache")
                except ValueError as exc:
                    logger.error(f"Background prefetch failed: {exc}", exc_info=True)
                    logger.info("App will continue using stale cache")

            import threading

            threading.Thread(target=_background_fetch, daemon=True).start()

        except OSError as e:
            logger.error(f"Prefetch init error: {e}", exc_info=True)
        except RuntimeError as e:
            logger.error(f"Prefetch init error: {e}", exc_info=True)
        except TypeError as e:
            logger.error(f"Prefetch init error: {e}", exc_info=True)
        except ValueError as e:
            logger.error(f"Prefetch init error: {e}", exc_info=True)

    def has_background_fetch_completed(self) -> bool:
        """Check if background fetch has completed (for frontend polling)"""
        return getattr(self, "_background_fetch_completed", False)

    def reset_background_fetch_flag(self) -> None:
        """Reset the completion flag (called by frontend after refresh)"""
        self._background_fetch_completed = False

    def get_github_cache_status(self) -> Dict[str, Any]:
        """Get GitHub releases cache status for UI display"""
        if not self.github_fetcher:
            return {"has_cache": False, "is_valid": False, "is_fetching": False}
        return self.github_fetcher.get_cache_status()

    # ==================== Dependency Checking ====================

    def check_setproctitle(self) -> bool:
        """Check if setproctitle module is installed"""
        return self.dependency_mgr.check_setproctitle()

    def check_git(self) -> bool:
        """Check if git is installed"""
        return self.dependency_mgr.check_git()

    def check_brave(self) -> bool:
        """Check if Brave browser is installed"""
        return self.dependency_mgr.check_brave()

    def get_missing_dependencies(self) -> List[str]:
        """Get list of missing dependencies"""
        return self.dependency_mgr.get_missing_dependencies()

    def install_missing_dependencies(self) -> bool:
        """Install missing dependencies (requires user interaction for sudo)"""
        return self.dependency_mgr.install_missing_dependencies()

    # ==================== Version Detection ====================

    def get_comfyui_version(self) -> str:
        """Get ComfyUI version from pyproject.toml, git, or GitHub API"""
        return self.version_info_mgr.get_comfyui_version()

    def check_for_new_release(self, force_refresh: bool = False) -> Dict[str, Any]:
        """Check if a new release is available on GitHub (cached)"""
        return self.version_info_mgr.check_for_new_release(force_refresh)

    # ==================== Patch Management ====================

    def is_patched(self, tag: Optional[str] = None) -> bool:
        """Check if selected main.py is patched with setproctitle"""
        return self.patch_mgr.is_patched(tag)

    def patch_main_py(self, tag: Optional[str] = None) -> bool:
        """Patch selected main.py to set process title"""
        return self.patch_mgr.patch_main_py(tag)

    def revert_main_py(self, tag: Optional[str] = None) -> bool:
        """Revert selected main.py to original state"""
        return self.patch_mgr.revert_main_py(tag)

    # ==================== Shortcut Management ====================

    def get_version_shortcut_state(self, tag: str) -> Dict[str, Any]:
        """Return the current shortcut state for a version"""
        return self.shortcut_mgr.get_version_shortcut_state(tag)

    def get_all_shortcut_states(self) -> Dict[str, Any]:
        """Get shortcut states for all installed versions"""
        return self.shortcut_mgr.get_all_shortcut_states()

    def create_version_shortcuts(
        self, tag: str, create_menu: bool = True, create_desktop: bool = True
    ) -> Dict[str, Any]:
        """Create menu/desktop shortcuts for a specific version"""
        return self.shortcut_mgr.create_version_shortcuts(tag, create_menu, create_desktop)

    def remove_version_shortcuts(
        self, tag: str, remove_menu: bool = True, remove_desktop: bool = True
    ) -> Dict[str, Any]:
        """Remove version-specific shortcuts and icons"""
        return self.shortcut_mgr.remove_version_shortcuts(tag, remove_menu, remove_desktop)

    def set_version_shortcuts(
        self, tag: str, enabled: bool, menu: bool = True, desktop: bool = True
    ) -> Dict[str, Any]:
        """Ensure shortcuts for a version are enabled/disabled"""
        return self.shortcut_mgr.set_version_shortcuts(tag, enabled, menu, desktop)

    def toggle_version_menu_shortcut(self, tag: str) -> Dict[str, Any]:
        """Toggle only the menu shortcut for a version"""
        return self.shortcut_mgr.toggle_version_menu_shortcut(tag)

    def toggle_version_desktop_shortcut(self, tag: str) -> Dict[str, Any]:
        """Toggle only the desktop shortcut for a version"""
        return self.shortcut_mgr.toggle_version_desktop_shortcut(tag)

    def menu_exists(self) -> bool:
        """Check if menu shortcut exists"""
        return self.shortcut_mgr.menu_exists()

    def desktop_exists(self) -> bool:
        """Check if desktop shortcut exists"""
        return self.shortcut_mgr.desktop_exists()

    def install_icon(self) -> bool:
        """Install icon to system icon directory"""
        return self.shortcut_mgr.install_icon()

    def create_menu_shortcut(self) -> bool:
        """Create application menu shortcut"""
        return self.shortcut_mgr.create_menu_shortcut()

    def create_desktop_shortcut(self) -> bool:
        """Create desktop shortcut"""
        return self.shortcut_mgr.create_desktop_shortcut()

    def remove_menu_shortcut(self) -> bool:
        """Remove application menu shortcut"""
        return self.shortcut_mgr.remove_menu_shortcut()

    def remove_desktop_shortcut(self) -> bool:
        """Remove desktop shortcut"""
        return self.shortcut_mgr.remove_desktop_shortcut()

    # ==================== Process Management ====================

    def is_comfyui_running(self) -> bool:
        """Check if ComfyUI is currently running"""
        return self.process_mgr.is_comfyui_running()

    def stop_comfyui(self) -> bool:
        """Stop running ComfyUI instance"""
        return self.process_mgr.stop_comfyui()

    def launch_comfyui(self) -> Dict[str, Any]:
        """Launch the active ComfyUI version with readiness detection."""
        return self.process_mgr.launch_comfyui()

    # ==================== Status API ====================

    def get_status(self) -> Dict[str, Any]:
        """Get complete system status"""
        return self.system_utils.get_status()

    def get_disk_space(self) -> Dict[str, Any]:
        """Get disk space information for the launcher directory"""
        return self.system_utils.get_disk_space()

    def get_system_resources(self) -> Dict[str, Any]:
        """Get current system resource usage (CPU, GPU, RAM, Disk)"""
        return self.system_utils.get_system_resources()

    # ==================== Action Handlers ====================

    def toggle_patch(self) -> bool:
        """Toggle main.py patch"""
        return self.system_utils.toggle_patch()

    def toggle_menu(self, tag: Optional[str] = None) -> bool:
        """Toggle menu shortcut (version-specific when available)"""
        return self.system_utils.toggle_menu(tag)

    def toggle_desktop(self, tag: Optional[str] = None) -> bool:
        """Toggle desktop shortcut (version-specific when available)"""
        return self.system_utils.toggle_desktop(tag)

    def open_path(self, path: str) -> Dict[str, Any]:
        """Open a filesystem path in the user's file manager (cross-platform)."""
        return self.system_utils.open_path(path)

    def open_url(self, url: str) -> Dict[str, Any]:
        """Open a URL in the default system browser."""
        return self.system_utils.open_url(url)

    def open_active_install(self) -> Dict[str, Any]:
        """Open the active ComfyUI installation directory in the file manager."""
        return self.system_utils.open_active_install()

    # ==================== Version Management API (Phase 5) ====================

    def get_available_versions(self, force_refresh: bool = False) -> List[Dict[str, Any]]:
        """
        Get list of available ComfyUI versions from GitHub with size information

        Args:
            force_refresh: Force refresh from GitHub API (bypass cache)

        Returns:
            List of release dictionaries with size data
        """
        if not self.version_manager:
            return []

        releases_source = "cache"
        releases: List[GitHubRelease] = []

        # Try to fetch (optionally forced); on failure, fall back to cached data without clearing it
        try:
            releases = self.version_manager.get_available_releases(force_refresh)
            releases_source = "remote" if force_refresh else "cache/remote"
        except OSError as e:
            logger.error(
                f"Error fetching releases (force_refresh={force_refresh}): {e}",
                exc_info=True,
            )
            releases = []
        except RuntimeError as e:
            logger.error(
                f"Error fetching releases (force_refresh={force_refresh}): {e}",
                exc_info=True,
            )
            releases = []
        except TypeError as e:
            logger.error(
                f"Error fetching releases (force_refresh={force_refresh}): {e}",
                exc_info=True,
            )
            releases = []
        except ValueError as e:
            logger.error(
                f"Error fetching releases (force_refresh={force_refresh}): {e}",
                exc_info=True,
            )
            releases = []

        if force_refresh and not releases:
            try:
                cache = self.metadata_manager.load_github_cache() if self.metadata_manager else None
                if cache and cache.get("releases"):
                    releases = cache.get("releases", [])
                    releases_source = "cache-fallback"
                    logger.info("Using cached releases due to fetch error/rate-limit.")
            except OSError as e:
                logger.error(
                    f"Error loading cached releases after fetch failure: {e}",
                    exc_info=True,
                )
            except TypeError as e:
                logger.error(
                    f"Error loading cached releases after fetch failure: {e}",
                    exc_info=True,
                )
            except ValueError as e:
                logger.error(
                    f"Error loading cached releases after fetch failure: {e}",
                    exc_info=True,
                )
        if releases:
            releases = [release for release in releases if isinstance(release, dict)]

        # Enrich releases with size information (Phase 6.2.5c) + installing flag
        installing_tag = None
        active_progress = None
        try:
            active_progress = self.version_manager.get_installation_progress()
            if active_progress and not active_progress.get("completed_at"):
                installing_tag = active_progress.get("tag")
        except OSError as e:
            logger.error(f"Error checking installation progress for releases: {e}", exc_info=True)
        except RuntimeError as e:
            logger.error(f"Error checking installation progress for releases: {e}", exc_info=True)
        except TypeError as e:
            logger.error(f"Error checking installation progress for releases: {e}", exc_info=True)
        except ValueError as e:
            logger.error(f"Error checking installation progress for releases: {e}", exc_info=True)

        enriched_releases: List[Dict[str, Any]] = []
        for release in releases:
            tag = release.get("tag_name", "")

            # Get cached size data if available
            size_data = (
                self.release_size_calculator.get_cached_size(tag)
                if self.release_size_calculator
                else None
            )

            # Add size information to release
            release_with_size = dict(release)
            if not release_with_size.get("html_url") and tag:
                release_with_size["html_url"] = (
                    f"https://github.com/comfyanonymous/ComfyUI/releases/tag/{tag}"
                )
            if size_data:
                release_with_size["total_size"] = size_data["total_size"]
                release_with_size["archive_size"] = size_data["archive_size"]
                release_with_size["dependencies_size"] = size_data["dependencies_size"]
            else:
                # Size not yet calculated
                release_with_size["total_size"] = None
                release_with_size["archive_size"] = None
                release_with_size["dependencies_size"] = None

            # Flag releases currently installing
            release_with_size["installing"] = bool(installing_tag and tag == installing_tag)

            enriched_releases.append(release_with_size)

        # Kick off background size refresh prioritizing non-installed releases
        try:
            installed_tags = set(self.get_installed_versions())
            if self.size_calc:
                self.size_calc._refresh_release_sizes_async(releases, installed_tags, force_refresh)
        except OSError as e:
            logger.error(f"Error scheduling size refresh: {e}", exc_info=True)
        except RuntimeError as e:
            logger.error(f"Error scheduling size refresh: {e}", exc_info=True)
        except TypeError as e:
            logger.error(f"Error scheduling size refresh: {e}", exc_info=True)
        except ValueError as e:
            logger.error(f"Error scheduling size refresh: {e}", exc_info=True)

        return enriched_releases

    def get_installed_versions(self) -> List[str]:
        """Get list of installed ComfyUI version tags"""
        if not self.version_manager:
            return []
        return self.version_manager.get_installed_versions()

    def validate_installations(self) -> Dict[str, Any]:
        """Validate all installations and clean up incomplete ones"""
        if not self.version_manager:
            return {"had_invalid": False, "removed": [], "valid": []}
        return self.version_manager.validate_installations()

    def get_installation_progress(self) -> Optional[Dict[str, Any]]:
        """Get current installation progress (Phase 6.2.5b)"""
        if not self.version_manager:
            return None
        return self.version_manager.get_installation_progress()

    def install_version(self, tag: str, progress_callback: InstallProgressCallback = None) -> bool:
        """Install a ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected install for invalid tag: {tag!r}")
            return False
        if self._is_rate_limited("install", tag):
            return False
        install_ok = self.version_manager.install_version(tag, progress_callback)
        if not install_ok:
            return False

        # Automatically patch the newly installed version so the UI button isn't needed
        patched = self.patch_mgr.patch_main_py(tag)
        if not patched and not self.patch_mgr.is_patched(tag):
            logger.warning(f"Installation succeeded but patching {tag} failed.")
            return False

        return True

    def cancel_installation(self) -> bool:
        """Cancel the currently running installation"""
        if not self.version_manager:
            return False
        if self._is_rate_limited("cancel"):
            return False
        return self.version_manager.cancel_installation()

    def remove_version(self, tag: str) -> bool:
        """Remove an installed ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected removal for invalid tag: {tag!r}")
            return False
        if self._is_rate_limited("remove", tag):
            return False
        removed = self.version_manager.remove_version(tag)
        if removed:
            # Clean up any version-specific shortcuts and icons
            self.shortcut_mgr.remove_version_shortcuts(tag, remove_menu=True, remove_desktop=True)
        return removed

    def switch_version(self, tag: str) -> bool:
        """Switch to a different ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected switch for invalid tag: {tag!r}")
            return False
        return self.version_manager.set_active_version(tag)

    def get_active_version(self) -> str:
        """Get currently active ComfyUI version"""
        if not self.version_manager:
            return ""
        return self.version_manager.get_active_version() or ""

    def get_default_version(self) -> str:
        """Get configured default ComfyUI version"""
        if not self.version_manager:
            return ""
        return self.version_manager.get_default_version() or ""

    def set_default_version(self, tag: Optional[str]) -> bool:
        """Set the default ComfyUI version (or clear when tag is None)"""
        if not self.version_manager:
            return False
        if tag is not None and not validate_version_tag(tag):
            logger.warning(f"Rejected default version for invalid tag: {tag!r}")
            return False
        return self.version_manager.set_default_version(tag)

    def check_version_dependencies(self, tag: str) -> DependencyStatus:
        """Check dependency installation status for a version"""
        if not self.version_manager:
            return {"installed": [], "missing": [], "requirementsFile": None}
        if not validate_version_tag(tag):
            logger.warning(f"Rejected dependency check for invalid tag: {tag!r}")
            return {"installed": [], "missing": [], "requirementsFile": None}
        return self.version_manager.check_dependencies(tag)

    def install_version_dependencies(
        self, tag: str, progress_callback: DependencyProgressCallback = None
    ) -> bool:
        """Install dependencies for a ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected dependency install for invalid tag: {tag!r}")
            return False
        return self.version_manager.install_dependencies(tag, progress_callback)

    def get_version_status(self) -> Dict[str, Any]:
        """Get comprehensive status of all versions"""
        if not self.version_manager:
            return {"installedCount": 0, "activeVersion": None, "versions": {}}
        return self.version_manager.get_version_status()

    def get_version_info(self, tag: str) -> Optional[VersionInfo]:
        """Get detailed information about a specific version"""
        if not self.version_manager:
            return None
        if not validate_version_tag(tag):
            logger.warning(f"Rejected version info request for invalid tag: {tag!r}")
            return None
        return self.version_manager.get_version_info(tag)

    def launch_version(self, tag: str, extra_args: Optional[List[str]] = None) -> Dict[str, Any]:
        """Launch a specific ComfyUI version"""
        if not self.version_manager:
            return {"success": False, "error": "Version manager unavailable"}
        if not validate_version_tag(tag):
            logger.warning(f"Rejected launch for invalid tag: {tag!r}")
            return {"success": False, "error": "Invalid version tag"}
        success, process, log_path, error_msg, ready = self.version_manager.launch_version(
            tag, extra_args
        )
        return {
            "success": success,
            "log_path": log_path,
            "ready": ready,
            "error": error_msg,
        }

    # ==================== Size Calculation API ====================

    def calculate_release_size(
        self, tag: str, force_refresh: bool = False
    ) -> Optional[Dict[str, Any]]:
        """Calculate total download size for a release (Phase 6.2.5c)"""
        if not self.size_calc:
            return None
        return self.size_calc.calculate_release_size(tag, force_refresh)

    def calculate_all_release_sizes(
        self, progress_callback: ReleaseSizeProgressCallback = None
    ) -> Dict[str, Dict[str, Any]]:
        """Calculate sizes for all available releases (Phase 6.2.5c)"""
        if not self.size_calc:
            return {}
        return self.size_calc.calculate_all_release_sizes(progress_callback)

    def get_release_size_info(self, tag: str, archive_size: int) -> Optional[Dict[str, Any]]:
        """Get size information for a release (Phase 6.2.5a/c)"""
        if not self.size_calc:
            return None
        return self.size_calc.get_release_size_info(tag, archive_size)

    def get_release_size_breakdown(self, tag: str) -> Optional[Dict[str, Any]]:
        """Get size breakdown for display (Phase 6.2.5c)"""
        if not self.size_calc:
            return None
        return self.size_calc.get_release_size_breakdown(tag)

    def get_release_dependencies(
        self, tag: str, top_n: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """Get dependencies for a release sorted by size (Phase 6.2.5c)"""
        if not self.size_calc:
            return []
        return self.size_calc.get_release_dependencies(tag, top_n)

    # ==================== Resource Management API (Phase 5) ====================

    def get_models(self) -> Dict[str, Any]:
        """Get list of models in shared storage"""
        if not self.resource_manager:
            return {}
        return self.resource_manager.get_models()

    def refresh_model_index(self) -> bool:
        """Rebuild the model library index."""
        if not self.resource_manager:
            return False
        self.resource_manager.refresh_model_index()
        return True

    def refresh_model_mappings(self, app_id: str = "comfyui") -> Dict[str, int]:
        """Refresh model mappings for all installed versions."""
        if not self.resource_manager:
            return {}
        return self.resource_manager.refresh_model_mappings(app_id)

    def import_model(
        self, local_path: str, family: str, official_name: str, repo_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """Import a local model into the library."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}
        try:
            model_dir = self.resource_manager.import_model(
                Path(local_path), family, official_name, repo_id
            )
            return {"success": True, "model_path": str(model_dir)}
        except OSError as exc:
            logger.error("Model import failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Model import failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except ValueError as exc:
            logger.error("Model import failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def download_model_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Download a model from Hugging Face into the library."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}
        try:
            model_dir = self.resource_manager.download_model_from_hf(
                repo_id=repo_id,
                family=family,
                official_name=official_name,
                model_type=model_type,
                subtype=subtype,
                quant=quant,
            )
            return {"success": True, "model_path": str(model_dir)}
        except OSError as exc:
            logger.error("Model download failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Model download failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except ValueError as exc:
            logger.error("Model download failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def start_model_download_from_hf(
        self,
        repo_id: str,
        family: str,
        official_name: str,
        model_type: Optional[str] = None,
        subtype: str = "",
        quant: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Start a Hugging Face model download with progress tracking."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}
        try:
            result = self.resource_manager.start_model_download_from_hf(
                repo_id=repo_id,
                family=family,
                official_name=official_name,
                model_type=model_type,
                subtype=subtype,
                quant=quant,
            )
            return {"success": True, **result}
        except OSError as exc:
            logger.error("Model download start failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Model download start failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except ValueError as exc:
            logger.error("Model download start failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def get_model_download_status(self, download_id: str) -> Dict[str, Any]:
        """Fetch status for a model download."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}
        status = self.resource_manager.get_model_download_status(download_id)
        if not status:
            return {"success": False, "error": "Download not found"}
        return {"success": True, **status}

    def cancel_model_download(self, download_id: str) -> Dict[str, Any]:
        """Cancel an active model download."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}
        cancelled = self.resource_manager.cancel_model_download(download_id)
        if not cancelled:
            return {"success": False, "error": "Download not active"}
        return {"success": True}

    def search_hf_models(
        self,
        query: str,
        kind: Optional[str] = None,
        limit: int = 25,
    ) -> Dict[str, Any]:
        """Search Hugging Face models for the download UI."""
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable", "models": []}
        try:
            results = self.resource_manager.search_hf_models(query=query, kind=kind, limit=limit)
            return {"success": True, "models": results}
        except OSError as exc:
            logger.error("Hugging Face search failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "models": []}
        except RuntimeError as exc:
            logger.error("Hugging Face search failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "models": []}
        except ValueError as exc:
            logger.error("Hugging Face search failed: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "models": []}

    def search_models_fts(
        self,
        query: str,
        limit: int = 100,
        offset: int = 0,
        model_type: Optional[str] = None,
        tags: Optional[List[str]] = None,
    ) -> Dict[str, Any]:
        """Search local model library using FTS5 full-text search.

        Performs fast full-text search across model metadata including
        names, types, tags, family, and description.

        Args:
            query: Search terms (space-separated for OR matching)
            limit: Maximum number of results to return
            offset: Number of results to skip (for pagination)
            model_type: Filter by model type (e.g., "diffusion", "llm")
            tags: Filter by required tags

        Returns:
            Dict with keys:
                - success: Whether the search succeeded
                - models: List of matching model metadata
                - total_count: Total number of matches
                - query_time_ms: Query execution time in milliseconds
                - query: The FTS5 query that was executed
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager unavailable",
                "models": [],
                "total_count": 0,
                "query_time_ms": 0,
                "query": "",
            }
        try:
            result = self.resource_manager.search_models_fts(
                query=query,
                limit=limit,
                offset=offset,
                model_type=model_type,
                tags=tags,
            )
            return {"success": True, **result}
        except OSError as exc:
            logger.error("FTS5 search failed: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "models": [],
                "total_count": 0,
                "query_time_ms": 0,
                "query": "",
            }
        except RuntimeError as exc:
            logger.error("FTS5 search failed: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "models": [],
                "total_count": 0,
                "query_time_ms": 0,
                "query": "",
            }
        except ValueError as exc:
            logger.error("FTS5 search failed: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "models": [],
                "total_count": 0,
                "query_time_ms": 0,
                "query": "",
            }

    def import_batch(self, import_specs: List[Dict[str, str]]) -> Dict[str, Any]:
        """Import multiple models in a batch operation.

        Args:
            import_specs: List of import specifications, each containing:
                - path: Local filesystem path to model file or directory
                - family: Model family name
                - official_name: Display name for the model
                - repo_id: Optional Hugging Face repo ID

        Returns:
            Dict with keys:
                - success: Overall success status
                - imported: Number of successfully imported models
                - failed: Number of failed imports
                - results: List of individual import results
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager unavailable",
                "imported": 0,
                "failed": 0,
                "results": [],
            }
        try:
            return self.resource_manager.import_batch(import_specs)
        except OSError as exc:
            logger.error("Batch import failed: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "imported": 0,
                "failed": len(import_specs),
                "results": [],
            }
        except ValueError as exc:
            logger.error("Batch import failed: %s", exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "imported": 0,
                "failed": len(import_specs),
                "results": [],
            }

    def get_network_status(self) -> Dict[str, Any]:
        """Get network status including circuit breaker state.

        Returns network statistics for monitoring and UI indicators:
            - total_requests: Total number of network requests made
            - successful_requests: Number of successful requests
            - failed_requests: Number of failed requests
            - circuit_breaker_rejections: Requests rejected by circuit breaker
            - retries: Total number of retry attempts
            - success_rate: Success rate as percentage
            - circuit_states: Map of domain to circuit state
            - is_offline: True if any circuit breaker is OPEN

        Returns:
            Dict with network statistics
        """
        from dataclasses import asdict

        from backend.model_library.network import CircuitState, NetworkManager

        try:
            # Use global network manager for persistent circuit state tracking
            if not hasattr(self, "_network_manager"):
                self._network_manager = NetworkManager(
                    circuit_failure_threshold=3,  # 3 failures before circuit opens
                    circuit_recovery_timeout=60.0,  # 60s before circuit closes
                )

            manager = self._network_manager
            stats = manager.get_stats()
            stats_dict = asdict(stats)
            stats_dict["success_rate"] = stats.success_rate

            # Add circuit breaker states
            circuit_states = manager.get_all_circuit_states()
            stats_dict["circuit_states"] = circuit_states

            # Check if any circuit is OPEN (offline indicator)
            is_offline = any(state == CircuitState.OPEN.value for state in circuit_states.values())
            stats_dict["is_offline"] = is_offline

            return {"success": True, **stats_dict}
        except OSError as exc:
            logger.error("Failed to get network status: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Failed to get network status: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def get_model_overrides(self, rel_path: str) -> ModelOverrides:
        """Fetch overrides for a model by relative path."""
        if not self.resource_manager:
            return {}
        return self.resource_manager.get_model_overrides(rel_path)

    def update_model_overrides(self, rel_path: str, overrides: ModelOverrides) -> bool:
        """Update overrides for a model by relative path."""
        if not self.resource_manager:
            return False
        return self.resource_manager.update_model_overrides(rel_path, overrides)

    def get_link_health(self, version_tag: Optional[str] = None) -> Dict[str, Any]:
        """Get health status of model symlinks.

        Checks for broken links, orphaned links, and cross-filesystem warnings.

        Args:
            version_tag: Optional version tag to check orphaned links for

        Returns:
            Dict with health check results including status, broken/orphaned links
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available"}

        try:
            app_models_root = None
            if version_tag:
                if not validate_version_tag(version_tag):
                    logger.warning("Rejected link health check for invalid tag: %r", version_tag)
                    return {"success": False, "error": "Invalid version tag"}
                versions_dir = self.resource_manager.versions_dir
                app_models_root = versions_dir / version_tag / "models"

            result = self.resource_manager.get_link_health(app_models_root)
            return {"success": True, **result}
        except OSError as exc:
            logger.error("Failed to get link health: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Failed to get link health: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def clean_broken_links(self) -> Dict[str, Any]:
        """Remove broken links from the registry and filesystem.

        Returns:
            Dict with cleanup results
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available", "cleaned": 0}

        try:
            return self.resource_manager.clean_broken_links()
        except OSError as exc:
            logger.error("Failed to clean broken links: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "cleaned": 0}

    def remove_orphaned_links(self, version_tag: str) -> Dict[str, Any]:
        """Remove orphaned symlinks from a version's models directory.

        Args:
            version_tag: Version tag to clean orphaned links from

        Returns:
            Dict with cleanup results
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available", "removed": 0}

        if not validate_version_tag(version_tag):
            logger.warning("Rejected orphan removal for invalid tag: %r", version_tag)
            return {"success": False, "error": "Invalid version tag", "removed": 0}

        try:
            versions_dir = self.resource_manager.versions_dir
            app_models_root = versions_dir / version_tag / "models"
            return self.resource_manager.remove_orphaned_links(app_models_root)
        except OSError as exc:
            logger.error("Failed to remove orphaned links: %s", exc, exc_info=True)
            return {"success": False, "error": str(exc), "removed": 0}

    def get_links_for_model(self, model_id: str) -> Dict[str, Any]:
        """Get all links for a specific model.

        Args:
            model_id: ID of the model

        Returns:
            Dict with list of link information
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available", "links": []}

        try:
            links = self.resource_manager.get_links_for_model(model_id)
            return {"success": True, "links": links}
        except OSError as exc:
            logger.error("Failed to get links for model %s: %s", model_id, exc, exc_info=True)
            return {"success": False, "error": str(exc), "links": []}

    def delete_model_with_cascade(self, model_id: str) -> Dict[str, Any]:
        """Delete a model and all its symlinks.

        Args:
            model_id: ID of the model to delete

        Returns:
            Dict with deletion results
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available", "links_removed": 0}

        try:
            return self.resource_manager.delete_model_with_cascade(model_id)
        except OSError as exc:
            logger.error("Failed to cascade delete model %s: %s", model_id, exc, exc_info=True)
            return {"success": False, "error": str(exc), "links_removed": 0}

    def preview_model_mapping(self, version_tag: str) -> Dict[str, Any]:
        """Preview model mapping operations without making changes.

        Performs a dry run to show what would happen if model mapping was applied.
        Shows links to create, conflicts, broken links to remove, and warnings.

        Args:
            version_tag: ComfyUI version tag to preview mapping for

        Returns:
            Dict with preview information including to_create, conflicts, warnings
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available"}

        if not validate_version_tag(version_tag):
            logger.warning("Rejected mapping preview for invalid tag: %r", version_tag)
            return {"success": False, "error": "Invalid version tag"}

        try:
            return self.resource_manager.preview_model_mapping(version_tag)
        except OSError as exc:
            logger.error("Failed to preview mapping for %s: %s", version_tag, exc, exc_info=True)
            return {"success": False, "error": str(exc)}
        except RuntimeError as exc:
            logger.error("Failed to preview mapping for %s: %s", version_tag, exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def sync_models_incremental(self, version_tag: str, model_ids: List[str]) -> Dict[str, Any]:
        """Incrementally sync specific models to a version.

        Much faster than full sync when only a few models were added.

        Args:
            version_tag: ComfyUI version tag
            model_ids: List of model IDs (library paths) to sync

        Returns:
            Dict with sync results
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager not available",
                "links_created": 0,
                "links_updated": 0,
                "links_skipped": 0,
            }

        if not validate_version_tag(version_tag):
            logger.warning("Rejected incremental sync for invalid tag: %r", version_tag)
            return {
                "success": False,
                "error": "Invalid version tag",
                "links_created": 0,
                "links_updated": 0,
                "links_skipped": 0,
            }

        try:
            return self.resource_manager.sync_models_incremental(version_tag, model_ids)
        except OSError as exc:
            logger.error("Failed incremental sync for %s: %s", version_tag, exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "links_created": 0,
                "links_updated": 0,
                "links_skipped": 0,
            }

    def get_cross_filesystem_warning(self, version_tag: str) -> Dict[str, Any]:
        """Check if library and app version are on different filesystems.

        Args:
            version_tag: ComfyUI version tag

        Returns:
            Dict with warning information if cross-filesystem, empty dict otherwise
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager not available"}

        if not validate_version_tag(version_tag):
            logger.warning("Rejected cross-fs check for invalid tag: %r", version_tag)
            return {"success": False, "error": "Invalid version tag"}

        try:
            warning = self.resource_manager.get_cross_filesystem_warning(version_tag)
            if warning:
                return {"success": True, **warning}
            return {"success": True, "cross_filesystem": False}
        except OSError as exc:
            logger.error("Failed cross-fs check for %s: %s", version_tag, exc, exc_info=True)
            return {"success": False, "error": str(exc)}

    def apply_model_mapping(self, version_tag: str) -> Dict[str, Any]:
        """Apply model mapping for a specific version.

        This function:
        1. Removes broken symlinks (targets no longer exist)
        2. Re-applies all mapping configurations
        3. Recreates missing symlinks
        4. Creates new symlinks for newly imported models

        Args:
            version_tag: ComfyUI version tag

        Returns:
            Dict with sync results including links_created, links_removed, total_links
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager not available",
                "links_created": 0,
                "links_removed": 0,
                "total_links": 0,
            }

        if not validate_version_tag(version_tag):
            logger.warning("Rejected apply mapping for invalid tag: %r", version_tag)
            return {
                "success": False,
                "error": "Invalid version tag",
                "links_created": 0,
                "links_removed": 0,
                "total_links": 0,
            }

        try:
            return self.resource_manager.apply_model_mapping(version_tag)
        except OSError as exc:
            logger.error("Failed to apply mapping for %s: %s", version_tag, exc, exc_info=True)
            return {
                "success": False,
                "error": str(exc),
                "links_created": 0,
                "links_removed": 0,
                "total_links": 0,
            }

    def sync_with_resolutions(
        self, version_tag: str, resolutions: Dict[str, str]
    ) -> Dict[str, Any]:
        """Apply model mapping with user-provided conflict resolutions.

        For conflicts, applies the resolution chosen by the user:
        - 'skip': Do not create link, leave existing file
        - 'overwrite': Delete existing, create new link
        - 'rename': Rename existing to .old, create new link

        Args:
            version_tag: ComfyUI version tag
            resolutions: Dict mapping model_id to resolution action

        Returns:
            Dict with sync results including links_created, links_skipped,
            links_renamed, overwrites, errors
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager not available",
                "links_created": 0,
                "links_skipped": 0,
                "links_renamed": 0,
                "overwrites": 0,
                "errors": [],
            }

        if not validate_version_tag(version_tag):
            logger.warning("Rejected sync with resolutions for invalid tag: %r", version_tag)
            return {
                "success": False,
                "error": "Invalid version tag",
                "links_created": 0,
                "links_skipped": 0,
                "links_renamed": 0,
                "overwrites": 0,
                "errors": [],
            }

        # Validate resolution values
        valid_actions = {"skip", "overwrite", "rename"}
        for model_id, action in resolutions.items():
            if action not in valid_actions:
                return {
                    "success": False,
                    "error": f"Invalid resolution action '{action}' for model {model_id}",
                    "links_created": 0,
                    "links_skipped": 0,
                    "links_renamed": 0,
                    "overwrites": 0,
                    "errors": [],
                }

        try:
            return self.resource_manager.sync_with_resolutions(version_tag, resolutions)
        except OSError as exc:
            logger.error(
                "Failed to sync with resolutions for %s: %s", version_tag, exc, exc_info=True
            )
            return {
                "success": False,
                "error": str(exc),
                "links_created": 0,
                "links_skipped": 0,
                "links_renamed": 0,
                "overwrites": 0,
                "errors": [str(exc)],
            }

    def get_custom_nodes(self, version_tag: str) -> List[str]:
        """Get list of custom nodes for a specific version"""
        if not self.resource_manager:
            return []
        if not validate_version_tag(version_tag):
            logger.warning(f"Rejected custom node list for invalid tag: {version_tag!r}")
            return []
        return self.resource_manager.list_version_custom_nodes(version_tag)

    def install_custom_node(
        self, git_url: str, version_tag: str, node_name: Optional[str] = None
    ) -> bool:
        """Install a custom node for a specific version"""
        if not self.resource_manager:
            return False
        if not validate_version_tag(version_tag):
            logger.warning(f"Rejected custom node install for invalid tag: {version_tag!r}")
            return False
        if not validate_url(git_url):
            logger.warning(f"Rejected custom node install for invalid URL: {git_url!r}")
            return False
        if node_name and not validate_package_name(node_name):
            logger.warning(f"Rejected custom node install for invalid name: {node_name!r}")
            return False
        return self.resource_manager.install_custom_node(git_url, version_tag, node_name)

    def update_custom_node(self, node_name: str, version_tag: str) -> bool:
        """Update a custom node to latest version"""
        if not self.resource_manager:
            return False
        if not validate_version_tag(version_tag):
            logger.warning(f"Rejected custom node update for invalid tag: {version_tag!r}")
            return False
        if not validate_package_name(node_name):
            logger.warning(f"Rejected custom node update for invalid name: {node_name!r}")
            return False
        return self.resource_manager.update_custom_node(node_name, version_tag)

    def remove_custom_node(self, node_name: str, version_tag: str) -> bool:
        """Remove a custom node from a specific version"""
        if not self.resource_manager:
            return False
        if not validate_version_tag(version_tag):
            logger.warning(f"Rejected custom node removal for invalid tag: {version_tag!r}")
            return False
        if not validate_package_name(node_name):
            logger.warning(f"Rejected custom node removal for invalid name: {node_name!r}")
            return False
        return self.resource_manager.remove_custom_node(node_name, version_tag)

    def scan_shared_storage(self) -> ScanResult:
        """Scan shared storage and get statistics"""
        if not self.resource_manager:
            return {
                "modelsFound": 0,
                "workflowsFound": 0,
                "customNodesFound": 0,
                "totalSize": 0,
            }
        return self.resource_manager.scan_shared_storage()

    # =========================================================================
    # Model Import - HuggingFace Metadata Lookup
    # =========================================================================

    def lookup_hf_metadata_for_file(
        self,
        filename: str,
        file_path: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Look up HuggingFace metadata for a given filename.

        Uses hybrid filename + hash verification for accurate matching.

        Args:
            filename: Name of the model file
            file_path: Optional path to local file for hash verification

        Returns:
            Dict with success, found, metadata, or error
        """
        if not self.resource_manager:
            return {
                "success": False,
                "found": False,
                "error": "Resource manager unavailable",
            }

        try:
            path = Path(file_path) if file_path else None
            metadata = self.resource_manager.lookup_hf_metadata(filename, path)

            if metadata:
                return {
                    "success": True,
                    "found": True,
                    "metadata": metadata,
                }
            else:
                return {
                    "success": True,
                    "found": False,
                    "metadata": None,
                }
        except OSError as e:
            logger.error("Error looking up metadata: %s", e, exc_info=True)
            return {
                "success": False,
                "found": False,
                "error": str(e),
            }
        except RuntimeError as e:
            logger.error("Error looking up metadata: %s", e, exc_info=True)
            return {
                "success": False,
                "found": False,
                "error": str(e),
            }

    def detect_sharded_sets(
        self,
        file_paths: List[str],
    ) -> Dict[str, Any]:
        """Detect and group sharded model files.

        Args:
            file_paths: List of file paths to analyze

        Returns:
            Dict with groups mapping base names to file lists,
            and validation info for each group
        """
        from backend.model_library.importer import detect_sharded_sets as detect_sets
        from backend.model_library.importer import validate_shard_completeness

        try:
            paths = [Path(p) for p in file_paths]
            groups = detect_sets(paths)

            result_groups = {}
            for base_name, shard_files in groups.items():
                validation = validate_shard_completeness(shard_files)
                result_groups[base_name] = {
                    "files": [str(p) for p in shard_files],
                    "validation": validation,
                }

            return {
                "success": True,
                "groups": result_groups,
            }
        except OSError as e:
            logger.error("Error detecting sharded sets: %s", e, exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "groups": {},
            }

    def validate_file_type(self, file_path: str) -> Dict[str, Any]:
        """Validate file type using magic bytes.

        Prevents importing .txt/.html files masquerading as models.

        Args:
            file_path: Path to file to validate

        Returns:
            Dict with valid, detected_type, and optional error
        """
        from backend.model_library.importer import validate_file_type as validate_type

        try:
            result = validate_type(Path(file_path))
            return {"success": True, **result}
        except OSError as e:
            logger.error("Error validating file type: %s", e, exc_info=True)
            return {
                "success": False,
                "valid": False,
                "detected_type": "error",
                "error": str(e),
            }

    def mark_metadata_as_manual(self, model_id: str) -> Dict[str, Any]:
        """Mark model metadata as manually corrected to protect from auto-updates.

        Args:
            model_id: Model ID to protect

        Returns:
            Dict with success status
        """
        if not self.resource_manager:
            return {"success": False, "error": "Resource manager unavailable"}

        try:
            success = self.resource_manager.mark_metadata_as_manual(model_id)
            if success:
                return {"success": True}
            else:
                return {"success": False, "error": "Model not found"}
        except OSError as e:
            logger.error("Error marking metadata as manual: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}

    def get_library_status(self) -> Dict[str, Any]:
        """Get current library status, including indexing state.

        Returns:
            Dict with indexing state and model count
        """
        if not self.resource_manager:
            return {
                "success": False,
                "error": "Resource manager unavailable",
                "indexing": False,
                "model_count": 0,
            }

        try:
            status = self.resource_manager.get_library_status()
            return {"success": True, **status}
        except OSError as e:
            logger.error("Error getting library status: %s", e, exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "indexing": False,
                "model_count": 0,
            }

    def get_sandbox_info(self) -> Dict[str, Any]:
        """Get information about sandbox environment.

        Detects if running in Flatpak, Snap, Docker, or AppImage and returns
        any limitations that may affect model library operations.

        Returns:
            Dict with sandbox detection results:
                - success: Whether detection completed
                - is_sandboxed: True if running in sandboxed environment
                - sandbox_type: Type of sandbox (flatpak, snap, docker, appimage, none)
                - limitations: List of limitation strings
        """
        try:
            info = detect_sandbox_environment()
            return {
                "success": True,
                "is_sandboxed": info.is_sandboxed,
                "sandbox_type": info.sandbox_type,
                "limitations": info.limitations,
            }
        except OSError as e:
            logger.error("Error detecting sandbox: %s", e, exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "is_sandboxed": False,
                "sandbox_type": "unknown",
                "limitations": [],
            }

    def get_file_link_count(self, file_path: str) -> Dict[str, Any]:
        """Get number of hard links for a file.

        Used to warn users about hard-linked files on NTFS.

        Args:
            file_path: Path to file

        Returns:
            Dict with link_count and is_hard_linked flag
        """
        try:
            path = Path(file_path)
            link_count = path.stat().st_nlink
            return {
                "success": True,
                "link_count": link_count,
                "is_hard_linked": link_count > 1,
            }
        except OSError as e:
            logger.error("Failed to get link count: %s", e, exc_info=True)
            return {
                "success": False,
                "link_count": 1,
                "is_hard_linked": False,
                "error": str(e),
            }

    def check_files_writable(self, file_paths: List[str]) -> Dict[str, Any]:
        """Check if files can be safely deleted.

        Args:
            file_paths: List of file paths to check

        Returns:
            Dict with all_writable flag and per-file details
        """
        import os

        details = []
        all_writable = True

        for file_path in file_paths:
            try:
                path = Path(file_path)
                writable = path.exists() and os.access(path.parent, os.W_OK)

                details.append(
                    {
                        "path": str(path),
                        "writable": writable,
                        "reason": (
                            None if writable else "Read-only filesystem or no write permission"
                        ),
                    }
                )

                if not writable:
                    all_writable = False

            except OSError as e:
                logger.warning("Failed to check writability for %s: %s", file_path, e)
                details.append(
                    {
                        "path": file_path,
                        "writable": False,
                        "reason": str(e),
                    }
                )
                all_writable = False

        return {
            "success": True,
            "all_writable": all_writable,
            "details": details,
        }

    # ==================== Launcher Update Methods ====================

    def get_launcher_version(self) -> Dict[str, Any]:
        """Get current launcher version (git commit).

        Returns:
            Dict with version info: version, branch, isGitRepo
        """
        try:
            from backend.__version__ import __branch__, __version__, is_git_repo

            return {
                "success": True,
                "version": __version__,
                "branch": __branch__,
                "isGitRepo": is_git_repo(),
            }
        except ImportError as e:
            logger.error("Failed to get launcher version: %s", e, exc_info=True)
            return {"success": False, "error": str(e), "version": "unknown"}

    def check_launcher_updates(self, force_refresh: bool = False) -> Dict[str, Any]:
        """Check if launcher updates are available.

        Args:
            force_refresh: Force refresh from remote

        Returns:
            Dict with update info: hasUpdate, currentCommit, latestCommit, etc.
        """
        try:
            if self.metadata_manager is None:
                return {
                    "success": False,
                    "error": "Metadata manager not initialized",
                    "hasUpdate": False,
                }

            if self.launcher_updater is None:
                from backend.launcher_updater import LauncherUpdater

                self.launcher_updater = LauncherUpdater(self.metadata_manager)

            result = self.launcher_updater.check_for_updates(force_refresh)
            return {"success": True, **result}
        except ImportError as e:
            logger.error("Failed to check launcher updates: %s", e, exc_info=True)
            return {"success": False, "error": str(e), "hasUpdate": False}
        except OSError as e:
            logger.error("Failed to check launcher updates: %s", e, exc_info=True)
            return {"success": False, "error": str(e), "hasUpdate": False}
        except RuntimeError as e:
            logger.error("Failed to check launcher updates: %s", e, exc_info=True)
            return {"success": False, "error": str(e), "hasUpdate": False}

    def apply_launcher_update(self) -> Dict[str, Any]:
        """Apply launcher update (pull + rebuild).

        Returns:
            Dict with result: success, message, newCommit
        """
        try:
            if self.metadata_manager is None:
                return {"success": False, "error": "Metadata manager not initialized"}

            if self.launcher_updater is None:
                from backend.launcher_updater import LauncherUpdater

                self.launcher_updater = LauncherUpdater(self.metadata_manager)

            result = self.launcher_updater.apply_update()
            return {"success": result.get("success", False), **result}
        except ImportError as e:
            logger.error("Failed to apply launcher update: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}
        except OSError as e:
            logger.error("Failed to apply launcher update: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}
        except RuntimeError as e:
            logger.error("Failed to apply launcher update: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}

    def restart_launcher(self) -> Dict[str, Any]:
        """Restart the launcher application.

        Returns:
            Dict with result: success, message
        """
        import os
        import subprocess
        import sys
        import threading

        try:
            # Get the launcher script path
            launcher_root = self.script_dir
            launcher_script = launcher_root / "launcher"

            if launcher_script.exists():
                # Restart via launcher script
                subprocess.Popen([str(launcher_script)], start_new_session=True)
            else:
                # Restart Python directly
                python = sys.executable
                subprocess.Popen(
                    [python, str(launcher_root / "backend" / "main.py")],
                    start_new_session=True,
                )

            # Exit current process after a brief delay
            def delayed_exit():
                import time

                time.sleep(1)
                os._exit(0)

            threading.Thread(target=delayed_exit, daemon=True).start()

            return {"success": True, "message": "Restarting..."}
        except OSError as e:
            logger.error("Failed to restart launcher: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}
        except RuntimeError as e:
            logger.error("Failed to restart launcher: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}
