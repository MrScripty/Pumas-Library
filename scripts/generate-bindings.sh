#!/usr/bin/env bash
#
# Generate cross-language bindings for pumas-core.
#
# Usage:
#   ./scripts/generate-bindings.sh [python|csharp|kotlin|swift|ruby|elixir|all]
#
# Prerequisites:
#   Python/Kotlin/Swift/Ruby:  cargo install uniffi-bindgen-cli
#   C#:      cargo install uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs --tag v0.9.0+v0.28.3
#   Elixir:  mix deps.get (Rustler compiles NIFs as part of the Mix build)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_DIR="$PROJECT_ROOT/rust"
BINDINGS_DIR="$PROJECT_ROOT/bindings"
UNIFFI_CRATE="$RUST_DIR/crates/pumas-uniffi"
RUSTLER_CRATE="$RUST_DIR/crates/pumas-rustler"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ---------------------------------------------------------------------------
# Build the UniFFI cdylib (shared library)
# ---------------------------------------------------------------------------
build_uniffi() {
    info "Building pumas-uniffi in release mode..."
    cargo build --manifest-path "$RUST_DIR/Cargo.toml" \
        -p pumas-uniffi --release
    ok "pumas-uniffi built successfully."

    # Locate the built library
    local target_dir="$RUST_DIR/target/release"
    if [[ -f "$target_dir/libpumas_uniffi.so" ]]; then
        CDYLIB_PATH="$target_dir/libpumas_uniffi.so"
    elif [[ -f "$target_dir/libpumas_uniffi.dylib" ]]; then
        CDYLIB_PATH="$target_dir/libpumas_uniffi.dylib"
    elif [[ -f "$target_dir/pumas_uniffi.dll" ]]; then
        CDYLIB_PATH="$target_dir/pumas_uniffi.dll"
    else
        error "Could not find built pumas-uniffi library in $target_dir"
        exit 1
    fi
    info "Library: $CDYLIB_PATH"
}

# ---------------------------------------------------------------------------
# Python bindings (via uniffi-bindgen)
# ---------------------------------------------------------------------------
generate_python() {
    info "Generating Python bindings..."

    if ! command -v uniffi-bindgen &>/dev/null; then
        warn "uniffi-bindgen not found. Installing..."
        cargo install uniffi-bindgen-cli
        if [ $? -ne 0 ]; then
            error "Failed to install uniffi-bindgen-cli"
            return 1
        fi
        ok "uniffi-bindgen installed"
    fi

    local out_dir="$BINDINGS_DIR/python"
    mkdir -p "$out_dir"

    uniffi-bindgen generate \
        --library \
        --language python \
        --out-dir "$out_dir" \
        "$CDYLIB_PATH"

    # Copy the shared library next to the Python module
    cp "$CDYLIB_PATH" "$out_dir/"

    ok "Python bindings generated in $out_dir/"
    info "Usage:"
    info "  import sys; sys.path.insert(0, '$out_dir')"
    info "  from pumas_uniffi import *"
}

# ---------------------------------------------------------------------------
# C# bindings (via uniffi-bindgen-cs)
# ---------------------------------------------------------------------------
generate_csharp() {
    info "Generating C# bindings..."

    if ! command -v uniffi-bindgen-cs &>/dev/null; then
        warn "uniffi-bindgen-cs not found. Installing..."
        CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs --tag v0.9.0+v0.28.3
        if [ $? -ne 0 ]; then
            error "Failed to install uniffi-bindgen-cs"
            return 1
        fi
        ok "uniffi-bindgen-cs installed"
    fi

    local out_dir="$BINDINGS_DIR/csharp"
    mkdir -p "$out_dir"

    uniffi-bindgen-cs \
        --library \
        --out-dir "$out_dir" \
        "$CDYLIB_PATH"

    # Copy the shared library for .NET runtime to load
    cp "$CDYLIB_PATH" "$out_dir/"

    ok "C# bindings generated in $out_dir/"
    info "Add the generated .cs files to your .NET project and"
    info "ensure the native library is in your output directory."
}

# ---------------------------------------------------------------------------
# Kotlin bindings (via uniffi-bindgen)
# ---------------------------------------------------------------------------
generate_kotlin() {
    info "Generating Kotlin bindings..."

    if ! command -v uniffi-bindgen &>/dev/null; then
        warn "uniffi-bindgen not found. Installing..."
        cargo install uniffi-bindgen-cli
        if [ $? -ne 0 ]; then
            error "Failed to install uniffi-bindgen-cli"
            return 1
        fi
        ok "uniffi-bindgen installed"
    fi

    local out_dir="$BINDINGS_DIR/kotlin"
    mkdir -p "$out_dir"

    uniffi-bindgen generate \
        --library \
        --language kotlin \
        --out-dir "$out_dir" \
        "$CDYLIB_PATH"

    # Copy the shared library next to the generated module
    cp "$CDYLIB_PATH" "$out_dir/"

    ok "Kotlin bindings generated in $out_dir/"
    info "Add the generated .kt files to your Kotlin/JVM project and"
    info "ensure the native library is loadable via System.loadLibrary()."
}

# ---------------------------------------------------------------------------
# Swift bindings (via uniffi-bindgen)
# ---------------------------------------------------------------------------
generate_swift() {
    info "Generating Swift bindings..."

    if ! command -v uniffi-bindgen &>/dev/null; then
        warn "uniffi-bindgen not found. Installing..."
        cargo install uniffi-bindgen-cli
        if [ $? -ne 0 ]; then
            error "Failed to install uniffi-bindgen-cli"
            return 1
        fi
        ok "uniffi-bindgen installed"
    fi

    local out_dir="$BINDINGS_DIR/swift"
    mkdir -p "$out_dir"

    uniffi-bindgen generate \
        --library \
        --language swift \
        --out-dir "$out_dir" \
        "$CDYLIB_PATH"

    # Copy the shared library next to the generated module
    cp "$CDYLIB_PATH" "$out_dir/"

    ok "Swift bindings generated in $out_dir/"
    info "Add the generated .swift files and modulemap to your Xcode project."
}

# ---------------------------------------------------------------------------
# Ruby bindings (via uniffi-bindgen)
# ---------------------------------------------------------------------------
generate_ruby() {
    info "Generating Ruby bindings..."

    if ! command -v uniffi-bindgen &>/dev/null; then
        warn "uniffi-bindgen not found. Installing..."
        cargo install uniffi-bindgen-cli
        if [ $? -ne 0 ]; then
            error "Failed to install uniffi-bindgen-cli"
            return 1
        fi
        ok "uniffi-bindgen installed"
    fi

    local out_dir="$BINDINGS_DIR/ruby"
    mkdir -p "$out_dir"

    uniffi-bindgen generate \
        --library \
        --language ruby \
        --out-dir "$out_dir" \
        "$CDYLIB_PATH"

    # Copy the shared library next to the generated module
    cp "$CDYLIB_PATH" "$out_dir/"

    ok "Ruby bindings generated in $out_dir/"
    info "require the generated .rb file and ensure the native library"
    info "is in Ruby's library search path."
}

# ---------------------------------------------------------------------------
# Elixir bindings (Rustler - compiled via Mix)
# ---------------------------------------------------------------------------
generate_elixir() {
    info "Elixir bindings use Rustler and are compiled as part of the Mix build."
    info ""
    info "To use in an Elixir project:"
    info ""
    info "  1. Add to mix.exs deps:"
    info "     {:rustler, \"~> 0.34\"}"
    info ""
    info "  2. Create the NIF module (lib/pumas/native.ex):"
    info "     defmodule Pumas.Native do"
    info "       use Rustler, otp_app: :pumas, crate: \"pumas_rustler\""
    info ""
    info "       # NIFs"
    info "       def version(), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_model_type(_type), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_security_tier(_tier), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_download_status(_status), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_file_type(_type), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_health_status(_status), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_import_stage(_stage), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def parse_sandbox_type(_type), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def validate_json(_json), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def new_model_hashes(_sha256, _blake3), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def new_base_response(_success, _error), do: :erlang.nif_error(:nif_not_loaded)"
    info "       def new_download_option(_quant, _size), do: :erlang.nif_error(:nif_not_loaded)"
    info "     end"
    info ""
    info "  3. Point Rustler to the crate in config/config.exs:"
    info "     config :pumas, Pumas.Native,"
    info "       crate: \"pumas_rustler\","
    info "       path: \"$RUSTLER_CRATE\""
    info ""
    info "  4. Run: mix compile"
    info ""

    # Optionally verify the Rustler crate builds
    info "Verifying pumas_rustler builds..."
    cargo build --manifest-path "$RUST_DIR/Cargo.toml" \
        -p pumas_rustler --release
    ok "pumas_rustler built successfully."
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
usage() {
    echo "Usage: $0 [python|csharp|kotlin|swift|ruby|elixir|all]"
    echo ""
    echo "Generate cross-language bindings for pumas-core."
    echo ""
    echo "Targets:"
    echo "  python   Generate Python bindings via uniffi-bindgen"
    echo "  csharp   Generate C# bindings via uniffi-bindgen-cs"
    echo "  kotlin   Generate Kotlin bindings via uniffi-bindgen"
    echo "  swift    Generate Swift bindings via uniffi-bindgen"
    echo "  ruby     Generate Ruby bindings via uniffi-bindgen"
    echo "  elixir   Build Rustler NIF and print Elixir integration guide"
    echo "  all      Generate all bindings (default)"
    exit 1
}

main() {
    local target="${1:-all}"

    info "Pumas bindings generator"
    info "========================"
    echo ""

    case "$target" in
        python)
            build_uniffi
            generate_python
            ;;
        csharp)
            build_uniffi
            generate_csharp
            ;;
        kotlin)
            build_uniffi
            generate_kotlin
            ;;
        swift)
            build_uniffi
            generate_swift
            ;;
        ruby)
            build_uniffi
            generate_ruby
            ;;
        elixir)
            generate_elixir
            ;;
        all)
            build_uniffi
            generate_python
            generate_csharp
            generate_kotlin
            generate_swift
            generate_ruby
            generate_elixir
            ;;
        -h|--help|help)
            usage
            ;;
        *)
            error "Unknown target: $target"
            usage
            ;;
    esac

    echo ""
    ok "Done!"
}

main "$@"
