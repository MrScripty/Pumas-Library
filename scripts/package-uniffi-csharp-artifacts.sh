#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v uniffi-bindgen-cs >/dev/null 2>&1; then
  echo "Missing required generator: uniffi-bindgen-cs" >&2
  echo "Install a UniFFI 0.28-compatible C# generator, for example uniffi-bindgen-cs 0.9.x." >&2
  exit 1
fi

if ! command -v zip >/dev/null 2>&1; then
  echo "Missing required archiver: zip" >&2
  exit 1
fi

profile="${PUMAS_PACKAGE_PROFILE:-release}"
if [[ "$profile" == "release" ]]; then
  cargo build --manifest-path rust/Cargo.toml -p pumas-uniffi --release
  cargo_profile_dir="rust/target/release"
else
  cargo build --manifest-path rust/Cargo.toml -p pumas-uniffi
  cargo_profile_dir="rust/target/debug"
fi

case "$(uname -s)" in
  Darwin)
    platform="${PUMAS_PACKAGE_PLATFORM:-osx}"
    library_name="libpumas_uniffi.dylib"
    ;;
  MINGW*|MSYS*|CYGWIN*)
    platform="${PUMAS_PACKAGE_PLATFORM:-win-x64}"
    library_name="pumas_uniffi.dll"
    ;;
  *)
    platform="${PUMAS_PACKAGE_PLATFORM:-linux-x64}"
    library_name="libpumas_uniffi.so"
    ;;
esac

library_path="$cargo_profile_dir/$library_name"
if [[ ! -f "$library_path" ]]; then
  echo "Expected Pumas native library at '$library_path'" >&2
  exit 1
fi

package_root="rust/target/bindings-package"
artifact_dir="$package_root/artifacts"
generated_dir="$package_root/generated/csharp"
csharp_package="$package_root/pumas-csharp-bindings"
native_package="$package_root/pumas-library-native-$platform"

rm -rf "$package_root"
mkdir -p "$artifact_dir" "$generated_dir"

(
  cd rust/crates/pumas-uniffi
  uniffi-bindgen-cs \
    --library \
    --out-dir "$repo_root/$generated_dir" \
    "$repo_root/$library_path"
)

generated_primary="$generated_dir/pumas_uniffi.cs"
generated_support="$generated_dir/pumas_library.cs"
if [[ ! -f "$generated_primary" || ! -f "$generated_support" ]]; then
  echo "Expected generated C# bindings at '$generated_dir'" >&2
  exit 1
fi

write_csharp_manifest() {
  local destination="$1"
  cat > "$destination/manifest.json" <<EOF
{
  "package": "pumas-csharp-bindings",
  "native_module": "pumas_uniffi",
  "required_native_library": "$library_name",
  "platform": "$platform",
  "cargo_profile": "$profile",
  "generated_csharp": [
    "bindings/csharp/pumas_uniffi.cs",
    "bindings/csharp/pumas_library.cs"
  ],
  "docs": "docs/native-bindings.md",
  "native_package": "pumas-library-native-$platform.zip"
}
EOF
}

write_native_manifest() {
  local destination="$1"
  cat > "$destination/manifest.json" <<EOF
{
  "package": "pumas-library-native",
  "native_module": "pumas_uniffi",
  "native_library": "$library_name",
  "platform": "$platform",
  "cargo_profile": "$profile",
  "docs": "docs/native-bindings.md"
}
EOF
}

mkdir -p "$csharp_package/bindings/csharp" "$csharp_package/docs"
cp "$generated_primary" "$csharp_package/bindings/csharp/pumas_uniffi.cs"
cp "$generated_support" "$csharp_package/bindings/csharp/pumas_library.cs"
cp docs/native-bindings.md "$csharp_package/docs/native-bindings.md"
cp bindings/csharp/PACKAGE-README.md "$csharp_package/README.md"
write_csharp_manifest "$csharp_package"

mkdir -p "$native_package/native/$platform" "$native_package/docs"
cp "$library_path" "$native_package/native/$platform/$library_name"
cp docs/native-bindings.md "$native_package/docs/native-bindings.md"
cp docs/native-bindings.md "$native_package/README.md"
write_native_manifest "$native_package"

(
  cd "$package_root"
  zip -qr "artifacts/pumas-csharp-bindings.zip" "pumas-csharp-bindings"
  zip -qr "artifacts/pumas-library-native-$platform.zip" "pumas-library-native-$platform"
)

(
  cd "$artifact_dir"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum \
      "pumas-csharp-bindings.zip" \
      "pumas-library-native-$platform.zip" \
      > checksums-sha256.txt
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 \
      "pumas-csharp-bindings.zip" \
      "pumas-library-native-$platform.zip" \
      > checksums-sha256.txt
  else
    echo "Missing required checksum tool: sha256sum or shasum" >&2
    exit 1
  fi
)

echo "Packaged C# bindings: $artifact_dir/pumas-csharp-bindings.zip"
echo "Packaged native Pumas library: $artifact_dir/pumas-library-native-$platform.zip"
echo "Packaged checksums: $artifact_dir/checksums-sha256.txt"
