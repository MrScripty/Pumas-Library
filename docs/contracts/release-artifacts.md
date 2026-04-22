# Release Artifact Contract

## Purpose
This contract defines the artifacts, names, integrity files, and compatibility rules that must stay aligned when Pumas Library is released.

## Version Source
The release version is the single SemVer value shared by:

- `rust/Cargo.toml` `[workspace.package] version`
- `frontend/package.json` `version`
- `electron/package.json` `version`

Release artifacts must be built from the same tagged commit and version. Mixed-version desktop, native library, generated binding, or SBOM artifacts are not releaseable.

## Platform Identifiers
Use these platform identifiers in artifact names and manifests:

| Platform | Identifier | Runtime Target |
| --- | --- | --- |
| Linux x64 | `linux-x64` | glibc Linux desktop and native binding packages |
| Windows x64 | `win-x64` | Windows desktop and native binding packages |
| macOS ARM64 | `macos-arm64` | Apple Silicon desktop packages |
| macOS native binding alias | `osx` | Existing UniFFI C# native package script output |

`osx` remains accepted only for the current UniFFI C# packaging script. New release artifacts should use `macos-arm64` unless a tool requires a different runtime identifier.

## Artifact Matrix
| Artifact Family | Required Names | Producer | Consumer |
| --- | --- | --- | --- |
| Electron desktop app | `Pumas Library-<version>.AppImage`, `pumas-library_<version>_amd64.deb`, `Pumas Library Setup <version>.exe`, `Pumas Library <version>.exe`, `Pumas Library-<version>-arm64.dmg` | `electron-builder` through Electron package scripts | End users and release smoke checks |
| Rust RPC binary | `pumas-rpc-<version>-<platform>` or `pumas-rpc-<version>-<platform>.exe` | Cargo release build | Electron packaged `resources/bin` and diagnostics |
| UniFFI native library | `pumas-library-native-<platform>.zip` | `scripts/package-uniffi-csharp-artifacts.sh` | Host-language binding packages |
| Generated C# binding package | `pumas-csharp-bindings.zip` | `scripts/package-uniffi-csharp-artifacts.sh` | C# host applications and smoke harnesses |
| SBOMs | `sbom-frontend-<version>.json`, `sbom-python-<version>.json`, `sbom-rust-<version>.json` | `scripts/dev/generate-sbom.sh` plus Rust SBOM job when added | Release audit and dependency review |
| Checksums | `checksums-sha256.txt` | Release job or packaging script | Human and automated artifact verification |

Existing checked-in SBOM snapshots under `docs/sbom/` may omit the version in their filenames. Release-published SBOMs must include the release version.

## Native Binding Compatibility
Generated host-language bindings and native libraries must come from the same compiled `pumas-uniffi` artifact. A binding package manifest must state:

- generated package name;
- native module identity;
- required native library filename;
- platform identifier;
- cargo profile;
- documentation path;
- matching native package name.

The internal native module identity remains `pumas_uniffi`. Product-facing native packages use `pumas-library-native-<platform>.zip`.

## Checksum Contract
Every release artifact, including desktop installers, native packages, generated binding packages, and release SBOMs, must appear in `checksums-sha256.txt`.

Checksum rows use the standard two-column format:

```text
<sha256>  <artifact-file-name>
```

The checksum file itself is not included in its own digest list.

## SBOM Contract
Release SBOMs must be generated from the same commit as the release artifacts. SBOM producers must fail if dependency manifests and lockfiles are out of sync.

Minimum SBOM coverage:

- frontend and Electron Node dependency graph;
- Python sidecar dependency graph;
- Rust workspace dependency graph.

## Structured Producer Contract
Release automation or local release-prep scripts must write artifacts into a staging directory before publication. The staging directory must contain:

- all artifacts from the artifact matrix for the targeted platforms;
- `checksums-sha256.txt`;
- generated SBOM files;
- release notes derived from `CHANGELOG.md`.

## API Consumer Contract
Downstream consumers may assume artifacts with the same version and platform identifier are mutually compatible. Consumers must not mix a generated binding package with a native package from a different version or platform.

## Non-Goals
Code signing and notarization policy is not defined here. Reason: signing credentials and platform-specific notarization workflows are not present in the repository. Revisit trigger: add signed Windows or macOS release artifacts.

## Revisit Triggers
- Add a new platform target.
- Change Electron package names or `electron-builder` targets.
- Publish Rustler NIF artifacts.
- Replace the C# UniFFI generator or add another host-language package.
- Add automated release CI that changes staging layout.
