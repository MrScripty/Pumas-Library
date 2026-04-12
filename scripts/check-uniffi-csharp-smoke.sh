#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v uniffi-bindgen-cs >/dev/null 2>&1; then
  echo "Missing required generator: uniffi-bindgen-cs" >&2
  echo "Install a UniFFI 0.28-compatible C# generator, for example uniffi-bindgen-cs 0.9.x." >&2
  exit 1
fi

if ! command -v dotnet >/dev/null 2>&1; then
  echo "Missing required .NET SDK: dotnet" >&2
  exit 1
fi

cargo build --manifest-path rust/Cargo.toml -p pumas-uniffi

case "$(uname -s)" in
  Darwin)
    library_path="rust/target/debug/libpumas_uniffi.dylib"
    loader_env_var="DYLD_LIBRARY_PATH"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    library_path="rust/target/debug/pumas_uniffi.dll"
    loader_env_var="PATH"
    ;;
  *)
    library_path="rust/target/debug/libpumas_uniffi.so"
    loader_env_var="LD_LIBRARY_PATH"
    ;;
esac

if [[ ! -f "$library_path" ]]; then
  echo "Expected UniFFI library at '$library_path'" >&2
  exit 1
fi

generated_dir="rust/target/uniffi/csharp"
generated_binding="$generated_dir/pumas_uniffi.cs"
generated_support="$generated_dir/pumas_library.cs"
mkdir -p "$generated_dir"
rm -f "$generated_binding" "$generated_support"

(
  cd rust/crates/pumas-uniffi
  uniffi-bindgen-cs \
    --library \
    --out-dir "$repo_root/$generated_dir" \
    "$repo_root/$library_path"
)

if [[ ! -f "$generated_binding" ]]; then
  echo "Expected generated C# binding at '$generated_binding'" >&2
  exit 1
fi

require_generated_text() {
  local needle="$1"
  if ! grep -Fq "$needle" "$generated_binding"; then
    echo "Generated C# binding is missing expected text: $needle" >&2
    echo "Generated binding: $generated_binding" >&2
    exit 1
  fi
}

require_generated_text 'namespace uniffi.pumas_uniffi;'
require_generated_text 'public class FfiPumasApi'
require_generated_text 'public record FfiApiConfig'
require_generated_text 'public record FfiDownloadRequest'
require_generated_text 'Task<List<FfiModelRecord>> ListModels()'
require_generated_text 'Task<String> StartHfDownload(FfiDownloadRequest @request)'
require_generated_text 'public static String Version()'

dotnet_root="$(dirname "$(readlink -f "$(command -v dotnet)")")"
sdk_dir="$dotnet_root/sdk"
sdk_version="$(dotnet --version)"
csc_path="$sdk_dir/$sdk_version/Roslyn/bincore/csc.dll"
ref_dir="$(
  find "$dotnet_root/packs/Microsoft.NETCore.App.Ref" \
    -path '*/ref/net*' \
    -type d 2>/dev/null \
  | sort -V \
  | tail -n 1
)"

if [[ ! -f "$csc_path" ]]; then
  echo "Expected Roslyn compiler at '$csc_path'" >&2
  exit 1
fi

if [[ -z "$ref_dir" || ! -d "$ref_dir" ]]; then
  echo "Could not find installed .NET reference assemblies below the dotnet installation." >&2
  exit 1
fi

compile_dir="rust/target/csharp-smoke"
runtime_smoke_root="$repo_root/rust/target/csharp-runtime-smoke"
mkdir -p "$compile_dir"
rm -rf "$runtime_smoke_root"
mkdir -p "$runtime_smoke_root"

references=()
for reference in "$ref_dir"/*.dll; do
  references+=("-r:$reference")
done

generated_sources=()
while IFS= read -r generated_file; do
  generated_sources+=("$generated_file")
done < <(find "$generated_dir" -maxdepth 1 -name '*.cs' | sort)

dotnet "$csc_path" \
  -noconfig \
  -unsafe \
  -nullable:enable \
  -langversion:latest \
  -target:exe \
  -out:"$compile_dir/Pumas.NativeSmoke.dll" \
  "${references[@]}" \
  "${generated_sources[@]}" \
  bindings/csharp/Pumas.NativeSmoke/Program.cs

runtime_version="$(
  dotnet --list-runtimes \
  | awk '/^Microsoft\.NETCore\.App / {print $2}' \
  | sort -V \
  | tail -n 1
)"

if [[ -z "$runtime_version" ]]; then
  echo "Could not find an installed Microsoft.NETCore.App runtime." >&2
  exit 1
fi

cat > "$compile_dir/Pumas.NativeSmoke.runtimeconfig.json" <<EOF
{
  "runtimeOptions": {
    "tfm": "net${runtime_version%%.*}.0",
    "framework": {
      "name": "Microsoft.NETCore.App",
      "version": "$runtime_version"
    }
  }
}
EOF

case "$loader_env_var" in
  PATH)
    env \
      "PUMAS_CSHARP_SMOKE_ROOT=$runtime_smoke_root" \
      "PATH=$repo_root/rust/target/debug${PATH:+:$PATH}" \
      dotnet "$compile_dir/Pumas.NativeSmoke.dll"
    ;;
  DYLD_LIBRARY_PATH)
    env \
      "PUMAS_CSHARP_SMOKE_ROOT=$runtime_smoke_root" \
      "DYLD_LIBRARY_PATH=$repo_root/rust/target/debug${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" \
      dotnet "$compile_dir/Pumas.NativeSmoke.dll"
    ;;
  *)
    env \
      "PUMAS_CSHARP_SMOKE_ROOT=$runtime_smoke_root" \
      "LD_LIBRARY_PATH=$repo_root/rust/target/debug${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      dotnet "$compile_dir/Pumas.NativeSmoke.dll"
    ;;
esac

echo "Verified generated C# Pumas smoke: $generated_binding"
