#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUIRED_NODE_VERSION="$(cat "$SCRIPT_DIR/.node-version")"
REQUIRED_PYTHON_VERSION="$(cat "$SCRIPT_DIR/.python-version")"
REQUIRED_RUST_VERSION="$(sed -n 's/^channel = \"\\(.*\\)\"$/\\1/p' "$SCRIPT_DIR/rust-toolchain.toml")"

echo "Pumas Library system check"
echo ""

missing_tools=0

check_command() {
    local name="$1"
    if command -v "$name" >/dev/null 2>&1; then
        echo "[ok] $name: $(command -v "$name")"
        return 0
    fi

    echo "[missing] $name"
    missing_tools=1
    return 1
}

check_version_prefix() {
    local label="$1"
    local actual="$2"
    local expected="$3"

    if [[ "$actual" == "$expected"* ]]; then
        echo "[ok] $label version: $actual"
    else
        echo "[warn] $label version: $actual (expected $expected)"
    fi
}

check_command cargo
check_command rustc
check_command node
check_command corepack
check_command python3

if [[ "$missing_tools" -eq 0 ]]; then
    echo ""
    check_version_prefix "rustc" "$(rustc --version | awk '{print $2}')" "$REQUIRED_RUST_VERSION"
    check_version_prefix "node" "$(node --version | sed 's/^v//')" "$REQUIRED_NODE_VERSION"
    check_version_prefix "python3" "$(python3 --version | awk '{print $2}')" "$REQUIRED_PYTHON_VERSION"
fi

echo ""
echo "Recommended verification commands:"
echo "  corepack pnpm install --frozen-lockfile"
echo "  bash ./launcher.sh --install"
echo "  npm run test:launcher"
echo "  cargo test --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler"

if [[ "$missing_tools" -ne 0 ]]; then
    exit 1
fi
