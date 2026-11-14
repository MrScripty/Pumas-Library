# Releasing Pumas Library

## Prepare and rehearse before tagging

The `Build` workflow runs on pull requests, `main`, version tags, and manual
`workflow_dispatch`. Ordinary PR/main validation runs source/contract checks
and the dependency scan (advisory there, blocking for candidates); PR, `main`
and ordinary manual runs also produce a separately labeled Linux inference
review archive.
The complete optimized sixteen-artifact matrix runs for a `v*` version tag or an
explicit manual `build_candidates=true` dispatch. Manual candidate builds require
`expected_source` to equal the exact checked-out 40-character commit; a missing or
moved source fails before packaging. The default is false. Candidate mode omits
the separate review jobs and non-shipped Torch E2E; those retain their ordinary
manual/tag behavior. All outputs remain
unsigned, unverified candidates. The workflow has read-only contents permissions
and never creates a tag or GitHub release.

For a build-only rehearsal, first have the maintainer review the exact source
and run action. Set `PUMAS_REHEARSAL_BRANCH` to the branch holding that source
and `PUMAS_REHEARSAL_SHA` to its reviewed full commit, then dispatch:

```bash
gh workflow run build.yml --repo MrScripty/Pumas-Library \
  --ref "$PUMAS_REHEARSAL_BRANCH" \
  -F build_candidates=true -f expected_source="$PUMAS_REHEARSAL_SHA"
```

Do not dispatch this merely because a PR is green. Complete native artifact and
trained-model acceptance before the maintainer's later tag/publication decision.
The same existing package jobs and final inventory checker serve rehearsals and
tags; branch rehearsal does not imply a prerelease or final release. A tag must
match source manifests exactly; the manifests identify `0.8.0`.

1. Update `CHANGELOG.md` and the version's release notes. Keep an empty
   `Unreleased` section for subsequent changes.
2. Set the same version in root, frontend, Electron and
   `packages/desktop-contract` `package.json`, and
   `[workspace.package]` in `rust/Cargo.toml`. Update the workspace package
   versions in `rust/Cargo.lock` without resolving new dependencies.
3. Run `npm run check:release-versions -- vX.Y.Z`. CI also compares version tags
   against the manifests automatically.
4. Complete local QA below, push the candidate commit, and create its version
   tag. Inspect every platform result produced for that exact tag.
5. Review the `release-candidate` workflow artifact. After the tag build
   succeeds, the Draft release workflow attaches the same files to the tag's
   draft GitHub release. Publishing that draft is the maintainer's decision.
   The artifact contains all sixteen
   required files selected by [the artifact plan](scripts/release/artifact-plan.json):
   five full desktop installers (GUI with inference plugins), five
   no-inference desktop installers (GUI with an inference-disabled backend),
   three headless no-inference RPC archives, and three inference headless RPC
   archives for embedding. It also carries
   checksums over those final bytes.

The Build workflow assembles **candidates** with read-only repository
permissions. Only the Draft release workflow can write, and it only attaches a
successful tag build's verified files to that tag's draft release. Nothing in CI
publishes a release. Publication requires all the release acceptance evidence
below; green compilation alone is inadequate.

## Local QA

Use `rust-toolchain.toml`, `.node-version`, the root pnpm pin, and
`.python-version`. Tests require normal subprocess, filesystem, and loopback
socket access; sandbox-denied operations are unavailable evidence, not product
failures.

```bash
pnpm install --frozen-lockfile
npm run check:dependency-ownership
npm run check:release-versions -- v0.8.0
node --test scripts/release/*.test.mjs
./scripts/rust/check.sh
cargo test --locked --manifest-path rust/Cargo.toml --workspace --exclude pumas_rustler --profile ci-test
cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --no-default-features
cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features
pnpm --dir frontend lint
pnpm --dir frontend check:types
pnpm --dir frontend test:run
pnpm --dir frontend build:library-only --outDir /tmp/pumas-library-only --emptyOutDir
node scripts/release/check-frontend-variant.mjs /tmp/pumas-library-only library-only
pnpm --dir frontend build
node scripts/release/check-frontend-variant.mjs frontend/dist default
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

The checked-in [attribution collection](docs/release-attribution/0.8.0/README.md)
is embedded in desktop packages. After dependency or manifest changes, run
`python3 scripts/release/generate-notices.py` and review the source-text inventory.
CI and Electron beforePack reject missing or stale attribution. Preserve
Electron/Chromium notices alongside the combined notice.

For Linux, use `python3 scripts/release/verify-deb-install.py CANDIDATE_DEB` for
isolated native install/remove checks. Run `scripts/release/verify-packaged-onnx.py` with the extracted RPC
and a real local Nomic fixture to verify ONNX Runtime installation through Pumas,
import, load, gateway embeddings and unload. `write-linux-metadata.py` remains a separate optional extracted-file
inventory for the two full Linux installers. It does not generate the complete
sixteen-artifact publication metadata; do not mix its output directory into a
fresh final candidate assembly.
CI-built releases must use the CI build identity and actual final files.

## Inference headless candidates

The tag or explicit build-only `headless-inference` matrix produces Linux x86_64, Windows x86_64
and macOS ARM64 archives through `headless_inference_ci.py` and the existing
strict `headless_inference_build.py` producer. Candidate assembly requires all
three in addition to the thirteen existing assets. A missing or failed native
job blocks the combined candidate. No workflow step publishes a release.
Pull requests and ordinary (non-candidate) manual runs also run the bounded Linux-only
`review-inference-linux` job on the existing `ubuntu-24.04` runner. It checks out
the exact PR head (or manual run SHA), uses the same strict release producer,
extracts the archive, and compares authenticated core/RPC startup identities.
Review archives and diagnostic evidence are retained for seven days under
separate `review-inference-*` artifact names; they cannot satisfy the full
matrix's release-candidate inventory. This job grants neither pretrained-model
acceptance nor Windows/macOS qualification, and publishes no tag or release.
The Linux review job also installs ONNX Runtime through its own extracted Pumas,
then imports, loads, executes and unloads a deterministic untrained ONNX graph
on a profile pinned to that version, observes that the loaded runtime is the one
Pumas installed, and requires graceful shutdown. Its separately labeled fixture
hashes and lifecycle log are execution evidence, not pretrained Nomic acceptance
or embedding-quality evidence. No model weights are downloaded.

The producer requires source version 0.8; older 0.7 sources fail before
compilation. After manifest or dependency changes, regenerate/review the
matching `<version>` and `<version>-s3` attribution directories. The generator and inference producer
select those source-version paths without retaining a hard-coded 0.7 directory.

Headless archives bundle no runtime. Each contains exactly the RPC executable,
`LICENSE.txt`, `THIRD-PARTY-NOTICES.txt`, `protocol-schema.json`,
`build-record.json`, `manifest.json` and `SHA256SUMS`; the manifest is schema 2
(schema 1 archives also packaged ONNX Runtime 1.24.2 and a `runtime` record) and
admission refuses any other member. Consumers install ONNX Runtime, llama.cpp or
Torch through the Pumas version manager, like the desktop. Cargo never downloads
ONNX Runtime: builds keep `ORT_SKIP_DOWNLOAD=1` with the `ort` crate's dynamic
loading, so no step stages, probes or pins a runtime library.

The CI adapter exports the actual source DTO schema separately, pins the source
commit/tree and expected shared build advertisements, and compares the production
executable's actual `--build-info` against those expectations. It builds locked,
offline, native release with exactly `s3,inference-plugins`, embeds the checked
S3 notices, verifies/extracts the resulting archive, and runs authenticated owner
startup/graceful shutdown. Archive artifacts and diagnostic evidence are uploaded
separately. These remain unsigned, `unverified_candidate` outputs; startup never
qualifies model execution.

For real-model acceptance, `verify-packaged-onnx.py RPC NOMIC_DIRECTORY` refuses
any ONNX Runtime, llama.cpp or Torch library beside RPC, starts it with an
allowlist environment and isolated home/config/cache/registry/current directory
(no ambient ORT, loader, Pumas, Python or proxy overrides), and drives its
JSON-RPC/HTTP API: `get_available_versions` for app `onnx-runtime`, then
`install_version` of the newest stable `+cpu` release with
`get_installation_progress` polled to completion (this downloads from
`microsoft/onnxruntime`, so it needs network access and is CI only, never a unit
test). It pins a managed CPU `onnx_serve` profile to that version through
`runtime_version`, launches it, and imports, serves, embeds and unloads the
fixture. The version is resolved at run time; the receipt records which one was
verified. Windows retains the actual OS directory needed for system DLLs and uses
a new process group plus graceful CTRL_BREAK shutdown; Unix uses SIGINT. After
finite embedding inference, every ONNX Runtime library mapped by the RPC process
or its descendants (the out-of-process ONNX host included) must lie in the
installed version directory: Linux `/proc/PID/maps` binds path, device and inode;
Windows PSAPI and macOS lsof observe native module paths only. Missing installs,
installation failures, ambient mappings, model errors and forced shutdown fail
acceptance with their reason. This is not a filesystem sandbox, protection
against concurrent hostile same-user mutation, or a complete audit of every
loaded system dependency. ONNX Runtime's own platform prerequisites (for example
a Microsoft Visual C++ runtime on Windows) are the installed release's, not
redistributed by Pumas.

The focused Python/Node tests use fake RPC results, controlled subprocesses and
controlled mapping text; they download nothing and load no ONNX Runtime. None of
these local checks executes a release build, a runtime installation, a
pretrained ONNX model, native Windows/macOS startup, signing/notarization, a
final SBOM or security qualification. Those remain gates before the maintainer's
publication decision.

## Complete candidate metadata

After collecting the sixteen exact artifacts into a fresh directory, run:

```bash
node scripts/release/check-artifacts.mjs candidate-input required release-candidate
python3 scripts/release/write-release-metadata.py release-candidate --build-id local-candidate-rehearsal
```

CI invokes the same generator with its actual run/attempt build identity. It
requires unchanged tracked source and current default/S3 attribution inputs. It
writes sixteen SPDX 2.3 package inventories, aggregate unsigned in-toto provenance,
project license, combined default/S3 notices, and SHA-256 checksums over
all other final files. SPDX `filesAnalyzed=false` is intentional: these describe
exact artifact bytes and referenced attribution inputs, not exhaustive
extracted-file or native transitive component analysis. No artifact bundles an
inference runtime, so no SBOM lists one; the collected notices describe ONNX
Runtime and uv as downloaded by Pumas. Native package inspection, current audits
and maintainer license/notice acceptance remain separate gates. Embedded notice
identity uses each producer's accepted default/S3 collection; inspect the actual
package contents separately.
Reassemble a fresh set after changing artifacts or metadata; do not retain stale
SPDX/provenance or treat installer-only checksums as final publication coverage.

## Reviewed trained fixture

The documented fixture is Nomic's public `nomic-ai/nomic-embed-text-v1.5`, using
`onnx/model_fp16.onnx` plus its matching `tokenizer.json`, `config.json`,
`tokenizer_config.json` and `special_tokens_map.json`, all at the immutable
revision `ac6fcd72429d86ff25c17895e47a9bfcfc50c1b2` (publisher license:
Apache-2.0). The publisher's Git-LFS pointer gives `onnx/model_fp16.onnx` as
273,859,028 bytes with SHA-256
`cf5b5a86edb00f895561803cfc04729090a958340b8ca2ad76c143f565f6bb04`; that digest
comes from upstream and has not been recomputed locally. Fetch only those five
paths, never the whole (about 2.2 GB) repository. A provided directory is copied into an
isolated library and never modified. The lifecycle receipt records the copied
bytes; it does not itself authenticate their model provenance or establish model
quality. No model is bundled in release artifacts or silently downloaded by this
verifier. The inference matrix additionally executes an explicitly untrained
fixture on each native target; that result cannot accept the trained fixture.

## macOS candidate verification

The macOS ARM64 job mounts the DMG read-only, copies the application with `ditto`,
unmounts it, compares packaged resources, checks ARM64 executables and runs both
RPC and desktop startup from the copied application. Reproduce on a Mac with
`python3 scripts/release/smoke-macos-package.py CANDIDATE_DMG`. This check requires
the matching staged RPC and frontend inputs. It does not qualify Gatekeeper,
notarization, native chooser recovery or interactive desktop behavior. macOS is
a required target.

## Required release acceptance

The candidate workflow rejects missing, empty, duplicate, stale-version, and
unexpected artifacts, and hashes the exact final bytes. It checks native RPC
startup from staging on each target, compares packaged resources against their
build inputs on Linux, macOS and Windows, starts the packaged RPC and desktop
(Linux under Xvfb, the copied macOS application, the installed and portable
Windows applications), and asserts inference-route absence for each
no-inference package. These are bounded startup checks, not complete user
flows.

Before tagging or publishing, satisfy the remaining artifact-plan obligations:

- No package bundles a runtime. `stage-rpc.py` stages only the RPC executable,
  the package smoke checks refuse a packaged ONNX Runtime library, and headless
  archive admission refuses any member beyond the RPC and its metadata. Users
  install ONNX Runtime, llama.cpp and Torch through the Pumas version manager.
  Prove real ONNX execution with an installed version before claiming
  inference; an SDK-free build/health check does not qualify it.
- Extract each exact installer and verify renderer, RPC, native dependencies,
  version identity, and installation/startup on its target platform.
- Exercise the required desktop, launcher-root recovery, and termination flows
  on every claimed target.
- Generate current per-installer SPDX SBOMs, provenance, and third-party notices;
  embed the accepted notices and project license in each distributable.
- Review the run's `current-vulnerability-results` artifact. The
  `dependency-audit` job scans Cargo (advisories and licenses), pnpm and the
  pinned Python inputs, and `release-candidate` refuses a review that failed or
  scanned another commit. See [dependency scans](docs/SECURITY.md#dependency-scans).
- Verify the final publication inventory. Candidate checksums cover every final
  artifact and metadata file; regenerate them if anything is added or changed.
- If shipping a Torch runtime, verify its resolved dependencies and real
  load/inference/control behavior on each claimed device/platform tuple. Unit
  fakes and source adapters do not qualify a runtime distribution.
- Real ONNX inference checks require a valid model/tokenizer fixture. A test
  that returns early without that fixture does not prove packaged inference.

Windows is a required native target. Its CI job runs the build, release tests,
staged RPC startup, launcher/Electron tests, and packaging. The installer check
silently installs the NSIS candidate, compares installed resources with the build
inputs, starts the installed RPC and desktop, and starts the portable executable.
The combined candidate requires all sixteen files: both Windows installer
pairs (full and no-inference) alongside the Linux and macOS pairs and the
six headless archives. Each no-inference desktop additionally asserts
inference-route absence from its packaged backend.

Windows download authority uses held directory handles and physical file identity,
no-follow traversal, a pinned execution lock file, native handle-relative rename,
and writable directory flushes. Durable JSON publication retains the same explicit
failure and durability states across Linux, macOS, and Windows. Native startup
checks do not establish signing, SmartScreen acceptance, or interactive workflows.
The candidate's `current-vulnerability-results` must pass for that exact
commit; re-confirm the [accepted advisory exceptions](docs/SECURITY.md#accepted-advisory-exceptions)
against it.

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
