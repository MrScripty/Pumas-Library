#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LAUNCHER_CORE="$SCRIPT_DIR/scripts/launcher/cli.mjs"
RELEASE_BACKEND_BINARY="$SCRIPT_DIR/rust/target/release/pumas-rpc"
RELEASE_FRONTEND_INDEX="$SCRIPT_DIR/frontend/dist/index.html"
RELEASE_ELECTRON_MAIN="$SCRIPT_DIR/electron/dist/main.js"
RELEASE_ELECTRON_BINARY="$SCRIPT_DIR/electron/node_modules/electron/dist/electron"

if [[ "${1:-}" == "--run-release" ]] \
  && [[ -x "$RELEASE_ELECTRON_BINARY" ]] \
  && [[ -x "$RELEASE_BACKEND_BINARY" ]] \
  && [[ -f "$RELEASE_FRONTEND_INDEX" ]] \
  && [[ -f "$RELEASE_ELECTRON_MAIN" ]]; then
  shift
  if [[ "${1:-}" == "--" ]]; then
    shift
  fi

  export PUMAS_LAUNCHER_DISPLAY_NAME='./launcher.sh'
  export PUMAS_RUST_BACKEND='1'
  export PUMAS_RPC_BINARY="$RELEASE_BACKEND_BINARY"

  cd "$SCRIPT_DIR/electron"
  exec "$RELEASE_ELECTRON_BINARY" . "$@"
fi

if ! command -v node >/dev/null 2>&1; then
  printf '[launcher] error: node missing; install Node.js from https://nodejs.org/\n' >&2
  exit 1
fi

export PUMAS_LAUNCHER_DISPLAY_NAME='./launcher.sh'
exec node "$LAUNCHER_CORE" "$@"
