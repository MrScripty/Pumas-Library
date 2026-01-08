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

try:
    import psutil
except ImportError as exc:
    logger.debug("psutil not available; system resource info will be limited: %s", exc)
    psutil = None  # type: ignore


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

        running_processes = self.process_manager.get_processes_with_resources()
        running = bool(running_processes)

        # Aggregate per-app resources
        app_resources: Dict[str, Dict[str, float]] = {}

        # Map ComfyUI processes to app resources
        if running_processes:
            total_cpu = 0.0
            total_ram_memory = 0.0
            total_gpu_memory = 0.0

            for proc in running_processes:
                total_cpu += proc.get("cpu_usage", 0.0)
                total_ram_memory += proc.get("ram_memory", 0.0)
                total_gpu_memory += proc.get("gpu_memory", 0.0)

            # All ComfyUI processes are aggregated under "comfyui" app
            app_resources["comfyui"] = {
                "cpu": round(total_cpu, 1),
                "ram_memory": round(total_ram_memory, 2),
                "gpu_memory": round(total_gpu_memory, 2),
            }

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
            "app_resources": app_resources,
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
        except OSError as e:
            logger.error("Failed to read disk space: %s", e, exc_info=True)
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
                        return {
                            "success": False,
                            "error": f"xdg-open returned {result.returncode}",
                        }
                    return {"success": True}
                return {"success": False, "error": "Unable to open browser"}
            return {"success": True}
        except subprocess.SubprocessError as e:
            logger.error("Failed to open browser via subprocess: %s", e, exc_info=True)
            return {"success": False, "error": str(e)}
        except OSError as e:
            logger.error("Failed to open browser: %s", e, exc_info=True)
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
            return {
                "success": False,
                "error": "No active version or installation incomplete",
            }

        return self.open_path(str(active_path))

    def get_system_resources(self) -> Dict[str, Any]:
        """
        Get current system resource usage (CPU, GPU, RAM, Disk)

        Returns:
            Dictionary with system resource information
        """
        if not psutil:
            return {
                "success": False,
                "error": "psutil not available",
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }

        try:
            # Get CPU usage
            cpu_percent = psutil.cpu_percent(interval=0.1)

            # Get RAM usage
            ram = psutil.virtual_memory()
            ram_usage_gb = (ram.total - ram.available) / (1024**3)
            ram_total_gb = ram.total / (1024**3)

            # Get disk usage
            disk = shutil.disk_usage(self.script_dir)
            disk_usage_gb = disk.used / (1024**3)
            disk_total_gb = disk.total / (1024**3)
            disk_free_gb = disk.free / (1024**3)

            # Try to get GPU usage (NVIDIA only for now)
            gpu_usage = 0.0
            gpu_memory = 0.0
            gpu_memory_total = 0.0
            gpu_temp = None

            try:
                # Check if nvidia-smi is available - get utilization, used memory, and total memory
                result = subprocess.run(
                    [
                        "nvidia-smi",
                        "--query-gpu=utilization.gpu,memory.used,memory.total",
                        "--format=csv,noheader,nounits",
                    ],
                    capture_output=True,
                    text=True,
                    timeout=2,
                )
                if result.returncode == 0:
                    lines = result.stdout.strip().split("\n")
                    if lines:
                        # Get first GPU
                        parts = lines[0].split(",")
                        if len(parts) >= 3:
                            gpu_usage = float(parts[0].strip())
                            gpu_memory = float(parts[1].strip()) / 1024  # Convert MB to GB
                            gpu_memory_total = float(parts[2].strip()) / 1024  # Convert MB to GB
            except subprocess.SubprocessError as exc:
                logger.debug("nvidia-smi failed: %s", exc)
            except OSError as exc:
                logger.debug("nvidia-smi failed: %s", exc)
            except ValueError as exc:
                logger.debug("nvidia-smi output invalid: %s", exc)

            return {
                "success": True,
                "resources": {
                    "cpu": {"usage": round(cpu_percent, 1), "temp": None},
                    "gpu": {
                        "usage": round(gpu_usage, 1),
                        "memory": round(gpu_memory, 1),
                        "memory_total": round(gpu_memory_total, 1),
                        "temp": gpu_temp,
                    },
                    "ram": {
                        "usage": round(ram_usage_gb, 1),
                        "total": round(ram_total_gb, 1),
                    },
                    "disk": {
                        "usage": round(disk_usage_gb, 1),
                        "total": round(disk_total_gb, 1),
                        "free": round(disk_free_gb, 1),
                    },
                },
            }
        except OSError as e:
            logger.error(f"Failed to get system resources: {e}", exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }
        except RuntimeError as e:
            logger.error(f"Failed to get system resources: {e}", exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }
        except TypeError as e:
            logger.error(f"Failed to get system resources: {e}", exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }
        except ValueError as e:
            logger.error(f"Failed to get system resources: {e}", exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }
        except subprocess.SubprocessError as e:
            logger.error(f"Failed to get system resources: {e}", exc_info=True)
            return {
                "success": False,
                "error": str(e),
                "resources": {
                    "cpu": {"usage": 0, "temp": None},
                    "gpu": {"usage": 0, "memory": 0, "memory_total": 0, "temp": None},
                    "ram": {"usage": 0, "total": 0},
                    "disk": {"usage": 0, "total": 0, "free": 0},
                },
            }
