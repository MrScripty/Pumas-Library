#!/bin/bash

# =============================================================================
# ComfyUI Launcher Script Template
# =============================================================================
# TEMPLATE FILE: This file is used by version_manager.py to generate
# version-specific ComfyUI run scripts. Do not execute directly.
#
# Generated scripts are placed in: comfyui-versions/{version}/run_{version}.sh
#
# Features:
# - Works when script is inside a subfolder of the ComfyUI root
# - Finds virtual environment automatically (supports common names)
# - Stops previous instances, closes old Brave app windows
# - Starts server and opens isolated Brave app window
# - Ctrl+C for clean shutdown
# =============================================================================

set -euo pipefail

# -----------------------------------------------------------------------------
# Configuration
# -----------------------------------------------------------------------------
# Directory where this script is located (the cloned repo folder)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ComfyUI root directory (one level up from the script's folder)
COMFYUI_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Find the virtual environment folder (common patterns)
VENV_CANDIDATES=(
    "$COMFYUI_DIR/python3.12_venv" # Should remove this, was the venv name on the dev machine
    "$COMFYUI_DIR/venv"
    "$COMFYUI_DIR/.venv"
    "$COMFYUI_DIR/env"
)

VENV_PATH=""
for candidate in "${VENV_CANDIDATES[@]}"; do
    if [[ -d "$candidate" && -f "$candidate/bin/activate" ]]; then
        VENV_PATH="$candidate"
        break
    fi
done

if [[ -z "$VENV_PATH" ]]; then
    echo "[ERROR] Virtual environment not found in $COMFYUI_DIR"
    echo "Looked for: ${VENV_CANDIDATES[*]}"
    exit 1
fi

PID_FILE="$COMFYUI_DIR/comfyui.pid"
URL="http://127.0.0.1:8188"
TEMP_PROFILE_DIR="$(mktemp -d /tmp/comfyui-profile.XXXXXX)"
WINDOW_CLASS="ComfyUI-App"
SERVER_START_DELAY=10  # Increased slightly for reliability

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------

log() {
    echo "[$(date +'%H:%M:%S')] $*"
}

stop_previous_instance() {
    log "Checking for previous ComfyUI server instance..."

    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || echo "")

        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            log "Stopping previous server (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            sleep 3
            if kill -0 "$pid" 2>/dev/null; then
                log "Force killing stubborn process..."
                kill -9 "$pid" 2>/dev/null || true
            fi
            log "Previous server stopped."
        else
            log "Stale PID file found — cleaning up."
        fi
        rm -f "$PID_FILE"
    else
        log "No previous instance detected."
    fi
}

close_existing_app_window() {
    if ! command -v wmctrl >/dev/null 2>&1; then
        log "wmctrl not installed — skipping window close."
        return
    fi

    log "Looking for existing ComfyUI Brave app windows (class: $WINDOW_CLASS)..."

    local windows
    windows=$(wmctrl -l -x 2>/dev/null | grep -i "$WINDOW_CLASS" | awk '{print $1}' || true)

    if [[ -z "$windows" ]]; then
        log "No existing ComfyUI app window found."
        return
    fi

    log "Closing existing window(s)..."
    for win_id in $windows; do
        wmctrl -i -c "$win_id" || true
    done
    sleep 2
}

activate_venv() {
    log "Activating virtual environment: $VENV_PATH"
    # shellcheck source=/dev/null
    source "$VENV_PATH/bin/activate" || {
        log "ERROR: Failed to activate virtual environment at $VENV_PATH"
        exit 1
    }
}

open_comfyui_app() {
    if ! command -v brave-browser >/dev/null 2>&1; then
        log "brave-browser not found. Opening in default browser instead."
        xdg-open "$URL" >/dev/null 2>&1 &
        return
    fi

    log "Launching dedicated Brave app window..."
    brave-browser \
        --app="$URL" \
        --new-window \
        --user-data-dir="$TEMP_PROFILE_DIR" \
        --class="$WINDOW_CLASS" \
        >/dev/null 2>&1 &

    log "Brave app window launched (isolated profile: $TEMP_PROFILE_DIR)"
}

start_comfyui() {
    if [[ ! -f "$COMFYUI_DIR/main.py" ]]; then
        log "ERROR: main.py not found in $COMFYUI_DIR"
        exit 1
    fi

    log "Starting ComfyUI server..."
    python3 main.py --enable-manager &

    local pid=$!
    echo "$pid" > "$PID_FILE"
    log "ComfyUI server started (PID: $pid)"
}

# -----------------------------------------------------------------------------
# Main Execution
# -----------------------------------------------------------------------------

log "ComfyUI root detected: $COMFYUI_DIR"
log "Virtual environment: $VENV_PATH"
log "Script location: $SCRIPT_DIR"

cd "$COMFYUI_DIR"

stop_previous_instance
close_existing_app_window
activate_venv
start_comfyui

log "Waiting $SERVER_START_DELAY seconds for server to start..."
sleep "$SERVER_START_DELAY"

open_comfyui_app

log "ComfyUI is now running!"
log "   → UI: $URL"
log "   → Press Ctrl+C to stop the server."

# Keep script alive and allow clean shutdown on Ctrl+C
trap 'log "Shutting down..."; stop_previous_instance; deactivate || true; rm -rf "$TEMP_PROFILE_DIR"; exit 0' INT TERM
wait $!
