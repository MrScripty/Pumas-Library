# Releasing Pumas Library

## Prepare and rehearse before tagging

The `Build` workflow runs on pull requests, `main`, version tags, and manual
`workflow_dispatch`. Ordinary PR/main validation runs source/contract checks;
PR/ordinary manual runs also produce a separately labeled Linux inference review
archive.
The complete optimized sixteen-artifact matrix runs for a `v*` version tag or an
explicit manual `build_candidates=true` dispatch. Manual candidate builds require
`expected_source` to equal the exact checked-out 40-character commit; a missing or
moved source fails before packaging. The default is false. Candidate mode omits
the separate review jobs and non-shipped Torch E2E; those retain their ordinary
manual/tag behavior. All outputs remain
unsigned, unverified candidates. The workflow has read-only contents permissions
and never creates a tag or GitHub release.

For a build-only rehearsal, first have the parent review the exact source and
run action. Set `PUMAS_REHEARSAL_SHA` to that reviewed full commit, then dispatch
this branch (or the accepted branch containing this workflow):

```bash
gh workflow run build.yml --repo MrScripty/Pumas-Library \
  --ref release/v080-package-qualification-20261014 \
  -F build_candidates=true -f expected_source="$PUMAS_REHEARSAL_SHA"
```

Do not dispatch this merely because a PR is green. Complete native artifact and
trained-model acceptance before the maintainer's later tag/publication decision.
The same existing package jobs and final inventory checker serve rehearsals and
tags; branch rehearsal does not imply a prerelease or final release. A tag must
match source manifests exactly. Current `0.8.0-rc.1` must never be tagged `v0.8.0`.

1. Update `CHANGELOG.md` and the version's release notes. Keep an empty
   `Unreleased` section for subsequent changes.
2. Set the same version in root, frontend, and Electron `package.json`, and
   `[workspace.package]` in `rust/Cargo.toml`. Update the workspace package
   versions in `rust/Cargo.lock` without resolving new dependencies.
3. Run `npm run check:release-versions -- vX.Y.Z`. CI also compares version tags
   against the manifests automatically.
4. Complete local QA below, push the candidate commit, and create its version
   tag. Inspect every platform result produced for that exact tag.
5. Review the `release-candidate` workflow artifact. It contains all sixteen
   required files selected by [the artifact plan](scripts/release/artifact-plan.json):
   five full desktop installers (GUI with inference plugins), five
   no-inference desktop installers (GUI with an inference-disabled backend),
   three headless no-inference RPC archives, and three inference headless RPC
   archives for embedding. It also carries
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
npm run check:release-versions -- v0.8.0-rc.1
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
pumas_ort_work=$(mktemp -d)
python3 scripts/release/onnx_runtime_stage.py --target linux-x86_64 --download --output-dir "$pumas_ort_work/runtime"
python3 scripts/release/onnx_runtime_probe.py --target linux-x86_64 --runtime-dir "$pumas_ort_work/runtime" --output "$pumas_ort_work/loader.json"
python3 scripts/release/stage-rpc.py --target linux-x86_64 --runtime-dir "$pumas_ort_work/runtime"
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

The checked-in [attribution collection](docs/release-attribution/0.8.0-rc.1/README.md)
is embedded in desktop packages. After dependency or manifest changes, run
`python3 scripts/release/generate-notices.py` and review the source-text inventory.
CI and Electron beforePack reject missing or stale attribution. Preserve
Electron/Chromium notices alongside the combined notice.

For Linux, use `python3 scripts/release/verify-deb-install.py CANDIDATE_DEB` for
isolated native install/remove checks. Run `scripts/release/verify-packaged-onnx.py` with the extracted RPC
and a real local Nomic fixture to verify import, load, gateway embeddings and
unload. `write-linux-metadata.py` remains a separate optional extracted-file
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
Pull requests and manual workflow runs also run the bounded Linux-only
`review-inference-linux` job on the existing `ubuntu-24.04` runner. It checks out
the exact PR head (or manual run SHA), uses the same strict release producer,
extracts the archive, and compares authenticated core/RPC startup identities.
Review archives and diagnostic evidence are retained for seven days under
separate `review-inference-*` artifact names; they cannot satisfy the full
matrix's release-candidate inventory. This job grants neither pretrained-model
acceptance nor Windows/macOS qualification, and publishes no tag or release.
The Linux review job also imports, loads, executes and unloads a deterministic
untrained ONNX graph through its own extracted Pumas binary, observes the pinned
native runtime mapping, and requires graceful shutdown. Its separately labeled
fixture hashes and lifecycle log are execution evidence, not pretrained Nomic
acceptance or embedding-quality evidence. No model weights are downloaded.
The integrated candidate identity is `0.8.0-rc.1`. This source-version preparation
does not publish a tag or release and does not qualify native model execution.
The producer requires source version 0.8; older 0.7 sources fail before compilation.
After manifest or dependency changes, regenerate/review the matching `<version>`
and `<version>-s3` attribution directories. The generator and inference producer
select those source-version paths without retaining a hard-coded 0.7 directory.

`onnx-runtime-pins.json` records the official CPU 1.24.2 GitHub release archive
digests and the hashes/sizes of selected native members and license/notice bytes.
Those archive digests were checked against upstream release metadata and the
actual downloaded archives. `onnx_runtime_stage.py --target TARGET --archive FILE
--output-dir FRESH_DIRECTORY` verifies a private snapshot and writes only named
regular members, materializing Linux's versioned library under the loader name.
`--download` explicitly authorizes fetching that exact pinned official archive;
Cargo continues to use `ORT_SKIP_DOWNLOAD=1`. Pin updates require reviewing both
upstream archive identity and actual member/notice bytes. The records cover the
shipped CPU ORT files and observed static direct imports. Linux ORT requires
GLIBC 2.27, GLIBCXX 3.4.22 and CXXABI 1.3.11 (the final RPC may require newer
versions). Windows additionally requires compatible MSVCP140/MSVCP140_1 and
VCRUNTIME140/VCRUNTIME140_1 DLLs, which are absent from the upstream ORT archive;
users must have a compatible Microsoft Visual C++ runtime. The pinned macOS
library declares minimum macOS 14.0 in its Mach-O build command and imports
system frameworks and libraries. These prerequisites are not redistributed or qualified
merely by recording their names; recursive/session-dependent closure, signatures
and GPU providers remain separate gates.

Before compilation, `onnx_runtime_probe.py` runs an isolated, time-bounded native
child, validates the exact files, loads ORT and requires version 1.24.2 and C API
24. Every native workflow target must pass that loader gate; it detects loader
failures that lazy RPC startup cannot. The probe runs inside Python, so libraries
already loaded by that interpreter are part of its host context; it does not
substitute for exercising ORT from the final extracted RPC process. Linux evidence additionally
hashes every observed process file mapping before and after the API probe and
checks mapped device/inode identity. These include host/Python libraries and
are an observation of this process, not a universal redistributable closure.
Windows/macOS mapped-file audit remains unavailable in this probe. Process exit
ends the probe's native lifetime; no model or session is created.

The CI adapter exports the actual source DTO schema separately, pins the source
commit/tree and expected shared build advertisements, and compares the production
executable's actual `--build-info` against those expectations. It builds locked,
offline, native release with exactly `s3,inference-plugins`, embeds the checked
S3 notices plus exact upstream native notices, verifies/extracts the resulting
archive, and runs authenticated owner startup/graceful shutdown. Archive artifacts
and diagnostic evidence are uploaded separately. These remain unsigned,
`unverified_candidate` outputs; startup never qualifies model execution.

For native real-model acceptance, `verify-packaged-onnx.py RPC NOMIC_DIRECTORY`
requires the selected target's pinned CPU ORT files beside RPC. The child receives an allowlist
environment and isolated home/config/cache/registry/current directory, with no
ambient ORT, loader, Pumas, Python or proxy overrides. Windows retains the actual
OS directory needed for system DLLs and uses a new process group plus graceful
CTRL_BREAK shutdown; Unix uses SIGINT. Fixture copying uses Python on all hosts.
After finite embedding inference, Linux `/proc/PID/maps` must identify the packaged
loader by path, device and inode. Windows PSAPI and macOS lsof observe the actual
process's native module paths; current packaged bytes must match the reviewed pin.
Windows/macOS mapped-file-object identity is not established by these path
observations. The receipt records that narrower scope and copied fixture hashes. Missing runtime,
ambient mappings, model errors and forced shutdown fail acceptance. This observes
the mapped ORT file; it is not a filesystem sandbox, protection against concurrent
hostile same-user mutation, or a complete audit of every loaded system dependency.

The focused Python/Node tests use synthetic native bytes, controlled subprocesses
and controlled mapping text. Successful staging of real official Linux/Windows/
macOS archives establishes byte identity only. A Linux native loader/API probe
also passed against the pinned library in this environment. A separate C harness
executed a real CPU session using a synthetic two-element Identity graph, observed
the pinned mapped runtime, released its native handles and exited successfully.
That bounded untrained graph is not Pumas/Nomic or pretrained-model evidence.
This development environment has not executed a v0.8 release build, pretrained ONNX model, native Windows/macOS
startup, signing/notarization, final SBOM or security qualification. Preserve those
gates before the maintainer's publication decision.

## Complete candidate metadata

After collecting the sixteen exact artifacts into a fresh directory, run:

```bash
node scripts/release/check-artifacts.mjs candidate-input required release-candidate
python3 scripts/release/write-release-metadata.py release-candidate --build-id local-candidate-rehearsal
```

CI invokes the same generator with its actual run/attempt build identity. It
requires unchanged tracked source and current default/S3 attribution inputs. It
writes sixteen SPDX 2.3 package inventories, aggregate unsigned in-toto provenance,
project license, combined default/S3/native notices, and SHA-256 checksums over
all other final files. SPDX `filesAnalyzed=false` is intentional: these describe
exact artifact bytes and referenced attribution/runtime inputs, not exhaustive
extracted-file or native transitive component analysis. Native package inspection,
current audits and maintainer license/notice acceptance remain separate gates.
The aggregate disclosure includes every pinned native notice representation,
including Windows CRLF bytes. Embedded notice identity continues to use each
producer's accepted default/S3 collection and exact target runtime notice bytes;
inspect those actual package contents separately.
Reassemble a fresh set after changing artifacts or metadata; do not retain stale
SPDX/provenance or treat installer-only checksums as final publication coverage.

## Reviewed trained fixture

The documented fixture is Nomic's public `nomic-ai/nomic-embed-text-v1.5`, using
`onnx/model_fp16.onnx` plus its matching tokenizer/config files. The
[fixture investigation](docs/plans/v080-packaging/reports/nomic-fixture.md) records
the official revision, publisher license declaration, exact weight identity and
remaining byte-verification limits. A provided directory is copied into an
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

- Separately provision and verify ONNX Runtime for full inference packages.
  Cargo never downloads it. Stage the selected 1.24.2 distribution's native
  library closure beside RPC, with trusted archive/library hashes and notices;
  `stage-rpc.py --target TARGET --runtime-dir DIRECTORY` accepts only named pinned
  native members and notices from that explicitly provisioned directory; it never
  searches ambient developer outputs for runtime libraries.
  Prove real ONNX execution from the extracted package with developer runtime
  paths removed. An SDK-free build/health check does not qualify inference.
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
The combined candidate requires all sixteen files: both Windows installer
pairs (full and no-inference) alongside the Linux and macOS pairs and the
six headless archives. Each no-inference desktop additionally asserts
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

The bounded PR/manual `review-audio-dependencies` job separately requires the
frozen non-model audio dependency cohort to execute under real Linux confinement.
Unsupported confinement or recipe mismatch fails the job. Its evidence contains
the exact source head/tree, probe report and log; installed runtime bytes are not
uploaded. This dependency probe does not qualify a pretrained ASR model.
