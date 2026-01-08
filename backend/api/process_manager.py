#!/usr/bin/env python3
"""
Process Manager for ComfyUI
Handles process detection, launching, and stopping
"""

import os
import subprocess
import time
from pathlib import Path
from typing import Any, Dict, List, Optional

from backend.api.process_resource_tracker import ProcessResourceTracker
from backend.logging_config import get_logger

logger = get_logger(__name__)


class ProcessManager:
    """Manages ComfyUI process lifecycle"""

    def __init__(self, comfyui_dir: Path, version_manager=None):
        """
        Initialize process manager

        Args:
            comfyui_dir: Path to ComfyUI root directory
            version_manager: Optional VersionManager instance
        """
        self.comfyui_dir = Path(comfyui_dir)
        self.version_manager = version_manager
        self.last_launch_log: Optional[str] = None
        self.last_launch_error: Optional[str] = None
        self.resource_tracker = ProcessResourceTracker(cache_ttl=2.0)

    def _get_known_version_paths(self) -> Dict[str, Path]:
        """Return a mapping of installed version tags to their paths"""
        tag_paths: Dict[str, Path] = {}
        if not self.version_manager:
            return tag_paths

        try:
            for tag in self.version_manager.get_installed_versions():
                version_path = self.version_manager.get_version_path(tag)
                if version_path:
                    tag_paths[tag] = version_path
        except OSError as e:
            logger.error(f"Error collecting version paths: {e}", exc_info=True)
        except RuntimeError as e:
            logger.error(f"Error collecting version paths: {e}", exc_info=True)
        except TypeError as e:
            logger.error(f"Error collecting version paths: {e}", exc_info=True)
        except ValueError as e:
            logger.error(f"Error collecting version paths: {e}", exc_info=True)

        return tag_paths

    def _detect_comfyui_processes(self) -> List[Dict[str, Any]]:
        """
        Detect running ComfyUI processes using multiple detection methods.

        Detection strategy:
        1. Check PID files in version directories (most reliable)
        2. Scan process table for Python processes running main.py
        3. Verify processes are actually alive and ComfyUI instances

        Each process is deduplicated by PID to avoid duplicates across
        detection methods.

        Returns:
            List of dictionaries containing process information:
                - pid (int): Process ID
                - source (str): Detection method ('pid_file' or 'process_scan')
                - tag (str): Version tag if identified
                - cmdline (str): Command line arguments
                - exe (str): Executable path

        Side Effects:
            - Reads PID files from version directories
            - Scans system process table if psutil is available
            - May log warnings for stale PID files
        """
        processes: List[Dict[str, Any]] = []
        seen_pids: set[int] = set()

        tag_paths = self._get_known_version_paths()

        # 1) PID file checks (legacy root + per-version)
        pid_candidates: List[tuple[Optional[str], Path]] = [
            (None, self.comfyui_dir / "comfyui.pid")
        ]
        pid_candidates.extend([(tag, path / "comfyui.pid") for tag, path in tag_paths.items()])

        for tag, pid_file in pid_candidates:
            if not pid_file.exists():
                continue
            try:
                pid = int(pid_file.read_text().strip())
                os.kill(pid, 0)
                if pid not in seen_pids:
                    processes.append(
                        {
                            "pid": pid,
                            "source": "pid_file",
                            "tag": tag,
                            "pid_file": str(pid_file),
                        }
                    )
                    seen_pids.add(pid)
            except ValueError as exc:
                logger.debug("Invalid PID in %s: %s", pid_file, exc)
                continue
            except ProcessLookupError as exc:
                logger.debug("Stale PID file %s: %s", pid_file, exc)
                continue
            except OSError as exc:
                logger.debug("Failed to read PID file %s: %s", pid_file, exc)
                continue

        # 2) Process table scan (helps when PID files are missing/stale)
        try:
            ps = subprocess.run(
                ["ps", "-eo", "pid=,args="], capture_output=True, text=True, timeout=3
            )
            ps_output = ps.stdout.splitlines()
        except subprocess.SubprocessError as e:
            logger.error(f"Error scanning process table: {e}", exc_info=True)
            ps_output = []
        except OSError as e:
            logger.error(f"Error scanning process table: {e}", exc_info=True)
            ps_output = []

        for line in ps_output:
            line = line.strip()
            if not line:
                continue

            parts = line.split(None, 1)
            if len(parts) != 2:
                continue

            pid_str, cmdline = parts
            try:
                pid = int(pid_str)
            except ValueError as exc:
                logger.debug("Invalid PID from process scan %r: %s", pid_str, exc)
                continue

            if pid in seen_pids:
                continue

            lower_cmd = cmdline.lower()
            has_title = "comfyui server" in lower_cmd
            has_main = "main.py" in cmdline and ("comfyui" in lower_cmd)

            if not (has_title or has_main):
                continue

            inferred_tag = None
            for tag, path in tag_paths.items():
                if str(path) in cmdline:
                    inferred_tag = tag
                    break

            processes.append(
                {
                    "pid": pid,
                    "source": "process_scan",
                    "tag": inferred_tag,
                    "cmd": cmdline,
                }
            )
            seen_pids.add(pid)

        return processes

    def get_processes_with_resources(self) -> List[Dict[str, Any]]:
        """
        Get running ComfyUI processes with CPU, RAM, and GPU resource usage.

        Returns:
            List of dictionaries containing process information with resources:
                - pid (int): Process ID
                - source (str): Detection method ('pid_file' or 'process_scan')
                - tag (str): Version tag if identified
                - cmdline (str): Command line arguments (if available)
                - cpu_usage (float): CPU usage percentage (0-100+)
                - ram_memory (float): RAM memory usage in GB
                - gpu_memory (float): GPU memory usage in GB
        """
        processes = self._detect_comfyui_processes()

        # Enrich each process with resource usage
        for proc in processes:
            pid = proc.get("pid")
            if isinstance(pid, int):
                try:
                    resources = self.resource_tracker.get_process_resources(
                        pid, include_children=True
                    )
                    proc["cpu_usage"] = resources.get("cpu", 0.0)
                    proc["ram_memory"] = resources.get("ram_memory", 0.0)
                    proc["gpu_memory"] = resources.get("gpu_memory", 0.0)
                except OSError as e:
                    logger.debug(f"Failed to get resources for PID {pid}: {e}")
                    proc["cpu_usage"] = 0.0
                    proc["ram_memory"] = 0.0
                    proc["gpu_memory"] = 0.0
                except RuntimeError as e:
                    logger.debug(f"Failed to get resources for PID {pid}: {e}")
                    proc["cpu_usage"] = 0.0
                    proc["ram_memory"] = 0.0
                    proc["gpu_memory"] = 0.0
                except TypeError as e:
                    logger.debug(f"Failed to get resources for PID {pid}: {e}")
                    proc["cpu_usage"] = 0.0
                    proc["ram_memory"] = 0.0
                    proc["gpu_memory"] = 0.0
                except ValueError as e:
                    logger.debug(f"Failed to get resources for PID {pid}: {e}")
                    proc["cpu_usage"] = 0.0
                    proc["ram_memory"] = 0.0
                    proc["gpu_memory"] = 0.0
            else:
                proc["cpu_usage"] = 0.0
                proc["ram_memory"] = 0.0
                proc["gpu_memory"] = 0.0

        return processes

    def is_comfyui_running(self) -> bool:
        """Check if ComfyUI is currently running"""
        try:
            return bool(self._detect_comfyui_processes())
        except OSError as exc:
            logger.debug("Failed to detect processes: %s", exc)
            return False
        except RuntimeError as exc:
            logger.debug("Failed to detect processes: %s", exc)
            return False
        except TypeError as exc:
            logger.debug("Failed to detect processes: %s", exc)
            return False
        except ValueError as exc:
            logger.debug("Failed to detect processes: %s", exc)
            return False
        except subprocess.SubprocessError as exc:
            logger.debug("Failed to detect processes: %s", exc)
            return False

    def stop_comfyui(self) -> bool:
        """Stop running ComfyUI instance"""
        try:
            # First, kill the Brave browser process running ComfyUI
            try:
                # Find and kill Brave processes with ComfyUI in the command line
                result = subprocess.run(
                    ["pgrep", "-f", "brave.*--app=http://127.0.0.1"],
                    capture_output=True,
                    text=True,
                    timeout=5,
                )

                if result.returncode == 0 and result.stdout.strip():
                    # Kill each Brave process found
                    pids = result.stdout.strip().split("\n")
                    for pid_str in pids:
                        try:
                            os.kill(int(pid_str), 9)  # SIGKILL - force kill immediately
                        except ValueError as exc:
                            logger.debug("Invalid PID in pgrep output %s: %s", pid_str, exc)
                        except ProcessLookupError as exc:
                            logger.debug("Brave process already exited %s: %s", pid_str, exc)
            except subprocess.SubprocessError as exc:
                logger.debug("Failed to detect Brave process: %s", exc)
            except OSError as exc:
                logger.debug("Failed to detect Brave process: %s", exc)

            # Stop the ComfyUI server (all detected processes)
            processes = self._detect_comfyui_processes()
            killed = False

            for proc in processes:
                pid_value = proc.get("pid")
                if not isinstance(pid_value, int):
                    continue
                pid = pid_value
                try:
                    os.kill(pid, 15)  # SIGTERM for graceful shutdown
                    time.sleep(0.5)
                    try:
                        os.kill(pid, 9)  # SIGKILL as fallback
                    except ProcessLookupError as exc:
                        logger.debug("Process already exited %s: %s", pid, exc)
                    killed = True
                except ProcessLookupError as exc:
                    logger.debug("Process already exited %s: %s", pid, exc)
                except OSError as e:
                    logger.error(f"Error stopping PID {pid}: {e}", exc_info=True)
                except TypeError as e:
                    logger.error(f"Error stopping PID {pid}: {e}", exc_info=True)
                except ValueError as e:
                    logger.error(f"Error stopping PID {pid}: {e}", exc_info=True)

                pid_file = proc.get("pid_file")
                if isinstance(pid_file, str) and pid_file:
                    try:
                        Path(pid_file).unlink(missing_ok=True)
                    except OSError as exc:
                        logger.debug("Failed to remove PID file %s: %s", pid_file, exc)
                    except TypeError as exc:
                        logger.debug("Failed to remove PID file %s: %s", pid_file, exc)

            if killed:
                return True

            # Fallback: try process name kill if nothing was found
            try:
                subprocess.run(["pkill", "-9", "-f", "ComfyUI Server"], check=False)
                return True
            except subprocess.SubprocessError as exc:
                logger.debug("pkill failed: %s", exc)
            except OSError as exc:
                logger.debug("pkill failed: %s", exc)

            return False
        except OSError as e:
            logger.error(f"Error stopping ComfyUI: {e}", exc_info=True)
            return False
        except RuntimeError as e:
            logger.error(f"Error stopping ComfyUI: {e}", exc_info=True)
            return False
        except TypeError as e:
            logger.error(f"Error stopping ComfyUI: {e}", exc_info=True)
            return False
        except ValueError as e:
            logger.error(f"Error stopping ComfyUI: {e}", exc_info=True)
            return False
        except subprocess.SubprocessError as e:
            logger.error(f"Error stopping ComfyUI: {e}", exc_info=True)
            return False

    def launch_comfyui(self) -> Dict[str, Any]:
        """Launch the active ComfyUI version with readiness detection."""
        try:
            if self.version_manager:
                active_tag = self.version_manager.get_active_version()
                if active_tag:
                    success, _process, log_path, error_msg, ready = (
                        self.version_manager.launch_version(active_tag)
                    )
                    if success:
                        logger.info(f"Launched active managed version: {active_tag}")
                        self.last_launch_log = log_path
                        self.last_launch_error = None
                        return {"success": True, "log_path": log_path, "ready": ready}

                    logger.error(f"Failed to launch managed version {active_tag}")
                    self.last_launch_log = log_path
                    self.last_launch_error = error_msg
                    return {
                        "success": False,
                        "log_path": log_path,
                        "error": error_msg,
                        "ready": ready,
                    }

            self.last_launch_error = "No active version selected"
            self.last_launch_log = None
            return {"success": False, "error": "No active version selected"}
        except OSError as e:
            logger.error(f"Error launching ComfyUI: {e}", exc_info=True)
            self.last_launch_error = str(e)
            return {"success": False, "error": str(e)}
        except RuntimeError as e:
            logger.error(f"Error launching ComfyUI: {e}", exc_info=True)
            self.last_launch_error = str(e)
            return {"success": False, "error": str(e)}
        except TypeError as e:
            logger.error(f"Error launching ComfyUI: {e}", exc_info=True)
            self.last_launch_error = str(e)
            return {"success": False, "error": str(e)}
        except ValueError as e:
            logger.error(f"Error launching ComfyUI: {e}", exc_info=True)
            self.last_launch_error = str(e)
            return {"success": False, "error": str(e)}
