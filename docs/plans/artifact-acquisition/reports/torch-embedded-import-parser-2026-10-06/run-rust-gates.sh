#!/usr/bin/env bash
set -u
cd /workspace/Pumas-Library
source /workspace/.pumas-tools/env.sh
export ORT_SKIP_DOWNLOAD=1 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=/workspace/pumas-reconcile-target
export XDG_CONFIG_HOME=/workspace/pumas-reconcile-test-config
results=/workspace/pumas-parser-fix-evidence
: > "$results/rust-gate-results.txt"
run_gate() {
  gate_name=$1
  shift
  "$@" > "$results/$gate_name.log" 2>&1
  gate_result=$?
  printf '%s %s\n' "$gate_name" "$gate_result" >> "$results/rust-gate-results.txt"
}
run_gate feature-graphs python -B scripts/release/check-dependency-features.py --s3
run_gate rust-embedded cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support embedded_ -- --nocapture
run_gate workspace-test cargo test --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler
run_gate workspace-clippy cargo clippy --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --all-targets --all-features -- -D warnings
run_gate rustfmt cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cat "$results/rust-gate-results.txt"
