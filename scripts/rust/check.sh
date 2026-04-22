#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)
MANIFEST_PATH="${REPO_ROOT}/rust/Cargo.toml"

workspace_args=(--manifest-path "${MANIFEST_PATH}" --workspace --exclude pumas_rustler)

usage() {
  cat <<'USAGE'
Usage: scripts/rust/check.sh [all|fmt|check|clippy|test|doc|no-default]

Runs standards-aligned Rust workspace verification. The default `all` mode
excludes `pumas_rustler` because the Rustler NIF requires BEAM runtime tooling.
USAGE
}

run_fmt() {
  cargo fmt --manifest-path "${MANIFEST_PATH}" --all -- --check
}

run_check() {
  cargo check "${workspace_args[@]}" --all-targets --all-features
}

run_clippy() {
  cargo clippy "${workspace_args[@]}" --all-targets --all-features -- -D warnings
}

run_test() {
  cargo test "${workspace_args[@]}"
}

run_doc() {
  cargo test "${workspace_args[@]}" --doc
}

run_no_default() {
  cargo check "${workspace_args[@]}" --no-default-features
}

run_all() {
  run_fmt
  run_check
  run_clippy
  run_test
  run_doc
  run_no_default
}

command=${1:-all}

case "${command}" in
  all)
    run_all
    ;;
  fmt)
    run_fmt
    ;;
  check)
    run_check
    ;;
  clippy)
    run_clippy
    ;;
  test)
    run_test
    ;;
  doc)
    run_doc
    ;;
  no-default)
    run_no_default
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
