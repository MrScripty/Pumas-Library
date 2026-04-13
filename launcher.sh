#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LAUNCHER_CORE="$SCRIPT_DIR/scripts/launcher/cli.mjs"
RELEASE_BINARY="$SCRIPT_DIR/electron/release/linux-unpacked/pumas-library-electron"

if [[ "${1:-}" == "--run-release" ]] && [[ -x "$RELEASE_BINARY" ]]; then
  shift
  if [[ "${1:-}" == "--" ]]; then
    shift
  fi

  export PUMAS_LAUNCHER_DISPLAY_NAME='./launcher.sh'
  exec "$RELEASE_BINARY" "$@"
fi

if ! command -v node >/dev/null 2>&1; then
  printf '[launcher] error: node missing; install Node.js from https://nodejs.org/\n' >&2
  exit 1
fi

export PUMAS_LAUNCHER_DISPLAY_NAME='./launcher.sh'
exec node "$LAUNCHER_CORE" "$@"
