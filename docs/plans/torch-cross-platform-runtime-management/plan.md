# Plan: Cross-Platform Torch Runtime Management

**Plan status:** `Active`

**Owner:** Pumas runtime integration

**Authorization:** The repository owner authorized Windows and macOS Torch
runtime management on 2026-09-24, retaining the existing Linux support. On
2026-09-25, the owner authorized Pumas-managed CPython provisioning and removed
the requirement for a host-installed Python interpreter or a user Python choice.

**Current acceptance:** Manual native run
[36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
on runtime commit `21041697` passed the current v2.14.0 CPU/Core RPC installation
and restart path on Linux x86_64, Windows x86_64, and macOS arm64. Each target
provisioned Pumas-managed CPython 3.14.7 with pinned uv 0.12.18, installed 25
hashed official artifacts, persisted the interpreter across restart, returned
14 from a fresh CPU tensor operation, passed the resolver probe and protocol 3
sidecar trial/stop, shut down gracefully, and cleaned its temporary root. See
[Linux](reports/v2.14.0-linux-current-source-cpu-rpc-restart-acceptance/README.md),
[Windows](reports/v2.14.0-windows-current-source-cpu-rpc-restart-acceptance/README.md),
and [macOS](reports/v2.14.0-macos-current-source-cpu-rpc-restart-acceptance/README.md).
All active jobs in that manual workflow passed. The prior run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
also exercised macOS's 913-second Retry-After behavior, but predates the metadata
lock follow-up. Acceptance remains limited to CPU/Core RPC installation and
restart plus the Linux AppImage UI install recorded below. CUDA/device
execution, packaged Windows/macOS installation, and v2.14.0 Tuldok image
generation remain unverified; existing Tuldok image evidence remains scoped to
its recorded Torch 2.10 tuple.

**Direct-install policy (2026-09-26):** The owner rejected a multi-minute
compatibility check before installation. The normal Core Torch flow admits the
install from a local selection token, without refetching GitHub release
metadata, scanning every wheel channel, or running `pip --dry-run`. The visible,
cancellable install task provisions managed Python and makes a real binary-only
pip install into private staging. Linux automatic CUDA selection tries at most
four driver-compatible channels, then CPU; Windows and macOS start with CPU.
Only conclusive missing-wheel/dependency outcomes permit a bounded candidate
retry. Network, integrity, and ambiguous errors stop the actual attempt. The
staged report, installed package identities, wheel hashes, and exact file
manifest are checked before staged Python is run or the runtime is published.
Any optional release-options request now returns immediately as unchecked; the
installer resolves availability during the user's install attempt. The older
AppImage install report below predates this direct-install change and does not
accept it. A rebuilt local package now passes artifact and extracted-backend
health checks; manual packaged installation and native direct-install
acceptance remain pending.

Earlier local Linux v0.7.0 AppImage and deb candidates were rebuilt with the
generated CPython notices and Torch install feedback/readability repair, before
the direct-install change.
Artifact naming and extracted resource hashes pass; both extracted backends
start and pass `/health`. The packaged Electron archive contains the
`get_torch_release_options` bridge method. The last pre-direct-install local
AppImage SHA-256 is
`34a5ffc88f5f42aa03bd9ed72ead37e5ca385838037247b74186a67d9d128aaf`; the deb
SHA-256 is
`fea54b4694d7ddb77a797327a9e15d0f0e66e0cf4f2d38fc9169feb4a4811386`.
The prior successful Linux v2.14.0 cu132 UI install remains recorded with its
then-current AppImage hash in the [packaged UI acceptance report](reports/v2.14.0-linux-appimage-cu132-ui-acceptance/README.md).
The report also records the interaction/readability check on the candidate
built before the direct-install change; that follow-up did not claim another
install. These are local build outputs; they do not replace the toolbar-linked
public v0.7.0 assets (published 2026-09-17), and this evidence predates the
direct-install path described below. The generated attribution inventory
includes all three CPython
full-archive notice supersets and verifies 371 package entries, 78 hashed
inputs, and 57 retained legal texts against target-specific evidence.

The earlier local Linux package candidate recorded in the
[packaged Linux backend acceptance](reports/v2.14.0-linux-packaged-cpu-acceptance/README.md)
passed the full Torch 2.14.0 CPU/Core install/restart path with backend `PATH`
cleared. It provisioned managed CPython 3.14.7, installed 25 hashed official
artifacts, performed a CPU operation returning 14 after restart, and passed the
probe plus protocol 3 sidecar start/stop. The latest AppImage's separate
UI-driven installation is recorded above; packaged Windows/macOS install
acceptance remains unverified.

**Current phase:** The three-target CPU/Core RPC install and two-session
sidecar lifecycle pass on current runtime source `21041697`. That run verifies
the bounded metadata-lock follow-up. The repair closes the reviewed
cross-process metadata races by guarding Torch metadata read/modify/write with
`.torch-versions.lock`, refreshing state under that lease, and retaining cloned
leases through detached writes. Startup defers validation/normalization when
the lock is busy and avoids reading a possibly transitional active marker. Torch
status reads now use one coherent cached generation. The repair also addresses
failed child-custody drain, best-effort cleanup recovery, bounded Electron Torch
RPC requests, and fixed
preset access while upstream release discovery is pending. Selection, default
selection, removal, and installation now retry brief `WouldBlock` contention
for at most 500 ms; other I/O errors propagate immediately.

Follow-up verification passes 205 `pumas-app-manager` tests, Rust formatting,
workspace check and Clippy gates, 44 Torch resolver tests, Ruff, and
`git diff --check`. The full local workspace test did not complete cleanly:
`pumas-library` unit tests passed when rerun with four threads (1432 passed, 6
ignored), but the separate `intent_api_tests` hit local filesystem permission
and temporary-storage errors. This does not identify a failure in the changed
Torch paths. Manual workflow
[36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
on `21041697` passed its workflow/release contracts, frontend/desktop contracts,
headless checks, Rust quality, all three native Torch QA jobs, and all three
native RPC E2E jobs. PR head `57e5fb7f` adds only the acceptance evidence and
documentation after that runtime-source run. Astra high and Sol xhigh completed
read-only reviews with no lock-lifecycle blockers. Earlier manual run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
also passed the three-target RPC install/restart and exercised the macOS long
Retry-After path. Windows GNU compilation remains compile-only evidence but is
supplemented by native Windows MSVC acceptance.

**Direct-install follow-up:** The current source replaces the pre-install
Torch artifact scan and release-metadata refetch with a local selection token
and a real staged pip install. Linux `build=auto` now makes up to four ordered
driver-compatible CUDA install attempts before CPU fallback. Automatic Python
fallback is limited to conclusive wheel/dependency failures. pip report and
staged-file provenance checks run before package files move into the runtime.
The existing Linux AppImage install report above uses the earlier path, so it
does not accept this change. Focused source checks cover local admission,
installer retries, staging integrity, cancellation, and progress. The rebuilt
local package passes artifact and extracted-backend health checks, but a
packaged direct-install run remains pending. The fixed bundled adapter preset
continues to use its separate retained-lock resolution path.

**Remaining gates:** Exercise the direct-install flow through the local Linux
package, then run packaged Windows/macOS Torch installation and provider
cancellation/tamper/retry acceptance. CUDA/device acceleration and v2.14.0
image/Tuldok generation are untested. Local AppImage/deb candidates do not
update the public toolbar-linked package.

**Next slice:** Verify the rebuilt Linux direct-install candidate, then run
packaged install acceptance on Windows and macOS and close provider
cancellation/tamper gates. Keep Tuldok qualification limited to its recorded
exact image-generation tuple; publish a new toolbar-linked release only after
the PR is merged and a new version is tagged.

## Objective and scope

Extend the existing Torch release manager so users can discover a stable
upstream Torch release, let Pumas provision a compatible stable CPython, install
the selected package profile, inspect it, explicitly select it, and start/stop
its managed sidecar through the existing RPC and desktop flows. The host does
not need Python on `PATH`, and the desktop does not require a Python selection.

The normal install path must not run a full wheel scan, remote release metadata
fetch, or `pip --dry-run` as a gate. Local selection and host probing are
bounded to a few seconds; if a quick probe is unavailable or inconclusive,
installation still proceeds with the safe automatic choice. The UI admits the
install immediately and enters a visible, cancellable task. Pumas provisions its
managed Python and runs a real binary-only `pip install --report` into an
unpublished staging environment. pip's dependency resolution and any wheel
downloads happen inside that actual install attempt. Pumas validates the
resulting report's artifact URLs, hashes, package names and versions, interpreter
identity, and exact staged file manifest before it starts staged Python or
publishes the runtime. A failed attempt is removed from staging and reported to
the user. Automatic Python/build retries are bounded and allowed only after a
conclusive unsupported-wheel/dependency result; network, integrity, timeout,
and ambiguous failures stop without downgrading.

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
versions come from the pinned Python distribution catalog in the pinned uv
release; do not maintain a Pumas upper-version allowlist or treat
prerelease/free-threaded tags as standard CPython. CPython 3.10 is Pumas' runtime
minimum; there is no configured upper minor cap. Rank candidates numerically
from newest to oldest, validate each installed interpreter's observed target
and `sys_tags()`, and attempt the actual staged install. A proven missing wheel
or dependency incompatibility can advance to the next candidate. Network
failure, timeout, provisioning failure, or integrity failure stops the install
without downgrading Python. The user never has to install or choose Python.

The app-manager downloads and bootstraps uv 0.12.18 for the shipped Linux x86_64 GNU, Windows
x86_64 MSVC, and macOS arm64 targets from versioned official archives. It
verifies each archive against the exact SHA-256 in the provider report before
extracting or executing it. uv's Python distribution catalog is embedded and
frozen per uv release; its exact binary pin therefore pins that catalog too.
The manager enumerates that catalog on the native host, filters to stable
standard CPython candidates, and sorts them newest-first. The normal install
path provisions candidates in bounded newest-first order and invokes pip's
binary-only install into a fresh unpublished target for each attempt. An exact
absent wheel or definite dependency incompatibility permits trying the next
candidate. Network, provider, timeout, integrity, or ambiguous outcomes stop the
attempt. The quick selection token does not install Python or inspect package
indexes. uv
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

Exact package availability is determined by the install attempt against official
indexes and the provisioned interpreter's `packaging.tags.sys_tags()`, not a
Pumas Torch release allowlist or a guessed OS floor. A selection preview is
local and immediate; package availability and dependency compatibility are
learned during the staged install. On macOS,
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
  provisioning, selection tokens, staged Torch installation, identity,
  selection, and cleanup admission. Python artifacts live in an
  immutable Pumas-managed depot; uv's disposable download cache is separate.
- The app-manager invokes only the exact-hash-pinned uv binary that it has
  downloaded into its private bootstrap directory and verified. It clears
  ambient uv configuration, forces manager-owned Python, uses private
  install/cache directories, disables PATH shims and (on Windows) registry
  writes, and never runs a shell installer or administrator operation.
- A selected Python distribution's source catalog identity, exact CPython
  version, native target/build, download URL, uv/catalog identity, executable
  fingerprint, and observed `sys_tags()` remain attached to its retained
  install attempt and installed runtime record. uv verifies the Python archive with the
  digest from its embedded catalog.
  Existing installations keep their original base interpreter; managed-runtime
  cleanup may not delete an interpreter referenced by a preview, install, or
  installed runtime. Automatic pruning is out of scope until reference-safe
  removal is implemented.
- `torch-server/resolve_runtime.py` owns binary-only pip resolution and install
  into the unpublished target, wheel-tag matching, official wheel provenance,
  and hash reporting. Its protocol carries the upstream distribution version
  and exact build as separate validated facts. Its report is checked before
  staged Python is started or the runtime is published.
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
3. **Caller knowledge:** RPC/desktop clients consume manager choices, local
   selection identifiers, interpreter identity, and lifecycle status. They do not
   parse platform paths, wheel filenames, drivers, or process IDs to decide
   support.
4. **Representative changes:** a new supported host must update target facts,
   exact wheel fixtures/resolution, manager host/interpreter detection, native
   lifecycle evidence, provisioned Python target, and its native CI acceptance.
   A new upstream wheel tag remains local to official-tag resolution and its
   tests. A new image adapter does not change this runtime-management contract.
5. **Stable interfaces:** exact artifact and host facts cross the resolver to
   manager; the post-install report and verified staged-file manifest cross
   the resolver to installer before publication; process owners expose
   generation-scoped lifecycle results. OS path strings,
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

## Network activity monitoring

- Use one platform-neutral activity registry for transfer observations, keyed
  by operation and transfer IDs. Producers report cumulative bytes, optional
  expected size/source, direction, state, measurement basis, and coverage; the
  registry calculates rates from monotonic samples and never adds overlapping
  payload and OS wire counters.
- The first adapter is Torch package installation. The existing installer
  download callback and the pip wheel worker feed the same registry. The
  desktop progress view shows the measured payload rate and a copyable direct
  URL for any HTTP(S) host when the URL has no embedded credentials, query, or
  fragment. Other sources still report byte rate without exposing a potentially
  secret-bearing URL. Copyable source status is a display-safety check, not
  provenance approval; official package origin and hash validation remain
  separate install requirements.
- Keep OS process monitoring optional. It may add partial wire-byte observations
  under the same operation/transfer model, but permission-dependent OS counters
  cannot identify a repository or artifact and must not be presented as the
  exact package transfer. No Linux-only or administrator-required provider is
  assumed by the shared API.
- Current coverage is partial: the generic `DownloadManager` and Hugging Face
  stream paths still need adapters, and no OS process sampler is implemented.
  Do not claim that all Pumas network traffic is visible until those producer
  paths and platform permissions are tested.

## Acceptance claims

| ID | Observable criterion | Evidence required | Status |
| --- | --- | --- | --- |
| X1 | Linux behavior and existing accepted v2.9.0 install/v2.10 Tuldok evidence remain accurately scoped and pass relevant regression gates | Existing Linux manager/resolver/RPC/frontend suites; retained historical evidence references | pending |
| X2 | Each shipped target provisions a private stable standard CPython 3.10 or newer without a host Python dependency or global PATH/registry changes; artifact integrity, target identity, cache separation, cancellation, and retention are verified | Linux, Windows, and macOS v2.14.0 RPC installs provisioned CPython 3.14.7 and matched persisted interpreter identity across sessions ([Linux](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md), [Windows](reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md), [macOS](reports/v2.14.0-macos-cpu-rpc-restart-acceptance/README.md)). Artifact/source hashes are retained and all three full-archive license supersets are included in generated attribution; cancellation and cache separation remain | partial |
| X3 | Candidate versions come from the pinned stable provider catalog; the manager tries highest to lowest through actual staged installs and never treats network/provisioning failure as a reason to downgrade | CPython 3.14 preference when fully resolved; definite 3.14 dependency incompatibility falls to 3.13; prerelease/free-threaded/wrong-target and pre-3.10 artifact rejection; direct install retry and failure classification tests | partial |
| X4 | Windows x64 and macOS arm64 installation selects only official binary wheels matching the managed interpreter's native tags; CPU is automatic, CUDA remains an explicit Windows choice, and MPS is checked as a runtime capability | Native wheel fixtures and wrong-target rejection passed; prior manual RPC run `36223106097` resolved exact official CPU wheel tags on Linux, Windows, and macOS arm64 through the preview flow. Direct-install native acceptance, CUDA execution, and macOS MPS acceleration remain untested. | partial |
| X5 | The actual install attempt retains the upstream distribution version, build, official wheel URL/hash, Python provider artifact identity, interpreter fingerprint, and complete dependency report; validates installed package names/versions and the exact staged file manifest before publication | Historical run `36223106097` resolved 25 hashed CPU/Core artifacts through the previous preview path on all targets; current direct-install resolver and Rust provenance tests cover post-install checks. Native packaged direct-install acceptance is pending. | partial |
| X6 | Installed versions can be inspected, explicitly selected, started with health/protocol checks, and stopped by their owned generation; cancellation, timeout, RPC shutdown, and failed cleanup do not leak a resolver, installer, sidecar, interpreter provisioner, or unregistered runtime | Run `36223106097` passed second-session restart, fresh CPU operation, probe revalidation, protocol 3 sidecar trial/stop, and graceful shutdown on Linux, Windows, and macOS (see X2 reports); broader cancellation/timeout/failure cleanup remains | partial |
| X7 | The same manager choices and lifecycle are reachable through packaged desktop controls and the existing RPC API; the user never has to choose Python, and the desktop offers no unsupported adapter | In the historical Linux v0.7.0 AppImage UI install, the first sampled state at 9 seconds showed the current phase in the header, indeterminate bars, Cancel, and readable controls under the previous package-resolution flow. A separate interaction/readability follow-up observed immediate artifact-check status but did not install again. The direct-install local AppImage/deb are now built and pass extracted-resource/backend health checks; source/UI tests cover local selection and visible install state. Manual packaged install remains pending. Packaged Windows/macOS installation remains unverified. | partial |
| X8 | Network activity from each integrated producer uses the shared operation/transfer registry with explicit payload-versus-wire basis, coverage, lifecycle, and source; UI rates are derived from cumulative samples and source URLs are safely copyable | Torch pip and installer progress use the registry; arbitrary HTTP(S) hosts and direct paths without credentials or query/fragment are covered by Rust and Python tests. Generic `DownloadManager` and Hugging Face adapters, OS telemetry providers, and cross-platform permission behavior remain open. | partial |

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

- **Goal:** Implement the support contract through stable release discovery,
  local selection, direct staged install, post-install identity verification,
  selection, sidecar startup/stop, and existing RPC/desktop routes while
  preserving Linux.
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
- **State:** Cross-platform runtime implementation is present; v2.14.0 CPU/Core
  RPC install and restart passed on Linux, Windows, and macOS through the prior
  preview flow. The direct-install source path is implemented and under focused
  verification; packaged acceptance of that path is pending. CUDA and MPS
  behavior remain outside the accepted runtime tuple.

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
  packages passed resource/hash and backend-health checks, the packaged Linux
  backend passed Torch CPU/Core install/restart, and the v0.7.0 AppImage passed
  the Torch UI install-progress flow. Packaged Windows/macOS install acceptance
  remains pending.

## Constraints and re-plan triggers

- Keep a single exact platform/interpreter/wheel authority. Do not make the
  renderer or a second manually maintained table authoritative for support.
- Preserve isolated staging/publication ordering, official-source and
  post-install hash/file verification, explicit selection/defaulting, cleanup
  draining, owned generation stop, loopback exposure, and typed install errors.
  Dynamic Torch installs no longer require a pre-install hash-locked preview;
  the generated requirements file records the completed install for provenance
  and repeatability, but is not the former `pip --require-hashes` gate. The
  fixed bundled preset remains a separate lock-based path.
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
