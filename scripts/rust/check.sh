#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)
MANIFEST_PATH="${REPO_ROOT}/rust/Cargo.toml"

workspace_args=(--manifest-path "${MANIFEST_PATH}" --workspace --exclude pumas_rustler)

usage() {
  cat <<'USAGE'
Usage: scripts/rust/check.sh [all|fmt|check|clippy|test|doc|no-default|test-isolation|blocking-audit]

Runs standards-aligned Rust workspace verification. The default `all` mode
excludes `pumas_rustler` because the Rustler NIF requires BEAM runtime tooling.

Set PUMAS_RUST_TEST_ISOLATION_REPEATS=N to control test-isolation repeats.
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

run_test_isolation() {
  local repeats="${PUMAS_RUST_TEST_ISOLATION_REPEATS:-3}"
  local guarded_lib_tests=(
    "tests::test_api_creation"
    "tests::test_api_paths"
    "tests::test_get_status"
    "tests::test_get_disk_space"
    "tests::test_new_returns_client_for_existing_primary"
    "tests::test_start_ipc_server_is_idempotent"
    "tests::test_discover_returns_working_client_for_basic_ipc_methods"
    "tests::test_get_library_status_reconciles_stale_library_state_on_first_read"
    "tests::test_generate_migration_dry_run_reconciles_before_reporting"
  )

  if ! [[ "${repeats}" =~ ^[1-9][0-9]*$ ]]; then
    echo "PUMAS_RUST_TEST_ISOLATION_REPEATS must be a positive integer" >&2
    exit 2
  fi

  for ((i = 1; i <= repeats; i++)); do
    echo "Running pumas-library isolation check ${i}/${repeats}"
    for test_filter in "${guarded_lib_tests[@]}"; do
      cargo test -p pumas-library --manifest-path "${MANIFEST_PATH}" --lib "${test_filter}" -- --test-threads=4
    done
    cargo test -p pumas-library --manifest-path "${MANIFEST_PATH}" --test api_tests -- --test-threads=4
  done
}

run_blocking_audit() {
  local roots=(
    "rust/crates/pumas-core/src"
    "rust/crates/pumas-app-manager/src"
    "rust/crates/pumas-rpc/src"
  )
  local patterns=(
    'std::thread::sleep'
    'std::thread::spawn'
    '\.wait\(\)'
    'std::process::Command'
    'std::fs::'
  )

  echo "Blocking-work audit candidates:"
  echo "  roots: ${roots[*]}"
  echo "  note: classify hits as async request path, sync service path, explicit background worker, or test fixture"

  for pattern in "${patterns[@]}"; do
    echo
    echo "== ${pattern} =="
    (cd "${REPO_ROOT}" && rg -n --sort path "${pattern}" "${roots[@]}") || true
  done
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
  test-isolation)
    run_test_isolation
    ;;
  blocking-audit)
    run_blocking_audit
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
