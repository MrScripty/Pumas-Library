#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LAUNCHER_CORE="$SCRIPT_DIR/scripts/launcher/cli.mjs"

if ! command -v node >/dev/null 2>&1; then
  printf '[launcher] error: node missing; install Node.js from https://nodejs.org/\n' >&2
  exit 1
fi

export PUMAS_LAUNCHER_DISPLAY_NAME='./launcher.sh'
exec node "$LAUNCHER_CORE" "$@"
