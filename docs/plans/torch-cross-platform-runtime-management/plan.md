# Plan: Cross-Platform Torch Runtime Management

**Plan status:** `Active`

**Owner:** Pumas runtime integration

**Authorization:** The repository owner authorized Windows and macOS Torch
runtime management on 2026-09-24, retaining the existing Linux support. On
2026-09-25, the owner authorized Pumas-managed CPython provisioning and removed
the requirement for a host-installed Python interpreter or a user Python choice.

**Current acceptance:** Manual native run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
on commit `07f7e9f6` passed v2.14.0 CPU/Core RPC installation and restart on
Linux x86_64, Windows x64, and macOS arm64. Each target provisioned Pumas-managed
CPython 3.14.7, resolved and installed 25 hashed official artifacts, completed a
fresh CPU tensor operation returning 14 after a second-backend restart, rechecked
the retained resolver probe, passed protocol 3 sidecar trial/stop, and shut down
gracefully. macOS also honored a 913-second Retry-After before release discovery
succeeded on attempt two. Retained evidence: [Linux](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md),
[Windows](reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md), and
[macOS](reports/v2.14.0-macos-cpu-rpc-restart-acceptance/README.md).

That native run predates the current bounded metadata-lock follow-up, so it does
not verify that change. Acceptance is limited to the CPU/Core RPC install and
restart path. CUDA/device execution, Electron UI-driven Torch installation,
and v2.14.0 image generation through Tuldok remain unverified. Existing Tuldok
image evidence remains scoped to its recorded Torch 2.10 tuple.

Current-branch Linux v0.7.0 AppImage and deb candidates were rebuilt with the
generated CPython notices. Artifact naming and extracted resource hashes pass;
both extracted backends start and pass `/health`. The packaged Electron archive
contains the `get_torch_release_options` bridge method. AppImage SHA-256:
`1398ef0a9da1c0aab90681d3c91674ef88c6229a84984047938e7bd6eb350acd`; deb
SHA-256: `468b6f7c2af00ff8785f80e5486cd5133979e875dd351b4b9f5284ddfd195043`.
These are local build outputs: they do not exercise Electron UI-driven Torch
installation or replace the toolbar-linked public v0.7.0 assets (published
2026-09-17). The
generated attribution inventory includes all three CPython full-archive notice
supersets and verifies 371 package entries, 78 hashed inputs, and 57 retained
legal texts against target-specific evidence.

The bundled Linux backend from this candidate then passed the full Torch 2.14.0
CPU/Core install/restart acceptance with backend `PATH` cleared. It provisioned
managed CPython 3.14.7, installed 25 hashed official artifacts, performed a CPU
operation returning 14 after restart, and passed probe plus protocol 3 sidecar
start/stop. See the [packaged Linux acceptance](reports/v2.14.0-linux-packaged-cpu-acceptance/README.md).
This exercised the packaged backend; Electron UI interaction and packaged
Windows/macOS install acceptance remain unverified.
**Current phase:** v2.14.0 CPU/Core RPC install and two-session sidecar
lifecycle are accepted on the current exact-wheel implementation across Linux
x86_64, Windows x64, and macOS arm64. Each run provisioned CPython 3.14.7,
resolved 25 hashed core artifacts, ran a CPU operation returning 14 after a
second-session restart, and passed sidecar trial/stop plus graceful shutdown.
The macOS run also verified the bounded long `Retry-After` path. That native
run predates the current local lock follow-up. The PR repair closes the reviewed
cross-process metadata races by guarding Torch metadata read/modify/write with
`.torch-versions.lock`, refreshing state under that lease, and retaining cloned
leases through detached writes. Startup
defers validation/normalization when the lock is busy and avoids reading a
possibly transitional active marker. Torch status reads now use one coherent
cached generation. The repair also addresses failed child-custody drain,
best-effort cleanup recovery, bounded Electron Torch RPC requests, and fixed
preset access while upstream release discovery is pending. Selection, default
selection, removal, and installation now retry brief `WouldBlock` contention
for at most 500 ms; other I/O errors propagate immediately.

Follow-up verification passes 205 `pumas-app-manager` tests, Rust formatting,
workspace check and Clippy gates, 44 Torch resolver tests, Ruff, and
`git diff --check`. The full local workspace test did not complete cleanly:
`pumas-library` unit tests passed when rerun with four threads (1432 passed, 6
ignored), but the separate `intent_api_tests` hit local filesystem permission
and temporary-storage errors. This does not identify a failure in the changed
Torch paths. The current PR commit still needs its own CI run. Earlier PR run
[36225003150](https://github.com/MrScripty/Pumas-Library/actions/runs/36225003150)
on `66ccbfe7` passed required checks, including native Linux/Windows/macOS QA;
PR-triggered RPC E2E is skipped. Astra high and Sol xhigh completed read-only
reviews with no lock-lifecycle blockers. Manual run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
passed native v2.14.0 CPU/Core RPC install/restart on all three shipped targets,
but predates the current lock follow-up. Windows GNU compilation remains
compile-only evidence but is supplemented by the native Windows acceptance run.

**Remaining gates:** Packaged Electron UI interaction, packaged Windows/macOS
Torch installation, and provider cancellation/tamper/retry cases remain open.
CUDA/device acceleration and v2.14.0 image/Tuldok generation are untested. The
local AppImage/deb candidates do not update the public toolbar-linked package.

**Next slice:** Exercise the Torch discovery/install controls through the Linux
Electron UI and run packaged install acceptance on Windows and macOS; then close
provider cancellation/tamper gates. Keep Tuldok qualification limited to its
recorded exact image-generation tuple; publish a new toolbar-linked release only
after the PR is merged and a new version is tagged.

## Objective and scope

Extend the existing Torch release manager so users can discover a stable
upstream Torch release, let Pumas provision the newest stable CPython that
matches an exact official wheel and resolves the complete selected dependency
profile, install and inspect it, explicitly select it, and start/stop its
managed sidecar through the existing RPC and desktop flows. The host does not
need Python on `PATH`, and the desktop does not require a Python selection.

“Any release” continues to mean an upstream stable tag for which an exact host,
stable standard-CPython wheel, and complete dependency resolution exist. It does
not promise a wheel for every release or every supported host. Source builds and
unqualified adapters remain outside this objective.

The current accepted Linux contract remains the regression baseline in the
[upstream version-management plan](../torch-upstream-version-management/plan.md)
and its [runtime inventory](../torch-diffusion-serving/reports/upstream-version-manager-progress.md).
Native RPC acceptance now proves the v2.14.0 CPU/Core managed install and
restart path on Windows x64 and macOS arm64 as well as Linux x86_64. Broader
platform acceptance remains gated on the incomplete X2–X7 checks below,
especially packaged desktop installation.
This plan does not claim image generation or Tuldok support for every Torch
release or operating system; existing image evidence remains limited to its
recorded exact tuples.

## Binding support contract

| Pumas target | Rust target triple and shipped artifact | Torch runtime behavior | Explicit limits |
| --- | --- | --- | --- |
| Linux x86_64 | `x86_64-unknown-linux-gnu`; AppImage and deb | Preserve accepted exact CPU/CUDA/ROCm wheel discovery and lifecycle | Existing qualification is unchanged; no Linux ARM claim |
| Windows x86_64 | `x86_64-pc-windows-msvc`; NSIS and portable | Discover exact official CPU wheels; expose exact official CUDA wheels only when NVIDIA hardware is positively detected. CPU remains the safe default until Windows-specific driver compatibility is established. | No Windows ARM, ROCm, or XPU claim |
| macOS arm64 | `aarch64-apple-darwin`; arm64 DMG | Discover exact official native CPU wheels; inspect MPS after installation as an optional runtime capability, not a separate Torch build channel | The interpreter and wheel must be native arm64. Intel Mac and Rosetta/x86_64 Torch runtimes are unsupported. No CUDA/ROCm claim |

The target triples and artifacts match the current release contract in
[`artifact-plan.json`](../../../scripts/release/artifact-plan.json#L58). Pumas
owns a private CPython installation depot. Candidate stable CPython minor
versions come from the exact official Torch wheel tags and the pinned Python
distribution catalog in the pinned uv release; do not maintain a Pumas upper
version allowlist or treat prerelease/free-threaded tags as standard CPython.
CPython 3.10 is Pumas' runtime minimum; there is no configured upper minor cap.
Rank candidates numerically from newest to oldest, validate each installed
interpreter's observed target and `sys_tags()`, and retain the first complete
dependency resolution. A proven missing wheel or dependency incompatibility can
advance to the next candidate. Network failure, timeout, provisioning failure,
integrity failure, or incomplete scans are inconclusive and cannot justify a
lower Python version. The user never has to install or choose Python.

The app-manager downloads and bootstraps uv 0.12.18 for the shipped Linux x86_64 GNU, Windows
x86_64 MSVC, and macOS arm64 targets from versioned official archives. It
verifies each archive against the exact SHA-256 in the provider report before
extracting or executing it. uv's Python distribution catalog is embedded and
frozen per uv release; its exact binary pin therefore pins that catalog too.
The manager enumerates that catalog on the native host, filters to stable
standard CPython candidates, and sorts them newest-first. One managed
interpreter acts as a bootstrap for native wheel discovery; the resolver
synthesizes standard CPython tags for the catalog candidates, so discovery does
not install every Python minor. Preview provisions real candidates in bounded
newest-first order and runs complete dependency resolution under each candidate.
An exact absent wheel or definite dependency incompatibility permits trying the
next candidate. Network, provider, timeout, integrity, or incomplete-scan
outcomes stop as inconclusive. uv
provisions Python Build Standalone CPython releases; Pumas must retain the
selected distribution URL, version, target/build, pinned uv/catalog identity,
and a fingerprint of the installed interpreter. uv verifies the CPython archive
using the checksum in its pinned embedded catalog. The CPython build source must
be described accurately and not as a Python.org installer.

The release manager retains the exact upstream wheel URL, hash, distribution
version, build identity, managed interpreter distribution identity and
fingerprint, and dependency lock. Validate
the observed installed Torch identity against that retained resolution. The
Torch distribution version is not always `release+build`: for example, the
official Torch 2.14.0 CPU index publishes Windows wheels named
`2.14.0+cpu`, while its macOS arm64 wheels are named `2.14.0` and tagged
`macosx_14_0_arm64`. Preserve the release/build identity separately from the
upstream distribution version. Do not manufacture a `+cpu` local version for
the macOS artifact.

Exact package availability is determined from official indexes and the
provisioned interpreter's `packaging.tags.sys_tags()`, not a Pumas Torch release
allowlist or a guessed OS floor. Full preview remains required before install;
an exact wheel match alone never claims dependency compatibility. On macOS,
MPS availability is probed from the installed Torch runtime and host. MPS does
not establish model/adapter support. CUDA recommendation on Windows remains
CPU unless a Windows-specific driver rule and required evidence are added to
this contract; exact NVIDIA CUDA wheels may remain explicit advanced choices.

Stable upstream tag discovery remains global and independent of this support
matrix; install choices are limited to exact compatible host tuples. The
bundled v2.9.1 Linux preset is Linux-only. The manager will expose an explicit
`bundledPresetAvailable` host capability, true only for Linux x86_64 when its
managed CPython 3.12 provider artifact is available; the desktop must use that
result instead of the release tag alone. Windows and macOS use the retained
dynamic preview and wheel lock.

On every supported target, Core Torch runtime (`none`) is the default dependency
profile. Linux may offer Pumas' FLUX.2 profile as an explicit advanced choice;
Windows and macOS offer only core Torch until a platform-specific image adapter
has its own accepted plan. The desktop displays the manager-provided default
rather than guessing from the OS. This enables portable Torch install/select/
start without suggesting FLUX.2 or Nunchaku inference has been qualified on
every release or platform.

## Ownership and design

- `pumas-app-manager` owns release choices, CPython candidate ranking and
  provisioning, retained interpreter leases/previews, staged Torch installation,
  identity, selection, and cleanup admission. Python artifacts live in an
  immutable Pumas-managed depot; uv's disposable download cache is separate.
- The app-manager invokes only the exact-hash-pinned uv binary that it has
  downloaded into its private bootstrap directory and verified. It clears
  ambient uv configuration, forces manager-owned Python, uses private
  install/cache directories, disables PATH shims and (on Windows) registry
  writes, and never runs a shell installer or administrator operation.
- A selected Python distribution's source catalog identity, exact CPython
  version, native target/build, download URL, uv/catalog identity, executable
  fingerprint, and observed `sys_tags()` remain attached to its retained
  preview and installed runtime record. uv verifies the Python archive with the
  digest from its embedded catalog.
  Existing installations keep their original base interpreter; managed-runtime
  cleanup may not delete an interpreter referenced by a preview, install, or
  installed runtime. Automatic pruning is out of scope until reference-safe
  removal is implemented.
- `torch-server/resolve_runtime.py` continues to own pip dry-run resolution,
  wheel-tag matching, official wheel provenance, and hash reporting. Its
  protocol must carry the upstream distribution version and exact build as
  separate validated facts.
- `pumas-core::platform::paths` owns OS-native venv executable paths.
- `pumas-core::runtime_profiles::process_owner` owns the persistent
  generation-specific sidecar process lifecycle. Platform process primitives
  belong in the existing platform process API and, if the lifecycle interface
  needs its own depth, one `pumas-core/src/platform/managed_child.rs` module;
  installer and resolver operations remain their manager owners.
- RPC and generated contracts remain unchanged unless the request must allow an
  automatic Python choice or the response must report provisioning progress and
  selected interpreter identity. Any contract change is serial and updates its
  generator, Rust source, generated artifacts, preload, and consumers together.
- The desktop remains a presentation of manager-returned exact choices. It must
  not infer OS support, invent builds, select a mismatched interpreter, or
  interpret MPS as an image adapter. It never requires a Python selector and
  reports the manager-selected interpreter after resolution. It uses manager-provided
  `bundledPresetAvailable` and `defaultAdapter` capabilities rather than
  enabling the Linux bundled preset merely because the selected tag is v2.9.1.

### Composed-design review

**Applicability:** `applicable` — the work changes target selection, dependency
provisioning, Python runtime persistence, and process-lifecycle seams across the
resolver, manager, desktop, and platform runtime owners.

1. **Independent concerns:** upstream wheel identity and interpreter tags
   (Python resolver); managed CPython artifacts and leases (interpreter owner);
   host hardware facts and recommendations (manager); staged version
   identity/publication (installer); native process supervision (platform
   lifecycle); choice presentation (RPC/desktop).
2. **Interleavings:** release/build/distribution version; target/architecture/
   Python ABI and wheel tags; Python provider version/catalog/artifact and
   retained interpreter; hardware/driver evidence; wheel URLs and hashes;
   staged directory and cleanup; process generation and cancellation. These
   remain explicit fields/outcomes at the manager boundaries.
3. **Caller knowledge:** RPC/desktop clients consume manager choices, retained
   preview identifiers, interpreter identity, and lifecycle status. They do not
   parse platform paths, wheel filenames, drivers, or process IDs to decide
   support.
4. **Representative changes:** a new supported host must update target facts,
   exact wheel fixtures/resolution, manager host/interpreter detection, native
   lifecycle evidence, provisioned Python target, and its native CI acceptance.
   A new upstream wheel tag remains local to official-tag resolution and its
   tests. A new image adapter does not change this runtime-management contract.
5. **Stable interfaces:** exact artifact and host facts cross the resolver to
   manager; the retained resolution crosses preview to installer; process
   owners expose generation-scoped lifecycle results. OS path strings,
   display text, or ambient command lookup do not act as artifact identity.
6. **Independent verification:** resolver tags can be fixture-tested; host
   detection, Python provisioning, and venv paths require clean-host native
   execution; process-group/job ownership, cleanup, and sidecar lifecycle also
   require native target execution. Integration remains in the manager/API and
   desktop composition.
7. **Independent evolution and deletion test:** Linux/macOS process-group
   mechanics and Windows Job ownership may fail and evolve independently behind
   one generation-lifecycle contract. The Windows child must be created
   suspended (or through an equivalent atomic job-list mechanism), assigned to
   a retained kill-on-close Job, and resumed only after assignment; the Job
   remains held until the generation is drained. Native tests cover cancellation
   at admission and prove no child escapes. Do not add a generic OS
   Strategy/Factory or one-file-per-platform mirror. Any new process primitive
   must remove duplicate Linux-only process-tree logic from its callers and
   enforce generation ownership; otherwise delete it.
8. **Necessary complexity:** exact wheel-tag compatibility, private interpreter
   provisioning, and native process-tree ownership are required complexity,
   contained in the resolver, manager-owned interpreter lifecycle, and process
   lifecycle. uv is selected over local archive installation because it owns
   Python distribution selection, integrity checks, and platform installation;
   Pumas retains policy, artifact identity, private paths, leases, and cleanup.
   The target uv binary and its embedded interpreter catalog are pinned
   together. Do not use ambient uv or runtime `latest` URLs.

## Acceptance claims

| ID | Observable criterion | Evidence required | Status |
| --- | --- | --- | --- |
| X1 | Linux behavior and existing accepted v2.9.0 install/v2.10 Tuldok evidence remain accurately scoped and pass relevant regression gates | Existing Linux manager/resolver/RPC/frontend suites; retained historical evidence references | pending |
| X2 | Each shipped target provisions a private stable standard CPython 3.10 or newer without a host Python dependency or global PATH/registry changes; artifact integrity, target identity, cache separation, cancellation, and retention are verified | Linux, Windows, and macOS v2.14.0 RPC installs provisioned CPython 3.14.7 and matched persisted interpreter identity across sessions ([Linux](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md), [Windows](reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md), [macOS](reports/v2.14.0-macos-cpu-rpc-restart-acceptance/README.md)). Artifact/source hashes are retained and all three full-archive license supersets are included in generated attribution; cancellation and cache separation remain | partial |
| X3 | Candidate versions come from exact official standard-CPython wheel tags and the pinned stable provider catalog; the manager tries highest to lowest, selects the newest complete compatible resolution, and never treats network/provisioning failure as a reason to downgrade | CPython 3.14 preference when fully resolved; definite 3.14 dependency incompatibility falls to 3.13; prerelease/free-threaded/wrong-target and pre-3.10 artifact rejection; incomplete scan/provider failures remain typed inconclusive | pending |
| X4 | Windows x64 and macOS arm64 discovery returns only exact official wheels whose tags match the provisioned native CPython; CPU/MPS/CUDA choices follow this contract | Native wheel fixtures and wrong-target rejection passed; manual native RPC run `36223106097` resolved exact official CPU wheel tags on Linux, Windows, and macOS arm64. CUDA hardware execution and macOS MPS acceleration remain untested | partial |
| X5 | Preview retains the exact distribution version, release/build, official wheel URL/hash, Python provider artifact identity, interpreter fingerprint, and complete dependencies; install stages and verifies that exact identity before publication | Run `36223106097` resolved 25 hashed CPU/Core artifacts and passed v2.14.0 install on all three shipped targets; each report retains the exact Torch wheel URL/hash, dependency set, and managed interpreter identity (see X2 reports) | partial |
| X6 | Installed versions can be inspected, explicitly selected, started with health/protocol checks, and stopped by their owned generation; cancellation, timeout, RPC shutdown, and failed cleanup do not leak a resolver, installer, sidecar, interpreter provisioner, or unregistered runtime | Run `36223106097` passed second-session restart, fresh CPU operation, probe revalidation, protocol 3 sidecar trial/stop, and graceful shutdown on Linux, Windows, and macOS (see X2 reports); broader cancellation/timeout/failure cleanup remains | partial |
| X7 | The same manager choices and lifecycle are reachable through packaged desktop controls and the existing RPC API; the user never has to choose Python, and the desktop offers no unsupported adapter | Packaged Linux backend install/restart passes with backend `PATH` cleared; the generated `app.asar` includes `get_torch_release_options`. Electron UI interaction and packaged Windows/macOS installation remain unverified | partial |

Plan acceptance is `pending` until every required row is satisfied on its
declared native environment. Cross-compilation, Linux simulation, packaging,
and unit fixtures do not substitute for the Windows/macOS runtime claims.

## Milestones and write sets

### M0 — Contract and bounded design

- **Goal:** Define shipped target triples, capability limits, ownership,
  end-to-end gates, and authoritative official-wheel facts.
- **Write set:** This plan, execution ledger, issues, and target research report.
- **Verification:** Coding Standards MCP review; read-only architecture plan
  review; official PyTorch index evidence.
- **State:** Accepted.

### M1 — Native resolver, manager, and process lifecycle

- **Goal:** Implement the support contract through discovery, exact preview,
  isolated install, identity, selection, sidecar startup/stop, and existing
  RPC/desktop routes while preserving Linux.
- **Write sets:** `torch-server/resolve_runtime.py` and
  `torch-server/tests/test_resolve_runtime.py`;
  `rust/crates/pumas-app-manager/src/version_manager/torch_alternatives.rs`;
  `version_manager/torch_preview.rs`, `version_manager/installer.rs`,
  `version_manager/installer/torch.rs`, `version_manager/installer/torch_tests.rs`,
  `version_manager/installer/torch_upstream_contract_tests.rs`,
  `version_manager/state.rs`, `version_manager/mod.rs` for manager-owned
  cleanup shutdown; `rust/crates/pumas-core/src/process/manager.rs` with
  target-gated legacy Torch process-control tests;
  `rust/crates/pumas-app-manager/Cargo.toml` and `rust/Cargo.lock` for the
  already locked `getrandom 0.3.4` direct RNG dependency needed to preserve
  cryptographic preview-ID generation on Windows;
  `rust/crates/pumas-core/src/platform/{process.rs,paths.rs,mod.rs,managed_child.rs}`
  and `runtime_profiles/process_owner.rs` with their tests. The legacy Torch
  process-manager boundary and tests reject detached launch/stop on Windows and
  macOS; cross-platform starts use the owned runtime-profile APIs;
  `frontend/src/components/TorchInstallPreview.tsx` and its focused test,
  `frontend/src/components/TorchDesktopProjection.test.tsx`,
  `frontend/src/components/TorchInstalledVersionInspect.test.tsx`,
  `frontend/src/components/TorchRuntimeProbePanel.test.tsx`,
  `frontend/src/components/app-panels/TorchPanel.test.tsx`,
  `frontend/src/types/torch-install.ts`, `electron/src/preload.ts`, and
  `electron/tests/preload-rpc-contract.test.mjs` for validating and consuming
  the existing runtime-options response. Root owns only
  `.github/workflows/build.yml` and any new native Torch smoke script it creates,
  plus plan/inventory, shared schemas and generated files if contract changes
  prove necessary, final integration, and commit. The manager owns
  `bundledPresetAvailable` and `defaultAdapter` in its runtime-options result;
  no generated RPC contract changes are admitted unless needed to represent
  another required capability. Root owns plan/inventory, shared schemas and
  generated files if contract changes prove necessary, final integration, and
  commit.
- **Verification:** Focused Python and Rust suites; Rust format/Clippy; native
  Windows/macOS cargo and Python QA; exact release preview and lifecycle smoke.
- **Re-plan trigger:** A native target requires a contract/schema change,
  process primitive outside the existing ownership boundary, or a non-CPU
  runtime claim unsupported by official evidence.
- **State:** Implemented; v2.14.0 CPU/Core RPC install and restart passed on
  Linux, Windows, and macOS. CUDA and MPS behavior remain outside this accepted
  runtime tuple.

### M1b — Managed stable CPython

- **Goal:** Bootstrap a pinned uv helper without host Python, enumerate its
  target-native catalog, use it to discover native candidate wheel tags without
  provisioning every Python minor, then provision newest-first Python
  candidates in Pumas-owned storage, retain exact provider/interpreter identity,
  resolve dependencies against the
  observed wheel tags, and keep the user-facing flow automatic.
- **Write sets:** App-manager Python provider/lease and Torch manager/resolver
  files; Torch install UI and focused tests; Electron bridge timeout policy and
  tests; uv pin and attribution files; Linux/Windows/macOS native provisioning
  QA. The existing RPC string request already accepts `python: "auto"`, so this
  change does not require generated contract or preload changes. Root owns
  plan/inventory, native workflow composition, final review, and commit.
- **Verification:** Pinned uv artifact digest/version checks; stable catalog
  selection; managed Python 3.14 with full resolution; definite-incompatibility
  fallback to 3.13; inconclusive failures do not downgrade; clean host PATH,
  concurrency/cancellation, immutable retention, tamper, and safe cleanup tests.
- **State:** Manager/provider/resolver/UI implementation is present; native
  clean-host v2.14.0 CPU/Core provisioning and restart passed on all three
  shipped targets. All three target-specific CPython notice supersets are
  included and their provider pins, enum-arm target mapping, selected/full
  archive identity, exact legal-file lists, and full-archive hashes pass
  generation and packaging checks. Cancellation/cache/tamper cases remain open.

### M2 — Native acceptance and inventory

- **Goal:** Collect target-native resolver/install/lifecycle/packaged desktop
  evidence for managed Python and update the existing Torch inventory without
  broadening Tuldok claims.
- **Write set:** New plan evidence/report; relevant sections of
  `docs/plans/torch-diffusion-serving/reports/upstream-version-manager-progress.md`;
  execution ledger. Existing historical model reports remain unchanged.
- **Verification:** Every X1–X7 claim has an evidence environment, execution
  mode, result, and link. Reuse read-only review for any lifecycle/security
  repair.
- **State:** Active; native RPC CPU/Core acceptance is retained for all three
  targets and provider-license integration is complete. Current-branch Linux
  packages were rebuilt, passed resource/hash and backend-health checks, and
  passed Torch CPU/Core install/restart through the packaged Linux backend.
  Electron UI interaction and packaged Windows/macOS install acceptance remain
  pending.

## Constraints and re-plan triggers

- Keep a single exact platform/interpreter/wheel authority. Do not make the
  renderer or a second manually maintained table authoritative for support.
- Preserve hash-locked previews, staging/publication ordering, explicit
  selection/defaulting, cleanup draining, owned generation stop, loopback
  exposure, and typed inconclusive outcomes.
- Preview identifiers remain cryptographically random on every target. Use the
  already locked `getrandom 0.3.4` at the app-manager boundary instead of the
  Linux-only `/dev/urandom` path; RNG failure is a typed error, never a weak
  fallback.
- On Windows, admit child processes to a retained Job before they execute any
  code that could create descendants (suspended spawn + Job assignment + resume,
  or an OS-supported equivalent). Retain the Job handle through cancellation,
  process exit, descendant drain, and generation completion; do not close it
  before the owner has observed cleanup.
- If a Windows staged or unregistered published runtime remains locked after
  all owned processes have drained, move it to the manager-marked quarantine
  when the filesystem permits. If it cannot be renamed/deleted, keep it
  unregistered and unselectable, retain explicit pending-cleanup ownership,
  retry during startup and the next install/cleanup, and drain active cleanup
  work at shutdown. Never report a best-effort delete as completion.
- Never accept a wheel because its filename merely contains a platform word;
  parse and compare the wheel's complete interpreter/ABI/platform tags to the
  candidate interpreter's own tags and the target process architecture.
- Native tests must exercise paths with spaces, files held open during failed
  installs, cancellation/timeout, and simultaneous old/new sidecar generations
  where those behaviors differ by platform. Path evidence includes canonical
  root/ancestry, symlink aliases, and Windows reparse-point cases; comparisons
  use filesystem-aware path operations, not string prefixes.
- Keep FLUX.2, Nunchaku, and Tuldok image-generation claims tied to their
  existing exact evidence. Any MPS image adapter, Windows image model, or new
  model dependency needs its own product and hardware acceptance.
- Re-plan if Windows/macOS require changing persisted runtime identity,
  dependency-lock semantics, public RPC shape, or selection ownership.

## Linked artifacts

- [Execution ledger](execution-ledger.md)
- [Issues and dispositions](issues.md)
- [Official target and wheel research](reports/target-wheel-research.md)
- [Managed Python provider admission](reports/managed-python-provider.md)
- Coding Standards MCP policies applied: `topic.cross-platform`,
  `profile.language.rust.cross-platform`, `topic.architecture`,
  `profile.language.rust.security`, `topic.security.filesystem-containment`,
  `topic.contracts`, `topic.dependencies`, `workflow.planning`, and
  `topic.licensing`, `workflow.verification.platforms` from snapshot
  `snapshot:v1:588a50a0-e97e-444d-9c6b-a383baa4dfcf`.
