# Plan: Cross-Platform Torch Runtime Management

**Plan status:** `Active`

**Owner:** Pumas runtime integration

**Authorization:** The repository owner authorized Windows and macOS Torch
runtime management on 2026-09-24, retaining the existing Linux support.

**Current acceptance:** `pending` — the Windows and macOS implementation is not
yet accepted. Native install, RPC lifecycle, and packaged desktop evidence is
still required.

**Current phase:** M2 native acceptance. M1 implementation and repair are
complete in the candidate branch. Linux regressions and Windows GNU test-target
compilation pass; macOS test-target compilation and Windows/macOS native test
execution have not run in this Linux session.

**Blockers:** No known code blocker remains from the architecture or independent
reviews. Native Windows and macOS acceptance evidence is unavailable here:
real wheel preview/install/identity, RPC health and generation-owned stop, and
packaged desktop smoke must run on their target systems. The new native QA
matrix is committed as a gate but has not run for this candidate.

**Next slice:** On native Windows x64 and macOS arm64 runners, run the candidate's
Torch QA matrix, then exercise exact official CPU-wheel preview/install/identity,
RPC start/health/owned-stop, and packaged desktop smoke. Record each runner,
environment, result, and artifact before changing M2 acceptance.

## Objective and scope

Extend the existing Torch release manager so users can discover a stable
upstream Torch release, let Pumas select an exact official wheel matching the
host and an installed supported Python, preview the complete hash-locked
dependencies, install and inspect it, explicitly select it, and start/stop its
managed sidecar through the existing RPC and desktop flows.

“Any release” continues to mean an upstream stable tag for which the exact host,
Python, official wheel, and complete dependency resolution exist. It does not
promise a wheel for every release or every supported host. Python provisioning,
source builds, and unqualified adapters are outside this objective.

The current accepted Linux contract remains the regression baseline in the
[upstream version-management plan](../torch-upstream-version-management/plan.md)
and its [runtime inventory](../torch-diffusion-serving/reports/upstream-version-manager-progress.md).
Windows/macOS become supported only when this plan's native acceptance passes.
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
[`artifact-plan.json`](../../../scripts/release/artifact-plan.json#L58). Use an
already installed CPython 3.10–3.13 interpreter whose own supported wheel tags
match the Pumas process architecture, selected official Torch wheel, and all
resolved dependencies. Discover and rank eligible interpreters automatically;
keep interpreter/build overrides advanced and constrained to exact matches.
Do not provision Python or infer compatibility from a display name.

The release manager retains the exact upstream wheel URL, hash, distribution
version, build identity, interpreter fingerprint, and dependency lock. Validate
the observed installed Torch identity against that retained resolution. The
Torch distribution version is not always `release+build`: for example, the
official Torch 2.14.0 CPU index publishes Windows wheels named
`2.14.0+cpu`, while its macOS arm64 wheels are named `2.14.0` and tagged
`macosx_14_0_arm64`. Preserve the release/build identity separately from the
upstream distribution version. Do not manufacture a `+cpu` local version for
the macOS artifact.

Exact package availability is determined from official indexes and the
installed interpreter's `packaging.tags.sys_tags()`, not a Pumas release
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
required CPython 3.12 interpreter is present; the desktop must use that result
instead of the release tag alone. Windows and macOS use the retained dynamic
preview and wheel lock.

On Windows and macOS, Core Torch runtime (`none`) is the default dependency
profile and the only offered adapter until a platform-specific image adapter
has its own accepted plan. Linux retains its current default/profile behavior.
The desktop displays this manager-provided default rather than guessing from
the OS. This enables portable Torch install/select/start without suggesting
FLUX.2 or Nunchaku inference has been qualified there.

## Ownership and design

- `pumas-app-manager` continues to own release choices, automatic interpreter
  selection, retained previews, staged installation, identity, selection, and
  cleanup admission.
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
- RPC and generated contracts remain unchanged unless a required capability
  fact cannot be expressed by existing release-options, preview, install,
  selection, and startup results. Any contract change is serial and owns its
  generated artifacts.
- The desktop remains a presentation of manager-returned exact choices. It must
  not infer OS support, invent builds, select a mismatched interpreter, or
  interpret MPS as an image adapter. It uses manager-provided
  `bundledPresetAvailable` and `defaultAdapter` capabilities rather than
  enabling the Linux bundled preset merely because the selected tag is v2.9.1.

### Composed-design review

**Applicability:** `applicable` — the work changes target selection and
process-lifecycle seams across resolver, manager, and platform runtime owners.

1. **Independent concerns:** upstream wheel identity and interpreter tags
   (Python resolver); host hardware facts and recommendations (manager); staged
   version identity/publication (installer); native process supervision
   (platform lifecycle); choice presentation (existing RPC/desktop).
2. **Interleavings:** release/build/distribution version; target/architecture/
   Python ABI and wheel tags; hardware/driver evidence; wheel URLs and hashes;
   staged directory and cleanup; process generation and cancellation. These
   remain explicit fields/outcomes at the manager boundaries.
3. **Caller knowledge:** RPC/desktop clients consume manager choices, retained
   preview identifiers, and lifecycle status. They do not parse platform paths,
   wheel filenames, drivers, or process IDs to decide support.
4. **Representative changes:** a new supported host must update target facts,
   exact wheel fixtures/resolution, manager host/interpreter detection, native
   lifecycle evidence, and its native CI acceptance. A new upstream wheel tag
   remains local to official-tag resolution and its tests. A new image adapter
   does not change this runtime-management contract.
5. **Stable interfaces:** exact artifact and host facts cross the resolver to
   manager; the retained resolution crosses preview to installer; process
   owners expose generation-scoped lifecycle results. OS path strings,
   display text, or ambient command lookup do not act as artifact identity.
6. **Independent verification:** resolver tags can be fixture-tested; host
   detection and venv paths are tested natively; process-group/job ownership,
   cleanup, and sidecar lifecycle require native target execution. Integration
   remains in the manager/API and desktop composition.
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
8. **Necessary complexity:** exact wheel-tag compatibility and native
   process-tree ownership are required complexity, contained in resolver and
   process-lifecycle owners. No new public API or package dependency is admitted
   unless implementation evidence shows the current owners cannot express the
   contract.

## Acceptance claims

| ID | Observable criterion | Evidence required | Status |
| --- | --- | --- | --- |
| X1 | Linux behavior and existing accepted v2.9.0 install/v2.10 Tuldok evidence remain accurately scoped and pass relevant regression gates | Existing Linux manager/resolver/RPC/frontend suites; retained historical evidence references | pending |
| X2 | Windows x64 and macOS arm64 discovery returns only exact official wheels whose tags match an installed supported native CPython; CPU/MPS/CUDA choices follow this contract | Native wheel fixtures, wrong-OS/architecture/interpreter rejection, live official-index scan on both native runners | pending |
| X3 | Preview retains the exact distribution version, release/build, official wheel URL/hash, interpreter identity, and complete dependencies; install stages and verifies that exact identity before publication | Resolver/Rust contract fixtures plus real official CPU wheel preview, install, identity and cleanup on Windows/macOS | pending |
| X4 | Installed versions can be inspected, explicitly selected, started with health/protocol checks, and stopped by their owned generation; cancellation, timeout, RPC shutdown, and failed cleanup do not leak a resolver, installer, sidecar, or unregistered runtime | Native lifecycle tests and real RPC start/health/owned-stop integration on Windows/macOS; Linux regression | pending |
| X5 | The same manager choices and lifecycle are reachable through packaged desktop controls and the existing RPC API; the desktop consumes manager-provided preset/default-adapter capability and offers no unsupported adapter | Preload boundary validation, composed desktop tests, native RPC integration, packaged desktop smoke on all declared targets | pending |

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
  process primitive outside the existing ownership boundary, Python version
  expansion, or a non-CPU runtime claim unsupported by official evidence.
- **State:** Implemented; target-native acceptance pending.

### M2 — Native acceptance and inventory

- **Goal:** Collect target-native resolver/install/lifecycle/packaged desktop
  evidence, update the existing Torch inventory, and report any model-support
  limit without broadening Tuldok claims.
- **Write set:** New plan evidence/report; relevant sections of
  `docs/plans/torch-diffusion-serving/reports/upstream-version-manager-progress.md`;
  execution ledger. Existing historical model reports remain unchanged.
- **Verification:** Every X1–X5 claim has an evidence environment, execution
  mode, result, and link. Reuse read-only review for any lifecycle/security
  repair.
- **State:** Active; native acceptance pending.

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
- Coding Standards MCP policies applied: `topic.cross-platform`,
  `profile.language.rust.cross-platform`, `topic.architecture`,
  `profile.language.rust.security`, `topic.security.filesystem-containment`,
  `topic.contracts`, `topic.dependencies`, `workflow.planning`, and
  `workflow.verification.platforms` from snapshot
  `snapshot:v1:305ccfba-eed5-46b7-bfa2-02868fe804b3`.
