"""Launch helpers for VersionManager."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import time
from pathlib import Path
from typing import Optional
from urllib import error as url_error
from urllib import request as url_request

from backend.config import INSTALLATION
from backend.logging_config import get_logger
from backend.retry_utils import calculate_backoff_delay

logger = get_logger(__name__)


class LauncherMixin:
    """Mix-in for preparing and launching ComfyUI versions."""

    def _wait_for_server_ready(
        self, url: str, process: subprocess.Popen, log_file: Path, timeout: int = 90
    ) -> tuple[bool, Optional[str]]:
        """Poll the server URL until ready or process exits."""
        start = time.time()
        last_error = None
        attempt = 0

        while True:
            if process.poll() is not None:
                exit_code = process.returncode
                msg = f"ComfyUI process exited early with code {exit_code}"
                logger.error(msg)
                return False, msg

            try:
                with url_request.urlopen(
                    url, timeout=INSTALLATION.URL_QUICK_CHECK_TIMEOUT_SEC
                ) as resp:
                    if resp.status == 200:
                        return True, None
            except (url_error.URLError, OSError) as exc:
                last_error = str(exc)

            if time.time() - start > timeout:
                return False, last_error or "Timed out waiting for server"

            delay = calculate_backoff_delay(attempt, base_delay=0.5, max_delay=5.0)
            time.sleep(delay)
            attempt += 1

    def _tail_log(self, log_file: Path, lines: int = 20) -> list[str]:
        """Return the last N lines of a log file."""
        if not log_file.exists():
            return []
        try:
            content = log_file.read_text().splitlines()
            return content[-lines:]
        except (IOError, OSError, UnicodeDecodeError):
            return []

    def _open_frontend(self, url: str, slug: str) -> None:
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
                        f"--class=ComfyUI-{slug}",
                    ],
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
            else:
                subprocess.Popen(
                    ["xdg-open", url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
                )
        except (OSError, subprocess.SubprocessError) as exc:
            logger.warning(f"Failed to open frontend: {exc}")

    def _slugify_tag(self, tag: str) -> str:
        """Safe slug for filenames."""
        if not tag:
            return "comfyui"
        safe = "".join(c if c.isalnum() or c in ("-", "_") else "-" for c in tag.strip().lower())
        if safe.startswith("v") and len(safe) > 1:
            safe = safe[1:]
        safe = re.sub(r"-+", "-", safe).strip("-_")
        return safe or "comfyui"

    def _ensure_version_run_script(self, tag: str, version_path: Path) -> Path:
        """Ensure a version-specific run.sh exists that also opens the UI."""
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
        except (IOError, OSError) as exc:
            logger.warning(f"Could not write run.sh for {tag}: {exc}")
        return script_path

    def launch_version(
        self, tag: str, extra_args: Optional[list[str]] = None
    ) -> tuple[bool, Optional[subprocess.Popen], Optional[str], Optional[str], Optional[bool]]:
        """Launch a ComfyUI version with readiness detection."""
        if tag not in self.get_installed_versions():
            logger.error(f"Version {tag} is not installed")
            return (False, None, None, "Version not installed", None)

        if not self.set_active_version(tag):
            logger.error("Failed to activate version")
            return (False, None, None, "Failed to activate version", None)

        dep_status = self.check_dependencies(tag)
        if dep_status["missing"]:
            logger.warning(f"Missing dependencies detected for {tag}: {len(dep_status['missing'])}")
            logger.info("Attempting to install missing dependencies before launch...")
            if not self.install_dependencies(tag):
                logger.error("Failed to install dependencies, aborting launch.")
                return (False, None, None, "Dependencies missing", None)
            dep_status = self.check_dependencies(tag)
            if dep_status["missing"]:
                logger.error(f"Dependencies still missing after install: {dep_status['missing']}")
                return (False, None, None, "Dependencies still missing after install", None)

        repair_report = self.resource_manager.validate_and_repair_symlinks(tag)
        if repair_report["broken"]:
            logger.warning(f"Repaired {len(repair_report['repaired'])} broken symlinks")

        version_path = self.versions_dir / tag
        main_py = version_path / "main.py"

        if not main_py.exists():
            logger.error(f"main.py not found in {tag}")
            return (False, None, None, "main.py missing", None)

        venv_python = version_path / "venv" / "bin" / "python"

        if not venv_python.exists():
            logger.error(f"Virtual environment not found for {tag}")
            return (False, None, None, "Virtual environment missing", None)

        run_script = self._ensure_version_run_script(tag, version_path)
        slug = self._slugify_tag(tag)
        url = "http://127.0.0.1:8188"

        log_file = self.logs_dir / f"launch-{slug}-{int(time.time())}.log"
        log_handle = None
        try:
            log_handle = open(log_file, "a", encoding="utf-8")
        except (IOError, OSError) as exc:
            logger.warning(f"Could not open log file {log_file} for {tag}: {exc}")

        cmd = ["bash", str(run_script)]
        if extra_args:
            cmd.extend(extra_args)

        logger.info(f"Launching ComfyUI {tag}...")
        logger.debug(f"Command: {' '.join(cmd)}")

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
                env=env,
            )

            ready, ready_error = self._wait_for_server_ready(url, process, log_file)

            if ready:
                logger.info(f"✓ ComfyUI {tag} reported ready (PID: {process.pid})")
                self._open_frontend(url, slug)
                return (True, process, str(log_file), None, True)

            tail = self._tail_log(log_file)
            if tail:
                logger.warning("Launch log tail:")
                for line in tail:
                    logger.warning(line)
            return (
                False,
                process if process and process.poll() is None else None,
                str(log_file),
                ready_error,
                False,
            )

        except (subprocess.SubprocessError, OSError) as exc:
            logger.error(f"Error launching ComfyUI: {exc}", exc_info=True)
            return (False, None, str(log_file), str(exc), None)
        finally:
            if log_handle:
                try:
                    log_handle.close()
                except (IOError, OSError):
                    pass
