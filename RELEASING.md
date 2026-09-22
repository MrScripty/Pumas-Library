# Releasing Pumas Library

## Prepare and rehearse before tagging

The `Build` workflow runs on pull requests, `main`, version tags, and manual
`workflow_dispatch`. Pull requests, ordinary branch pushes, and branch-targeted
manual runs execute source quality and contract tests only. Fully optimized
backends, renderer bundles, installers, archives, artifact uploads, startup
smoke, and release-candidate assembly run only for a `v*` version tag (including
a manual dispatch explicitly targeting that tag). Complete the local release QA
before creating the tag; CI then qualifies the exact tagged commit and assembles
its candidate artifacts.

1. Update `CHANGELOG.md` and the version's release notes. Keep an empty
   `Unreleased` section for subsequent changes.
2. Set the same version in root, frontend, and Electron `package.json`, and
   `[workspace.package]` in `rust/Cargo.toml`. Update the workspace package
   versions in `rust/Cargo.lock` without resolving new dependencies.
3. Run `npm run check:release-versions -- vX.Y.Z`. CI also compares version tags
   against the manifests automatically.
4. Complete local QA below, push the candidate commit, and create its version
   tag. Inspect every platform result produced for that exact tag.
5. Review the `release-candidate` workflow artifact. It contains all thirteen
   required files selected by [the artifact plan](scripts/release/artifact-plan.json):
   five full desktop installers (GUI with inference plugins), five
   no-inference desktop installers (GUI with an inference-disabled backend),
   and three headless no-inference RPC archives for embedding. It also carries
   checksums over those final bytes.

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
cargo test --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --profile ci-test
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

Native workspace tests use `ci-test`, which inherits release optimizations and
assertion settings but disables whole-program LTO and uses 16 code-generation
units to avoid repeatedly optimizing every test executable. Installers still
contain the fully optimized `release` backend, verified by native startup smoke.
Torch installer tests use small local HTTP fixtures and offline requirements;
they do not download Torch wheels and fail with stage logs after 60 seconds.

Use Xvfb for Linux GUI smoke on a machine without a display. Build the headless
RPC separately with `--no-default-features`, then run `smoke-rpc.py` with
`--inference-disabled` to check health and inference-route absence. This gate
uses no Node, Electron, or frontend build. The launcher wrappers themselves
still require Node.

The headless embedding archives are assembled with
`python3 scripts/release/make-headless-archive.py --binary rust/target/release
--output-dir headless-output --os <linux|macos|windows>` after that smoke
check; the binary inside keeps its plain `pumas-rpc` (`pumas-rpc.exe`) name
while the archive filename carries the `-no-inference-` marker. The binary
is self-contained: the `pumas-library` core is compiled into `pumas-rpc`,
so embedders run one binary, not a core-plus-RPC pair. Verify one
archive with `node scripts/release/check-artifacts.mjs headless-output
headless-<linux|macos|windows>`.

The no-inference desktop reuses the exact headless backend bytes: stage the
extracted `pumas-rpc` into `electron/resources/bin`, package with
`electron-builder --<platform> --publish never -c
./electron-builder.no-inference.cjs` (the config file carries the
`com.pumas.library.no-inference` identity and `-no-inference` artifact
names; electron-builder has no `-c.key=value` CLI overrides), then verify
with the `linux-no-inference`, `mac-no-inference`, or `win-no-inference`
checker tokens and the matching `smoke-*-packages.py --variant no-inference`.
`scripts/release/check-no-inference-config.test.mjs` pins the config's names
to the artifact plan. The no-inference installers
share their product name with the full line, so install only one desktop
variant per machine; AppImage, portable, and headless archives are
side-by-side safe. In-process Rust API consumers keep depending on the
immutable Git revision (the plan's `pantograph` source consumer); the archives
cover sidecar/subprocess embedding, not host-language bindings, which still
have no accepted tuple.

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

## macOS candidate verification

The macOS ARM64 job mounts the DMG read-only, copies the application with `ditto`,
unmounts it, compares packaged resources, checks ARM64 executables and runs both
RPC and desktop startup from the copied application. Reproduce on a Mac with
`python3 scripts/release/smoke-macos-package.py CANDIDATE_DMG`. This check requires
the matching staged RPC and frontend inputs. It does not qualify Gatekeeper,
notarization, native chooser recovery or interactive desktop behavior. macOS
remains a required target; its native results are pending until that job runs.

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

Windows is a required native target. Its CI job runs the build, release tests,
staged RPC startup, launcher/Electron tests, and packaging. The installer check
silently installs the NSIS candidate, compares installed resources with the build
inputs, starts the installed RPC and desktop, and starts the portable executable.
The combined candidate requires all thirteen files: both Windows installer
pairs (full and no-inference) alongside the Linux and macOS pairs and the
three headless archives. Each no-inference desktop additionally asserts
inference-route absence from its packaged backend.

Windows download authority uses held directory handles and physical file identity,
no-follow traversal, a pinned execution lock file, native handle-relative rename,
and writable directory flushes. Durable JSON publication retains the same explicit
failure and durability states across Linux, macOS, and Windows. Native startup
checks do not establish signing, SmartScreen acceptance, or interactive workflows.
See the [dependency review](docs/dependency-review-0.7.0.md) for the remaining
security and attribution decisions.

## Tag and publish

Only after candidate QA and acceptance evidence pass, commit the prepared
version, push that commit, and create/push `vX.Y.Z`. Re-run the candidate workflow
for the tag and confirm it points to the reviewed commit. Upload the accepted
installer and metadata set into a draft release and use the reviewed notes from
`docs/release-notes-X.Y.Z.md`. Publication remains a maintainer action.

Do not move a published tag to repair a broken release. Fix and rehearse a new
version. Failed workflow artifacts are diagnostics, never distribution assets.
(The v0.7.0 tag was moved once before any publication because its original
commit never passed CI and no release was cut from it; that pre-publication
move is the exception, not the practice.)

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
