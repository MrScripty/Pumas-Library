# Releasing Pumas Library

## Prerequisites

- All CI checks pass on `main`
- `npm run test:launcher` passes locally
- `bash launcher.sh --release-smoke` passes locally on a machine that can launch Electron
- `cargo test --workspace --exclude pumas_rustler` passes locally (from `rust/`)
- `cargo clippy --workspace --exclude pumas_rustler -- -D warnings` clean
- `cargo audit` shows no high/critical vulnerabilities
- Local toolchains match the repo pins in `rust-toolchain.toml`, `.node-version`, and `.python-version`

## Steps

1. Update `CHANGELOG.md`: rename `[Unreleased]` to `[X.Y.Z] - YYYY-MM-DD`
2. Add a fresh empty `[Unreleased]` section above the new version heading
3. Update version in `rust/Cargo.toml` (`[workspace.package] version`)
4. Update version in `frontend/package.json` and `electron/package.json`
5. Commit: `git commit -m "chore(release): prepare vX.Y.Z"`
6. Tag: `git tag vX.Y.Z`
7. Push: `git push && git push --tags`
8. CI creates a draft GitHub Release with artifacts
9. Review the draft release — verify all expected artifacts are present
10. Publish the release

## Local Verification

Run the release-facing verification commands from the repo root:

```bash
npm run test:launcher
bash launcher.sh --build-release
bash launcher.sh --release-smoke

cargo test --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler
npm run -w frontend test:run
npm run -w frontend check:types
npm run -w electron validate
```

## Artifacts Produced by CI

Artifact names, platform identifiers, checksum coverage, SBOM requirements, and
native binding compatibility are governed by
`docs/contracts/release-artifacts.md`.

| Artifact | Platforms |
|----------|-----------|
| `pumas-rpc` binary | Linux x86_64, Windows x86_64, macOS ARM |
| `libpumas_uniffi` shared library | Linux x86_64, Windows x86_64, macOS ARM |
| Electron desktop app | Linux (AppImage, .deb), Windows (.exe), macOS (.dmg) |
| `checksums-sha256.txt` | All |

**Note:** `pumas-rustler` (Erlang NIF) is excluded from CI builds and default
workspace validation because it requires the BEAM runtime to link. Validate it
separately on machines with Erlang/OTP installed.

## Native Binding Packaging

For local release prep or future CI wiring, package the C# and native binding
artifacts with:

```bash
./scripts/check-uniffi-surface.sh
./scripts/check-uniffi-csharp-smoke.sh
./scripts/package-uniffi-csharp-artifacts.sh
```

The packaging script writes:

- `rust/target/bindings-package/artifacts/pumas-csharp-bindings.zip`
- `rust/target/bindings-package/artifacts/pumas-library-native-<platform>.zip`
- `rust/target/bindings-package/artifacts/checksums-sha256.txt`

These are additive release-prep artifacts. If CI is updated to publish them,
keep the generated C# package and native package version-matched from the same
build.

## Version Locations

| File | Field |
|------|-------|
| `rust/Cargo.toml` | `[workspace.package] version` |
| `frontend/package.json` | `version` |
| `electron/package.json` | `version` |

## Toolchain Pins

| File | Purpose |
|------|---------|
| `rust-toolchain.toml` | Rust compiler and component pin used by local builds and CI |
| `.node-version` | Node.js version pin used by local builds and CI |
| `.python-version` | Python version pin for local tooling and Python-side helpers |

All three must be updated together before tagging.
