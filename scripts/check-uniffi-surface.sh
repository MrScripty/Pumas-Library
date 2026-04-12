#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo build --manifest-path rust/Cargo.toml -p pumas-uniffi

case "$(uname -s)" in
  Darwin)
    library_path="rust/target/debug/libpumas_uniffi.dylib"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    library_path="rust/target/debug/pumas_uniffi.dll"
    ;;
  *)
    library_path="rust/target/debug/libpumas_uniffi.so"
    ;;
esac

if [[ ! -f "$library_path" ]]; then
  echo "Expected UniFFI library at '$library_path'" >&2
  exit 1
fi

repr_dir="rust/target/uniffi"
repr_path="$repr_dir/pumas-uniffi.repr.txt"
mkdir -p "$repr_dir"

cargo run --manifest-path rust/Cargo.toml -p pumas-uniffi --bin pumas-uniffi-bindgen --features cli -- \
  print-repr "$library_path" > "$repr_path"

require_metadata() {
  local needle="$1"
  if ! grep -Fq "$needle" "$repr_path"; then
    echo "UniFFI metadata is missing expected binding item: $needle" >&2
    echo "Metadata dump: $repr_path" >&2
    exit 1
  fi
}

require_metadata 'crate_name: "pumas_uniffi"'
require_metadata 'name: "FfiError"'
require_metadata 'self_name: "FfiPumasApi"'
require_metadata 'name: "FfiApiConfig"'
require_metadata 'name: "FfiDownloadRequest"'
require_metadata 'name: "FfiModelRecord"'
require_metadata 'name: "new"'
require_metadata 'name: "with_config"'
require_metadata 'name: "version"'
require_metadata 'name: "list_models"'
require_metadata 'name: "search_hf_models"'
require_metadata 'name: "start_hf_download"'
require_metadata 'name: "get_disk_space"'
require_metadata 'name: "get_status"'

echo "Verified Pumas UniFFI surface metadata: $repr_path"
