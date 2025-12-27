#!/usr/bin/env bash
#
# Developer Runner - Quick launch for development
# Activates venv and runs the application
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VENV_ACTIVATE="$SCRIPT_DIR/venv/bin/activate"

if [ ! -f "$VENV_ACTIVATE" ]; then
    echo "Error: Virtual environment not found"
    echo "Please run: scripts/dev/setup.sh"
    exit 1
fi

echo "Activating virtual environment..."
source "$VENV_ACTIVATE"

echo "Starting Linux ComfyUI Launcher (development mode with debug console)..."
cd "$SCRIPT_DIR"
export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
python "$SCRIPT_DIR/backend/main.py" --dev
