#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_NAME="$(basename "$0")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

APP_BIN="pumas-rpc"
RUST_MANIFEST_PATH="$SCRIPT_DIR/rust/Cargo.toml"
RUST_DEBUG_BIN_PATH="$SCRIPT_DIR/rust/target/debug/$APP_BIN"
RUST_RELEASE_BIN_PATH="$SCRIPT_DIR/rust/target/release/$APP_BIN"
FRONTEND_DIST_INDEX="$SCRIPT_DIR/frontend/dist/index.html"
ELECTRON_DIST_MAIN="$SCRIPT_DIR/electron/dist/main.js"
ELECTRON_EXECUTABLE="$SCRIPT_DIR/electron/node_modules/.bin/electron"

DEPENDENCIES=(
  "cargo"
  "node"
  "npm"
  "frontend_node_modules"
  "electron_node_modules"
)

usage() {
  cat <<EOF_USAGE
Pumas Library launcher.

Usage:
  ./${SCRIPT_NAME} --help
  ./${SCRIPT_NAME} --install
  ./${SCRIPT_NAME} --build
  ./${SCRIPT_NAME} --build-release
  ./${SCRIPT_NAME} --run [-- <app args...>]
  ./${SCRIPT_NAME} --run-release [-- <app args...>]

Examples:
  ./${SCRIPT_NAME} --install
  ./${SCRIPT_NAME} --build
  ./${SCRIPT_NAME} --build-release
  ./${SCRIPT_NAME} --run -- --devtools
  ./${SCRIPT_NAME} --run-release -- --debug

Exit codes:
  0 success
  1 operation failed
  2 usage error
  3 missing dependency for runtime
  4 missing release artifact
EOF_USAGE
}

log() {
  printf '[launcher] %s\n' "$*"
}

die() {
  log "error: $*"
  exit 1
}

die_usage() {
  log "usage error: $*"
  usage
  exit 2
}

check_cargo() {
  command -v cargo >/dev/null 2>&1
}

install_cargo() {
  log "[error] cargo missing; install Rust toolchain from https://rustup.rs"
  return 1
}

check_node() {
  command -v node >/dev/null 2>&1
}

install_node() {
  log "[error] node missing; install Node.js from https://nodejs.org/"
  return 1
}

check_npm() {
  command -v npm >/dev/null 2>&1
}

install_npm() {
  log "[error] npm missing; install npm with your Node.js installation"
  return 1
}

check_frontend_node_modules() {
  [[ -d "$SCRIPT_DIR/frontend/node_modules" ]]
}

install_frontend_node_modules() {
  (
    cd "$SCRIPT_DIR"
    npm install --workspace frontend
  )
}

check_electron_node_modules() {
  [[ -d "$SCRIPT_DIR/electron/node_modules" ]]
}

install_electron_node_modules() {
  (
    cd "$SCRIPT_DIR"
    npm install --workspace electron
  )
}

check_dep() {
  "check_$1"
}

install_dep() {
  "install_$1"
}

install_dependencies() {
  local dep
  for dep in "${DEPENDENCIES[@]}"; do
    if check_dep "$dep"; then
      log "[ok] $dep already satisfied"
      continue
    fi

    log "[install] $dep missing; installing"
    if ! install_dep "$dep"; then
      log "[error] $dep install failed"
      exit 1
    fi

    if check_dep "$dep"; then
      log "[done] $dep installed"
    else
      log "[error] $dep install failed verification"
      exit 1
    fi
  done
}

ensure_runtime_dependencies() {
  local dep
  for dep in "${DEPENDENCIES[@]}"; do
    if ! check_dep "$dep"; then
      log "missing dependency: $dep"
      log "run ./${SCRIPT_NAME} --install first"
      exit 3
    fi
  done
}

build_app() {
  local mode="$1"

  ensure_runtime_dependencies

  case "$mode" in
    dev)
      log "[build] compiling debug backend binary: $APP_BIN"
      cargo build --manifest-path "$RUST_MANIFEST_PATH" -p pumas-rpc --bin "$APP_BIN"
      ;;
    release)
      log "[build] compiling release backend binary: $APP_BIN"
      cargo build --manifest-path "$RUST_MANIFEST_PATH" -p pumas-rpc --release --bin "$APP_BIN"
      ;;
    *)
      die_usage "invalid build mode: $mode"
      ;;
  esac

  log "[build] compiling frontend assets"
  (
    cd "$SCRIPT_DIR"
    npm --workspace frontend run build
  )

  log "[build] compiling electron main process"
  (
    cd "$SCRIPT_DIR"
    npm --workspace electron run build
  )

  log "[done] build completed ($mode)"
}

ensure_dev_runtime_artifacts() {
  if [[ ! -x "$RUST_RELEASE_BIN_PATH" ]]; then
    die "missing runtime backend binary: $RUST_RELEASE_BIN_PATH (run ./${SCRIPT_NAME} --build-release first)"
  fi
}

ensure_release_artifacts() {
  if [[ ! -x "$RUST_RELEASE_BIN_PATH" ]]; then
    log "missing release binary: $RUST_RELEASE_BIN_PATH"
    log "run ./${SCRIPT_NAME} --build-release first"
    exit 4
  fi

  if [[ ! -f "$FRONTEND_DIST_INDEX" ]]; then
    log "missing release frontend artifact: $FRONTEND_DIST_INDEX"
    log "run ./${SCRIPT_NAME} --build-release first"
    exit 4
  fi

  if [[ ! -f "$ELECTRON_DIST_MAIN" ]]; then
    log "missing release electron artifact: $ELECTRON_DIST_MAIN"
    log "run ./${SCRIPT_NAME} --build-release first"
    exit 4
  fi
}

run_dev_app() {
  local run_args=("$@")

  ensure_runtime_dependencies
  ensure_dev_runtime_artifacts

  log "[run] launching development runtime"
  exec env PUMAS_RUST_BACKEND=1 npm --workspace electron run dev -- "${run_args[@]}"
}

run_release_app() {
  local run_args=("$@")

  ensure_runtime_dependencies
  ensure_release_artifacts

  if [[ ! -x "$ELECTRON_EXECUTABLE" ]]; then
    log "missing runtime executable: $ELECTRON_EXECUTABLE"
    log "run ./${SCRIPT_NAME} --install first"
    exit 3
  fi

  log "[run] launching release runtime"
  exec env PUMAS_RUST_BACKEND=1 "$ELECTRON_EXECUTABLE" "$SCRIPT_DIR/electron" "${run_args[@]}"
}

main() {
  local action=""
  local run_args=()

  while (($#)); do
    case "$1" in
      --help)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="help"
        shift
        ;;
      --install)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="install"
        shift
        ;;
      --build)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="build"
        shift
        ;;
      --build-release)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="build-release"
        shift
        ;;
      --run)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="run"
        shift
        ;;
      --run-release)
        [[ -z "$action" ]] || die_usage "only one action flag is allowed"
        action="run-release"
        shift
        ;;
      --)
        [[ "$action" == "run" || "$action" == "run-release" ]] \
          || die_usage "-- is only valid with --run or --run-release"
        shift
        run_args=("$@")
        break
        ;;
      *)
        die_usage "unknown argument: $1"
        ;;
    esac
  done

  [[ -n "$action" ]] || die_usage "one action flag is required"

  case "$action" in
    help)
      usage
      ;;
    install)
      ((${#run_args[@]} == 0)) || die_usage "--install does not accept app args"
      install_dependencies
      ;;
    build)
      ((${#run_args[@]} == 0)) || die_usage "--build does not accept app args"
      build_app "dev"
      ;;
    build-release)
      ((${#run_args[@]} == 0)) || die_usage "--build-release does not accept app args"
      build_app "release"
      ;;
    run)
      run_dev_app "${run_args[@]}"
      ;;
    run-release)
      run_release_app "${run_args[@]}"
      ;;
    *)
      die_usage "invalid action: $action"
      ;;
  esac
}

main "$@"
