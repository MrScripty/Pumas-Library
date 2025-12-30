#!/usr/bin/env python3
"""
System Utilities for ComfyUI Setup API
Handles status reporting, disk space, and file/URL operations
"""

import shutil
import subprocess
import webbrowser
from pathlib import Path
from typing import Any, Dict, Optional

from backend.exceptions import ValidationError
from backend.file_opener import open_in_file_manager
from backend.logging_config import get_logger
from backend.validators import sanitize_path, validate_url

logger = get_logger(__name__)


class SystemUtils:
    """System utilities for ComfyUI setup"""

    def __init__(
        self,
        script_dir: Path,
        dependency_manager,
        patch_manager,
        shortcut_manager,
        process_manager,
        version_info_manager,
        version_manager=None,
    ):
        """
        Initialize system utilities

        Args:
            script_dir: Path to launcher directory
            dependency_manager: DependencyManager instance
            patch_manager: PatchManager instance
            shortcut_manager: ShortcutManager instance
            process_manager: ProcessManager instance
            version_info_manager: VersionInfoManager instance
            version_manager: Optional VersionManager instance
        """
        self.script_dir = Path(script_dir)
        self.dependency_manager = dependency_manager
        self.patch_manager = patch_manager
        self.shortcut_manager = shortcut_manager
        self.process_manager = process_manager
        self.version_info_manager = version_info_manager
        self.version_manager = version_manager

    def get_status(self) -> Dict[str, Any]:
        """Get complete system status"""
        missing_deps = self.dependency_manager.get_missing_dependencies()
        deps_ready = len(missing_deps) == 0
        patched = self.patch_manager.is_patched()
        active_version = self.version_manager.get_active_version() if self.version_manager else None

        if active_version:
            shortcut_state = self.shortcut_manager.get_version_shortcut_state(active_version)
            menu = shortcut_state["menu"]
            desktop = shortcut_state["desktop"]
        else:
            menu = False
            desktop = False

        running_processes = self.process_manager._detect_comfyui_processes()
        running = bool(running_processes)

        # Check for new releases
        release_info = self.version_info_manager.check_for_new_release()

        # Determine status message
        if running:
            message = ""  # Suppress running banner text in GUI
        elif not deps_ready:
            message = "Missing dependencies detected."
        elif deps_ready and patched and menu and desktop:
            message = "Setup complete – everything is ready"
        else:
            message = ""

        return {
            "version": self.version_info_manager.get_comfyui_version(),
            "deps_ready": deps_ready,
            "missing_deps": missing_deps,
            "patched": patched,
            "menu_shortcut": menu,
            "desktop_shortcut": desktop,
            "shortcut_version": active_version,
            "comfyui_running": running,
            "running_processes": running_processes,
            "message": message,
            "release_info": release_info,
            "last_launch_log": self.process_manager.last_launch_log,
            "last_launch_error": self.process_manager.last_launch_error,
        }

    def get_disk_space(self) -> Dict[str, Any]:
        """
        Get disk space information for the launcher directory

        Returns:
            Dictionary with total, used, free space in bytes and usage percentage
        """
        try:
            stat = shutil.disk_usage(self.script_dir)
            usage_percent = (stat.used / stat.total) * 100 if stat.total > 0 else 0

            return {
                "success": True,
                "total": stat.total,
                "used": stat.used,
                "free": stat.free,
                "percent": round(usage_percent, 1),
            }
        except (OSError, PermissionError) as e:
            return {
                "success": False,
                "error": str(e),
                "total": 0,
                "used": 0,
                "free": 0,
                "percent": 0,
            }

    def toggle_patch(self) -> bool:
        """Toggle main.py patch"""
        if self.patch_manager.is_patched():
            return bool(self.patch_manager.revert_main_py())
        return bool(self.patch_manager.patch_main_py())

    def toggle_menu(self, tag: Optional[str] = None) -> bool:
        """Toggle menu shortcut (version-specific when available)"""
        target = tag or (
            self.version_manager.get_active_version() if self.version_manager else None
        )

        if target:
            result = self.shortcut_manager.toggle_version_menu_shortcut(target)
            return bool(result.get("success", False))

        logger.warning("No active version available for menu shortcut")
        return False

    def toggle_desktop(self, tag: Optional[str] = None) -> bool:
        """Toggle desktop shortcut (version-specific when available)"""
        target = tag or (
            self.version_manager.get_active_version() if self.version_manager else None
        )

        if target:
            result = self.shortcut_manager.toggle_version_desktop_shortcut(target)
            return bool(result.get("success", False))

        logger.warning("No active version available for desktop shortcut")
        return False

    def open_path(self, path: str) -> Dict[str, Any]:
        """
        Open a filesystem path in the user's file manager (cross-platform).

        Args:
            path: Path to open (absolute or relative to launcher root)

        Returns:
            Dict with success status and optional error message
        """
        try:
            safe_path = sanitize_path(path, self.script_dir)
        except ValidationError as exc:
            logger.warning(f"Rejected open_path for {path!r}: {exc}")
            return {"success": False, "error": "Invalid path"}

        return open_in_file_manager(str(safe_path))

    def open_url(self, url: str) -> Dict[str, Any]:
        """
        Open a URL in the default system browser.

        Args:
            url: URL to open (must start with http:// or https://)

        Returns:
            Dict with success status and optional error message
        """
        if not validate_url(url):
            return {"success": False, "error": "Only http/https URLs are allowed"}

        try:
            opened = webbrowser.open(url, new=2)
            if not opened:
                # Fallback to xdg-open/xdg-utils on Linux
                opener = shutil.which("xdg-open")
                if opener:
                    result = subprocess.run([opener, url], capture_output=True)
                    if result.returncode != 0:
                        return {"success": False, "error": f"xdg-open returned {result.returncode}"}
                    return {"success": True}
                return {"success": False, "error": "Unable to open browser"}
            return {"success": True}
        except (OSError, subprocess.SubprocessError) as e:
            return {"success": False, "error": str(e)}

    def open_active_install(self) -> Dict[str, Any]:
        """
        Open the active ComfyUI installation directory in the file manager.

        Returns:
            Dict with success status and optional error message
        """
        if not self.version_manager:
            return {"success": False, "error": "Version manager not initialized"}

        active_path = self.version_manager.get_active_version_path()
        if not active_path:
            return {"success": False, "error": "No active version or installation incomplete"}

        return self.open_path(str(active_path))
