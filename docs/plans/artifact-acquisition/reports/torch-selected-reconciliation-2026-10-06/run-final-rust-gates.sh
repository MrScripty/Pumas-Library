#!/usr/bin/env bash
set -u
cd /workspace/Pumas-Library
source /workspace/.pumas-tools/env.sh
export ORT_SKIP_DOWNLOAD=1 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=/workspace/pumas-reconcile-target
export XDG_CONFIG_HOME=/workspace/pumas-reconcile-test-config
export PUMAS_QUALIFIED_UV=/workspace/pumas-reconcile-tools/bin/uv
export PYTHONPATH=/workspace/pumas-reconcile-python-full:/workspace/pumas-reconcile-tools:torch-server/tests
export PUMAS_SELECTED_LIFECYCLE_EVIDENCE=/workspace/pumas-reconcile-evidence/publication
results=/workspace/pumas-reconcile-evidence
source_commit=$(git rev-parse HEAD)
source_tree=$(git rev-parse HEAD^{tree})
printf '%s\n%s\n' "$source_commit" "$source_tree" > "$results/tested-source.txt"
run_gate() {
  gate_name=$1
  shift
  "$@" > "$results/$gate_name.log" 2>&1
  gate_result=$?
  printf '%s %s\n' "$gate_name" "$gate_result" >> "$results/gate-results.txt"
  return 0
}
: > "$results/gate-results.txt"
run_gate graphs python3 scripts/release/check-dependency-features.py --s3
run_gate lifecycle cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support selected_runtime_ -- --include-ignored --test-threads=1 --nocapture
run_gate rust cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support version_manager::installer::torch::torch_ -- --include-ignored --test-threads=1 --skip selected_runtime_ --nocapture
run_gate workspace-test cargo test --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler
run_gate workspace-check cargo check --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --all-targets --all-features
run_gate workspace-clippy cargo clippy --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --all-targets --all-features -- -D warnings
run_gate no-default cargo check --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --no-default-features
run_gate doc cargo test --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --doc
run_gate rustfmt cargo fmt --manifest-path rust/Cargo.toml --all -- --check
run_gate evidence-verification python3 docs/plans/artifact-acquisition/reports/torch-selected-lifecycle-2026-10-06/verify_evidence.py "$results/publication" /workspace/Pumas-Library
git diff --exit-code "$source_commit" -- rust torch-server > "$results/source-unchanged.log" 2>&1
printf 'source-unchanged %s\n' "$?" >> "$results/gate-results.txt"
