#!/usr/bin/env python3
"""
Process Manager for ComfyUI
Handles process detection, launching, and stopping
"""

import os
import subprocess
import time
from pathlib import Path
from typing import List, Dict, Any, Optional


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
        except Exception as e:
            print(f"Error collecting version paths: {e}")

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
        pid_candidates.extend([
            (tag, path / "comfyui.pid") for tag, path in tag_paths.items()
        ])

        for tag, pid_file in pid_candidates:
            if not pid_file.exists():
                continue
            try:
                pid = int(pid_file.read_text().strip())
                os.kill(pid, 0)
                if pid not in seen_pids:
                    processes.append({
                        "pid": pid,
                        "source": "pid_file",
                        "tag": tag,
                        "pid_file": str(pid_file)
                    })
                    seen_pids.add(pid)
            except (ValueError, ProcessLookupError, OSError):
                continue

        # 2) Process table scan (helps when PID files are missing/stale)
        try:
            ps = subprocess.run(
                ['ps', '-eo', 'pid=,args='],
                capture_output=True,
                text=True,
                timeout=3
            )
            ps_output = ps.stdout.splitlines()
        except Exception as e:
            print(f"Error scanning process table: {e}")
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
            except ValueError:
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

            processes.append({
                "pid": pid,
                "source": "process_scan",
                "tag": inferred_tag,
                "cmd": cmdline
            })
            seen_pids.add(pid)

        return processes

    def is_comfyui_running(self) -> bool:
        """Check if ComfyUI is currently running"""
        try:
            return bool(self._detect_comfyui_processes())
        except Exception:
            return False

    def stop_comfyui(self) -> bool:
        """Stop running ComfyUI instance"""
        try:
            # First, kill the Brave browser process running ComfyUI
            try:
                # Find and kill Brave processes with ComfyUI in the command line
                result = subprocess.run(
                    ['pgrep', '-f', 'brave.*--app=http://127.0.0.1'],
                    capture_output=True,
                    text=True,
                    timeout=5
                )

                if result.returncode == 0 and result.stdout.strip():
                    # Kill each Brave process found
                    pids = result.stdout.strip().split('\n')
                    for pid in pids:
                        try:
                            os.kill(int(pid), 9)  # SIGKILL - force kill immediately
                        except (ValueError, ProcessLookupError):
                            pass
            except Exception:
                pass  # Continue even if this fails

            # Stop the ComfyUI server (all detected processes)
            processes = self._detect_comfyui_processes()
            killed = False

            for proc in processes:
                pid = proc.get("pid")
                if pid is None:
                    continue
                try:
                    os.kill(pid, 15)  # SIGTERM for graceful shutdown
                    time.sleep(0.5)
                    try:
                        os.kill(pid, 9)  # SIGKILL as fallback
                    except ProcessLookupError:
                        pass
                    killed = True
                except (ProcessLookupError, OSError):
                    pass
                except Exception as e:
                    print(f"Error stopping PID {pid}: {e}")

                pid_file = proc.get("pid_file")
                if pid_file:
                    try:
                        Path(pid_file).unlink(missing_ok=True)
                    except Exception:
                        pass

            if killed:
                return True

            # Fallback: try process name kill if nothing was found
            try:
                subprocess.run(['pkill', '-9', '-f', 'ComfyUI Server'], check=False)
                return True
            except Exception:
                pass

            return False
        except Exception as e:
            print(f"Error stopping ComfyUI: {e}")
            return False

    def launch_comfyui(self) -> Dict[str, Any]:
        """Launch the active ComfyUI version with readiness detection."""
        try:
            if self.version_manager:
                active_tag = self.version_manager.get_active_version()
                if active_tag:
                    success, _process, log_path, error_msg, ready = self.version_manager.launch_version(active_tag)
                    if success:
                        print(f"Launched active managed version: {active_tag}")
                        self.last_launch_log = log_path
                        self.last_launch_error = None
                        return {"success": True, "log_path": log_path, "ready": ready}

                    print(f"Failed to launch managed version {active_tag}")
                    self.last_launch_log = log_path
                    self.last_launch_error = error_msg
                    return {"success": False, "log_path": log_path, "error": error_msg, "ready": ready}

            self.last_launch_error = "No active version selected"
            self.last_launch_log = None
            return {"success": False, "error": "No active version selected"}
        except Exception as e:
            print(f"Error launching ComfyUI: {e}")
            self.last_launch_error = str(e)
            return {"success": False, "error": str(e)}
