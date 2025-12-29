#!/usr/bin/env python3
"""
ComfyUI Setup API - Core Module
Main API class that coordinates all setup operations
"""

import sys
import tomllib
from pathlib import Path
from typing import Any, Dict, List, Optional

from backend.api.dependency_manager import DependencyManager
from backend.api.patch_manager import PatchManager
from backend.api.process_manager import ProcessManager
from backend.api.shortcut_manager import ShortcutManager
from backend.api.size_calculator import SizeCalculator
from backend.api.system_utils import SystemUtils
from backend.api.version_info import VersionInfoManager
from backend.logging_config import get_logger
from backend.models import GitHubRelease
from backend.validators import validate_package_name, validate_url, validate_version_tag

logger = get_logger(__name__)


class ComfyUISetupAPI:
    """Main API class for ComfyUI setup operations"""

    def __init__(self):
        # Determine directories based on launcher location
        # Handle both development mode and PyInstaller bundled mode
        if getattr(sys, "frozen", False):
            # Running as PyInstaller bundle
            # Search upward from executable location to find ComfyUI root
            self.comfyui_dir = self._find_comfyui_root(Path(sys.executable).parent)
            # Launcher directory is where run.sh and icon should be
            # Try common locations
            launcher_candidates = [
                self.comfyui_dir / "Linux-ComfyUI-Launcher",
                Path(sys.executable).parent.parent,  # dist/ parent
                Path(sys.executable).parent,  # same dir as executable
            ]
            self.script_dir = None
            for candidate in launcher_candidates:
                if candidate.exists():
                    self.script_dir = candidate
                    break
            if not self.script_dir:
                # Fallback to executable directory
                self.script_dir = Path(sys.executable).parent
        else:
            # Running in development mode
            self.script_dir = Path(__file__).parent.parent.parent.resolve()
            self.comfyui_dir = self.script_dir.parent

        self.main_py = self.comfyui_dir / "main.py"
        self.icon_webp = self.script_dir / "comfyui-icon.webp"
        self.launcher_data_dir = self.script_dir / "launcher-data"
        self.shortcut_scripts_dir = self.launcher_data_dir / "shortcuts"
        self.generated_icons_dir = self.launcher_data_dir / "icons"

        # Ensure directories used by shortcut tooling exist
        self.shortcut_scripts_dir.mkdir(parents=True, exist_ok=True)
        self.generated_icons_dir.mkdir(parents=True, exist_ok=True)

        # Initialize version management components (Phase 2-4)
        self._init_version_management()

        # Initialize specialized managers
        self._init_managers()

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
                except (OSError, tomllib.TOMLDecodeError):
                    pass

            # Move up one directory
            parent = current.parent
            if parent == current:
                # Reached filesystem root
                break
            current = parent

        # Fallback: return the parent of start_path
        return start_path.parent

    def _init_version_management(self):
        """Initialize version management components"""
        try:
            from backend.github_integration import GitHubReleasesFetcher
            from backend.metadata_manager import MetadataManager
            from backend.package_size_resolver import PackageSizeResolver
            from backend.release_data_fetcher import ReleaseDataFetcher
            from backend.release_size_calculator import ReleaseSizeCalculator
            from backend.resource_manager import ResourceManager
            from backend.version_manager import VersionManager

            launcher_data_dir = self.script_dir / "launcher-data"
            cache_dir = launcher_data_dir / "cache"

            self.metadata_manager = MetadataManager(launcher_data_dir)
            self.github_fetcher = GitHubReleasesFetcher(self.metadata_manager)
            self.resource_manager = ResourceManager(self.script_dir, self.metadata_manager)
            self.version_manager = VersionManager(
                self.script_dir, self.metadata_manager, self.github_fetcher, self.resource_manager
            )

            # Initialize size calculation components (Phase 6.2.5a)
            self.release_data_fetcher = ReleaseDataFetcher(cache_dir)
            self.package_size_resolver = PackageSizeResolver(cache_dir)
            self.release_size_calculator = ReleaseSizeCalculator(
                cache_dir, self.release_data_fetcher, self.package_size_resolver, cache_dir / "pip"
            )

            self._prefetch_releases_if_needed()
        except (ImportError, OSError, RuntimeError, TypeError, ValueError) as e:
            logger.warning(f"Version management initialization failed: {e}", exc_info=True)
            self.metadata_manager = None
            self.github_fetcher = None
            self.resource_manager = None
            self.version_manager = None
            self.release_size_calculator = None

    def _init_managers(self):
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

    def _prefetch_releases_if_needed(self):
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
                except (KeyError, TypeError, ValueError) as e:
                    logger.warning(
                        f"Error checking cache validity: {e} - prefetching", exc_info=True
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
                except (OSError, RuntimeError, TypeError, ValueError) as exc:
                    logger.error(f"Background prefetch failed: {exc}", exc_info=True)
                    logger.info("App will continue using stale cache")

            import threading

            threading.Thread(target=_background_fetch, daemon=True).start()

        except (OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Prefetch init error: {e}", exc_info=True)

    def has_background_fetch_completed(self) -> bool:
        """Check if background fetch has completed (for frontend polling)"""
        return getattr(self, "_background_fetch_completed", False)

    def reset_background_fetch_flag(self):
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
        releases = []

        # Try to fetch (optionally forced); on failure, fall back to cached data without clearing it
        try:
            releases = self.version_manager.get_available_releases(force_refresh)
            releases_source = "remote" if force_refresh else "cache/remote"
        except (OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(
                f"Error fetching releases (force_refresh={force_refresh}): {e}", exc_info=True
            )
            releases = []

        if force_refresh and not releases:
            try:
                cache = self.metadata_manager.load_github_cache() if self.metadata_manager else None
                if cache and cache.get("releases"):
                    releases = cache.get("releases", [])
                    releases_source = "cache-fallback"
                    logger.info("Using cached releases due to fetch error/rate-limit.")
            except (OSError, TypeError, ValueError) as e:
                logger.error(
                    f"Error loading cached releases after fetch failure: {e}", exc_info=True
                )

        # Enrich releases with size information (Phase 6.2.5c) + installing flag
        installing_tag = None
        active_progress = None
        try:
            active_progress = self.version_manager.get_installation_progress()
            if active_progress and not active_progress.get("completed_at"):
                installing_tag = active_progress.get("tag")
        except (OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Error checking installation progress for releases: {e}", exc_info=True)

        enriched_releases = []
        for release in releases:
            tag = release.get("tag_name", "")

            # Get cached size data if available
            size_data = self.release_size_calculator.get_cached_size(tag)

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
            self.size_calc._refresh_release_sizes_async(
                enriched_releases, installed_tags, force_refresh
            )
        except (OSError, RuntimeError, TypeError, ValueError) as e:
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

    def install_version(self, tag: str, progress_callback=None) -> bool:
        """Install a ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected install for invalid tag: {tag!r}")
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
        return self.version_manager.cancel_installation()

    def remove_version(self, tag: str) -> bool:
        """Remove an installed ComfyUI version"""
        if not self.version_manager:
            return False
        if not validate_version_tag(tag):
            logger.warning(f"Rejected removal for invalid tag: {tag!r}")
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

    def check_version_dependencies(self, tag: str) -> Dict[str, Any]:
        """Check dependency installation status for a version"""
        if not self.version_manager:
            return {"installed": [], "missing": []}
        if not validate_version_tag(tag):
            logger.warning(f"Rejected dependency check for invalid tag: {tag!r}")
            return {"installed": [], "missing": []}
        return self.version_manager.check_dependencies(tag)

    def install_version_dependencies(self, tag: str, progress_callback=None) -> bool:
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

    def get_version_info(self, tag: str) -> Dict[str, Any]:
        """Get detailed information about a specific version"""
        if not self.version_manager:
            return {}
        if not validate_version_tag(tag):
            logger.warning(f"Rejected version info request for invalid tag: {tag!r}")
            return {}
        return self.version_manager.get_version_info(tag)

    def launch_version(self, tag: str, extra_args: List[str] = None) -> Dict[str, Any]:
        """Launch a specific ComfyUI version"""
        if not self.version_manager:
            return {"success": False, "error": "Version manager unavailable"}
        if not validate_version_tag(tag):
            logger.warning(f"Rejected launch for invalid tag: {tag!r}")
            return {"success": False, "error": "Invalid version tag"}
        success, process, log_path, error_msg, ready = self.version_manager.launch_version(
            tag, extra_args
        )
        return {"success": success, "log_path": log_path, "ready": ready, "error": error_msg}

    # ==================== Size Calculation API ====================

    def calculate_release_size(
        self, tag: str, force_refresh: bool = False
    ) -> Optional[Dict[str, Any]]:
        """Calculate total download size for a release (Phase 6.2.5c)"""
        return self.size_calc.calculate_release_size(tag, force_refresh)

    def calculate_all_release_sizes(self, progress_callback=None) -> Dict[str, Dict[str, Any]]:
        """Calculate sizes for all available releases (Phase 6.2.5c)"""
        return self.size_calc.calculate_all_release_sizes(progress_callback)

    def get_release_size_info(self, tag: str, archive_size: int) -> Optional[Dict[str, Any]]:
        """Get size information for a release (Phase 6.2.5a/c)"""
        return self.size_calc.get_release_size_info(tag, archive_size)

    def get_release_size_breakdown(self, tag: str) -> Optional[Dict[str, Any]]:
        """Get size breakdown for display (Phase 6.2.5c)"""
        return self.size_calc.get_release_size_breakdown(tag)

    def get_release_dependencies(
        self, tag: str, top_n: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """Get dependencies for a release sorted by size (Phase 6.2.5c)"""
        return self.size_calc.get_release_dependencies(tag, top_n)

    # ==================== Resource Management API (Phase 5) ====================

    def get_models(self) -> Dict[str, Any]:
        """Get list of models in shared storage"""
        if not self.resource_manager:
            return {}
        return self.resource_manager.get_models()

    def get_custom_nodes(self, version_tag: str) -> List[str]:
        """Get list of custom nodes for a specific version"""
        if not self.resource_manager:
            return []
        if not validate_version_tag(version_tag):
            logger.warning(f"Rejected custom node list for invalid tag: {version_tag!r}")
            return []
        return self.resource_manager.list_version_custom_nodes(version_tag)

    def install_custom_node(self, git_url: str, version_tag: str, node_name: str = None) -> bool:
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

    def scan_shared_storage(self) -> Dict[str, Any]:
        """Scan shared storage and get statistics"""
        if not self.resource_manager:
            return {"modelCount": 0, "totalSize": 0, "categoryCounts": {}}
        return self.resource_manager.scan_shared_storage()
