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
cargo test --offline --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler > /workspace/pumas-reconcile-evidence/workspace-test-xdg.log 2>&1
printf 'workspace-test-xdg %s\n' "$?" >> /workspace/pumas-reconcile-evidence/gate-results.txt
