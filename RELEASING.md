# Releasing Pumas Library

## Prepare and rehearse before tagging

The `Build` workflow runs on pull requests, `main`, version tags, and manual
`workflow_dispatch`. Every trigger runs the same release-candidate gates and
installer assembly. Windows is a separate best-effort target for 0.7.0. Rehearse the intended commit before creating a version tag;
a tag is not the first opportunity to discover packaging failures.

1. Update `CHANGELOG.md` and the version's release notes. Keep an empty
   `Unreleased` section for subsequent changes.
2. Set the same version in root, frontend, and Electron `package.json`, and
   `[workspace.package]` in `rust/Cargo.toml`. Update the workspace package
   versions in `rust/Cargo.lock` without resolving new dependencies.
3. Run `npm run check:release-versions -- vX.Y.Z`. CI also compares version tags
   against the manifests automatically.
4. Complete local QA below, then run the workflow against the candidate commit
   once it is available remotely. Inspect every platform's result.
5. Review the `release-candidate` workflow artifact. It contains the three required
   Linux/macOS installers selected by [the artifact plan](scripts/release/artifact-plan.json)
   and checksums over those final installer bytes. A successful `windows-candidate`
   job separately provides both Windows installers and their checksums.

The workflow deliberately assembles **candidates**, with read-only repository
permissions. It does not create or publish a GitHub release. Publication requires
all the release acceptance evidence below; green compilation alone is inadequate.

## Local QA

Use `rust-toolchain.toml`, `.node-version`, the root pnpm pin, and
`.python-version`. Tests require normal subprocess, filesystem, and loopback
socket access; sandbox-denied operations are unavailable evidence, not product
failures.

```bash
pnpm install --frozen-lockfile
npm run check:dependency-ownership
npm run check:release-versions -- v0.7.0
node --test scripts/release/*.test.mjs
./scripts/rust/check.sh
cargo test --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --release
cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --no-default-features
cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features
pnpm --dir frontend lint
pnpm --dir frontend check:types
pnpm --dir frontend test:run
pnpm --dir frontend build:library-only
pnpm --dir frontend build
pnpm --dir electron lint
pnpm --dir electron validate
node electron/node_modules/electron/install.js
pnpm --dir electron test
cargo fetch --locked --manifest-path rust/Cargo.toml
pnpm --dir electron check:desktop-contract
pnpm --dir electron test:desktop-contract-generator
pnpm --dir electron test:desktop-contract-conformance
pnpm --dir frontend test:desktop-contract
npm run test:launcher
python3 -m ruff check torch-server scripts/release
python3 -m ruff format --check torch-server scripts/release
python3 -m unittest discover -s torch-server/tests
./launcher.sh --build-release
python3 scripts/release/stage-rpc.py
python3 scripts/release/smoke-rpc.py electron/resources/bin
smoke_root=$(mktemp -d)
mkdir -p "$smoke_root/library/shared-resources/models" "$smoke_root/config"
PUMAS_LAUNCHER_ROOT="$smoke_root/library" XDG_CONFIG_HOME="$smoke_root/config" ./launcher.sh --release-smoke
pnpm --dir electron exec electron-builder --linux --publish never
node scripts/release/check-artifacts.mjs electron/release linux
python3 scripts/release/smoke-linux-packages.py electron/release
```

Use Xvfb for Linux GUI smoke on a machine without a display. Build the headless
RPC separately with `--no-default-features`, then run `smoke-rpc.py` with
`--inference-disabled` to check health and inference-route absence. This gate
uses no Node, Electron, or frontend build. The launcher wrappers themselves
still require Node.

The Rust workspace checks exclude `pumas_rustler` because it requires an Erlang
host. `bindings/support-matrix.json` currently accepts no host-binding tuple;
crate archives and host-binding bundles are not release assets.

## Attribution and artifact metadata

The checked-in [attribution collection](docs/release-attribution/0.7.0/README.md)
is embedded in desktop packages. After dependency or manifest changes, run
`python3 scripts/release/generate-notices.py` and review the source-text inventory.
CI and Electron beforePack reject missing or stale attribution. Preserve
Electron/Chromium notices alongside the combined notice.

For Linux, use `python3 scripts/release/verify-deb-install.py CANDIDATE_DEB` for
isolated native install/remove checks. Run `scripts/release/verify-packaged-onnx.py` with the extracted RPC
and a real local Nomic fixture to verify import, load, gateway embeddings and
unload. Generate per-installer SPDX, local provenance and final checksums with
`write-linux-metadata.py`; see its attribution report for scope and limitations.
CI-built releases must use the CI build identity and actual final files.

## Required release acceptance

The candidate workflow rejects missing, empty, duplicate, stale-version, and
unexpected installer archives, and hashes the exact staged bytes. It checks
native RPC startup from staging and both extracted Linux installers, compares
packaged Linux resources against their build inputs, and checks Linux desktop
startup. These
are bounded startup checks, not installer installation or complete user flows.

Before tagging or publishing, satisfy the remaining artifact-plan obligations:

- Extract each exact installer and verify renderer, RPC, native dependencies,
  version identity, and installation/startup on its target platform.
- Exercise the required desktop, launcher-root recovery, and termination flows
  on every claimed target.
- Generate current per-installer SPDX SBOMs, provenance, and third-party notices;
  embed the accepted notices and project license in each distributable.
- Run current Cargo and pnpm vulnerability/license checks and review their
  results against the shipped dependency closure.
- Verify the final publication inventory and regenerate checksums after adding
  release metadata. Candidate checksums cover installers only.
- If shipping a Torch runtime, verify its resolved dependencies and real
  load/inference/control behavior on each claimed device/platform tuple. Unit
  fakes and source adapters do not qualify a runtime distribution.
- Real ONNX inference checks require a valid model/tokenizer fixture. A test
  that returns early without that fixture does not prove packaged inference.

For 0.7.0, Windows is best effort: its independent CI job attempts the native
build, release tests, staged RPC startup, launcher/Electron tests, and packaging.
A failure is recorded but does not block Linux/macOS candidate assembly. Windows
candidate artifacts are uploaded only after every step passes; missing Windows
artifacts cannot be mistaken for a complete five-installer release.

Download destination authority and durable JSON publication currently refuse
non-Unix targets. Those limitations still block qualification of a Windows
artifact, even though they no longer block release of other targets. Native
installation and the remaining acceptance evidence are required before publishing
a passing Windows candidate. See the [dependency review](docs/dependency-review-0.7.0.md)
for the remaining security and attribution decisions.

## Tag and publish

Only after candidate QA and acceptance evidence pass, commit the prepared
version, push that commit, and create/push `vX.Y.Z`. Re-run the candidate workflow
for the tag and confirm it points to the reviewed commit. Upload the accepted
installer and metadata set into a draft release and use the reviewed notes from
`docs/release-notes-X.Y.Z.md`. Publication remains a maintainer action.

Do not move a published tag to repair a broken release. Fix and rehearse a new
version. Failed workflow artifacts are diagnostics, never distribution assets.

## Why this workflow changed for 0.7

The histories leading to v0.1–v0.6 repeatedly needed last-minute fixes for
binding-generator invocations, pnpm/native optional dependencies, action pins,
platform paths and symlinks, test isolation, lint, and Windows RAM-disk behavior.
The previous pipeline also shipped outputs excluded by the current artifact
plan and allowed missing uploads. The replacement removes RAM-disk setup and
unsupported bundle generation, locks Cargo resolution, reads the Rust toolchain
pin once, makes headless execution a separate gate, and rehearses installer
assembly before tags. Release failures remain visible instead of being converted
into partial success.
