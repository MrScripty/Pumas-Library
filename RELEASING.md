# Releasing Pumas Library

## Prerequisites

- All CI checks pass on `main`
- `cargo test --workspace --exclude pumas_rustler` passes locally (from `rust/`)
- `cargo clippy --workspace --exclude pumas_rustler -- -D warnings` clean
- `cargo audit` shows no high/critical vulnerabilities

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

## Artifacts Produced by CI

| Artifact | Platforms |
|----------|-----------|
| `pumas-rpc` binary | Linux x86_64, Windows x86_64, macOS ARM |
| `libpumas_uniffi` shared library | Linux x86_64, Windows x86_64, macOS ARM |
| Electron desktop app | Linux (AppImage, .deb), Windows (.exe), macOS (.dmg) |
| `checksums-sha256.txt` | All |

**Note:** `pumas-rustler` (Erlang NIF) is excluded from CI builds and default
workspace validation because it requires the BEAM runtime to link. Validate it
separately on machines with Erlang/OTP installed.

## Version Locations

| File | Field |
|------|-------|
| `rust/Cargo.toml` | `[workspace.package] version` |
| `frontend/package.json` | `version` |
| `electron/package.json` | `version` |

All three must be updated together before tagging.
