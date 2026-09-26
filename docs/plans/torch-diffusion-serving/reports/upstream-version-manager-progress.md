# Upstream Torch version manager progress

Date: 2026-09-26 (America/Vancouver). This is an implementation and evidence
inventory, not a completed Pumas desktop acceptance. The prior
[A1 packaged acceptance](a1-packaged-acceptance.md) applies only to its original
`v2.9.1` / CPython 3.12 / CUDA 13.0 / Linux x86_64 combination. The Tuldok image
evidence below uses a separate, exact Torch 2.10 tuple and does not qualify a
newly resolved upstream release.

## Scope handoff authorized 2026-09-24

The repository owner selected the broad release-management scope after reviewing
this inventory. Every stable upstream `vMAJOR.MINOR.PATCH` tag is discoverable
independently of Pumas qualification when GitHub pagination completes within the
20-page / 120-second request budget. A page failure or exhausted budget returns
an error without a partial list presented as complete. Installation is offered for
an exact official CPU, CUDA, or ROCm binary wheel plus fully resolved dependencies
on Linux x86_64 with an already installed CPython 3.10–3.13. This records the
original Linux-only scope and is superseded for the in-progress install flow by
the 2026-09-25 managed-CPython authorization below. The current implementation
provisions stable native CPython 3.10+ from a pinned provider and chooses the
newest candidate whose exact official wheel and complete dependencies resolve.
The current-source Torch 2.14.0 CPU/Core RPC install/restart and native QA pass
on Linux x86_64, Windows x86_64, and macOS arm64 in manual run
[36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
on runtime commit `21041697`; each target provisioned Pumas-managed CPython
3.14.7 and installed 25 hashed official artifacts. The broader support claim
remains bounded by Electron UI-driven and packaged Windows/macOS installation,
provider cancellation/tamper, and non-CPU runtime gates. CPython notices are
generated for all three desktop targets, with fail-closed provider-pin,
archive-mapping, and exact legal-file checks. Pumas does not compile Torch from
source; XPU remains outside this
provider contract. A release with no compatible wheel is discoverable but
correctly rejected for that tuple.

The fixed qualified `v2.9.1` / CUDA 13.0 / Python 3.12 bundled preset remains a
separate recipe. The current manager path provisions its Python 3.12 base
interpreter from the managed depot; the previous host-interpreter condition is
historical. Other exact combinations, including alternate `v2.9.1`
combinations, use dynamic retained previews and do not inherit the preset's
qualification. The serialized follow-up and acceptance contract live in the
[Torch upstream version management plan](../../torch-upstream-version-management/plan.md);
the current-standards coordination entry is [PRG-I17](../../current-standards-remediation-2026-09-03/issues.md).

## Cross-platform runtime expansion authorized 2026-09-24

Windows x86_64 (`x86_64-pc-windows-msvc`) and macOS arm64
(`aarch64-apple-darwin`) Torch release-manager paths are implemented in the
current candidate branch under the separate
[cross-platform runtime plan](../../torch-cross-platform-runtime-management/plan.md).
The current runtime source passed v2.14.0 CPU/Core RPC install/restart, exact
wheel, managed identity, and lifecycle checks on all three targets in manual run
[36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
at `21041697`, including the current metadata-lock follow-up. Per-target logs
and JSON evidence are retained in the linked plan. A preceding run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
at `07f7e9f6` also verified the 913-second macOS `Retry-After` case but predates
the lock follow-up. Broader support remains gated on packaged desktop and other
incomplete plan requirements. An earlier run at `032045ad` passed Linux but
stopped at Windows/macOS release-list preflight because of a harness retry cap;
the corrected harness completed the latest run. A Windows GNU test-target
compile also passed, but remains compile-only evidence. No macOS compile was
available in this Linux session. The existing
Pumas artifact targets show these target architectures are shipped; they do not
establish Torch runtime behavior.

The official Torch 2.14.0 CPU index lists Windows x64 local-version wheels
(`2.14.0+cpu`) and native macOS arm64 plain-version wheels (`2.14.0`,
`macosx_14_0_arm64`). The manager must retain upstream distribution version
separately from release and build identity. The official CUDA 13.2 index
contains Windows x64 wheel candidates; that alone does not establish driver
compatibility or CUDA image inference. macOS MPS is a runtime capability, not a
separate wheel channel. These observations are recorded in the linked plan's
[target research](../../torch-cross-platform-runtime-management/reports/target-wheel-research.md).

No new FLUX.2, Nunchaku, or Tuldok image-generation support is implied. The
current image result stays limited to its recorded Linux tuple until a separate
target/model acceptance proves otherwise.

## 2026-09-25 — Managed Python and desktop discovery repair

The install flow no longer requires host-installed Python or a Python choice.
PyTorch wheels do not declare one Python version for an entire release: each
official wheel declares its compatible CPython ABI and platform tags. Pumas
uses its pinned provider catalog to provision stable native CPython candidates
from newest to oldest, then retains the first interpreter whose exact selected
Torch wheel and complete dependency profile resolve. CPython 3.10 is the
minimum; no upper minor cap is configured. A newer candidate is skipped only
after a definite wheel or dependency incompatibility. Provider, network, and
incomplete-scan failures remain inconclusive. See the
[managed Python provider report](../../torch-cross-platform-runtime-management/reports/managed-python-provider.md).

The manager now defaults every target to Core Torch (`none`). Linux exposes
Pumas' FLUX.2 dependencies only as an explicit advanced profile, and the
qualified v2.9.1 bundled recipe is also opt-in. Windows and macOS expose Core
only until a platform-specific adapter is accepted. UI copy states that Pumas
installs a private Python runtime and does not require Python on the host.

The reported `Unknown API method: get_torch_release_options` is emitted by the
Electron `api:call` allowlist before a request reaches Rust. The registration
fix is in commit `6a726eac`, with a regression test covering allowlist admission
and payload validation. The local Linux 0.7.0 candidate contains the registration
in its Electron bundle. Its AppImage and deb pass the artifact checker and
bundled RPC health smoke. A package built before that commit still shows the
reported error; the toolbar-linked package has not been replaced or published
by this local build.

Current host verification: `pumas-app-manager` passed 179 tests and Clippy;
`pumas-rpc` passed 256 unit tests, 17 integration tests, and 2 intent tests
(10 ignored). The RPC suite needs loopback permission; it passed when rerun
outside the sandbox's socket restriction. Electron passed 12 test files
including the RPC registration regression; the Torch preview passed 14 tests
with frontend typecheck/lint; the full 98-test Python suite and Ruff passed.
Rust formatting and `git diff --check` passed. A fresh release build from
`ce9170d4` passed headless startup smoke and artifact validation; both the
AppImage and deb passed bundled RPC health smoke. The packaged Electron bundle
contains `get_torch_release_options`, and the packaged backend contains the
uv 0.12.18 provider pin.

The v2.14.0 live scan and full Core dependency preview pass through Pumas RPC in
an isolated Linux launcher root. Pinned uv 0.12.18 downloaded with its
published Linux SHA-256 and provisioned managed CPython 3.14.7. The recommended
`cu132` / `python3.14` / `none` preview resolved 44 artifacts. A CPU Core
preview resolved 25 artifacts; its retained resolution installed Torch 2.14.0,
passed installed-identity and CPU-operation checks, and was explicitly selected.
The latter run removed Python, pip, and PyPy from the backend's child `PATH` and
deleted its temporary launcher root afterward. At the time of this Linux-only
run, CUDA/device execution, sidecar lifecycle, and native Windows/macOS runtime
acceptance remained pending; later Linux lifecycle and three-platform native
acceptance are recorded below.

The live report exposed two stale compatibility assumptions. uv's actual
python-build-standalone catalog URLs use
`releases.astral.sh/github/python-build-standalone/...`, and current official
PyTorch indexes return wheel files directly under `/whl/{channel}/`; the Rust
validators now accept those exact official forms while retaining host, channel,
filename, tag, and nesting checks. The uv pin now uses published 0.12.18 and
version/hash-scoped cache paths, preventing a previous bootstrap from being
mistaken for the new pin. The missing Electron RPC allowlist registration was
already fixed in `6a726eac`; toolbar-linked release publication remains
separate from these local builds.

## 2026-09-25 — Managed Torch 2.14.0 CPU/Core lifecycle acceptance

This run supersedes the earlier statement that v2.14.0 sidecar lifecycle was
unverified. Through Pumas RPC in an isolated Linux x86_64 root, uv 0.12.18
provisioned CPython 3.14.7 and the retained CPU/Core preview resolved 25 exact
artifacts. With an empty backend child `PATH`, the installer verified Torch
2.14.0 identity and CPU operation; the installed sidecar dependencies passed;
the runtime was selected; a managed CPU profile passed startup, health, and
protocol 3; and its owned generation was stopped before graceful backend
shutdown. The acceptance verified the venv's actual base interpreter, private
depot containment, executable hash, and provider archive digest.

The retained [acceptance evidence](../../torch-cross-platform-runtime-management/reports/v2.14.0-linux-cpu-rpc-acceptance/)
contains the exact runtime record, wheel resolution, pip report, installed probe,
acceptance summary, and RPC log. At the time of this Linux-only acceptance,
Windows/macOS were covered by an opt-in native CI matrix but had not run; the
later three-platform manual acceptance is recorded below. This does not
establish CUDA/device execution, packaged desktop installation, an image
adapter, or Tuldok behavior for v2.14.0.

## 2026-09-25 — Native test coverage and desktop RPC diagnosis

The production automatic Python preview now has a platform-neutral regression
test for newest-first candidate selection, fallback only after definite wheel
or dependency incompatibility, and stopping on inconclusive resolver or
interpreter-provisioning failure. Windows and macOS provider tests cover
cancellation, timeout, descendant draining, and closed admission. The native
quality workflow runs those provider tests on all three OSes and has a guarded
Windows/macOS run for the fallback test. At the time of this entry, Linux
app-manager verification passed all 180 library tests; Windows GNU test-target
checking was compile-only, and Windows MSVC/macOS tests still needed native
runners. Later native run results are recorded below.

The reported `Unknown API method: get_torch_release_options` identifies an
older Electron bundle: the current source and local Linux candidate register
the method, but the toolbar-linked package has not been replaced. The managed
runtime path already provisions private CPython automatically; users do not
select a Python version or need Python on the host. Keep the toolbar-linked
release update, native Windows/macOS install and lifecycle acceptance, provider
license attribution, and packaged desktop Torch installation open. Do not
extend the Windows/macOS runtime support claim until their native gates pass.

The public [`v0.7.0` prerelease](https://github.com/MrScripty/Pumas-Library/releases/tag/v0.7.0)
was published on 2026-09-17, and its installer assets date from that build. The
local Linux 0.7.0 candidate was rebuilt on 2026-09-25 with the RPC allowlist
repair. The reported error is raised by Electron IPC validation before the Rust
Torch manager runs, so no Python/build selection can correct that installed
bundle. The public toolbar-linked release remains unchanged.

## Current branch behavior

- Stable upstream `vMAJOR.MINOR.PATCH` releases are discovered independently of
  Pumas qualification. Torch pagination continues until a short end-of-list page,
  with a 20-page / 120-second budget; a page error, deadline, or exhausted page
  budget fails without returning a partial list as complete. Non-Torch release
  listing restores the target base's one-page / 100-item budget. A new cache records
  completeness; an old unmarked Torch cache containing exactly 100 or 1,000
  entries is refreshed because these counts match earlier one-page and
  ten-page caps. Prereleases, nightlies, and source builds remain outside this
  scope. The v2.14.0 CPU/Core RPC install, managed-provider, restart, and
  sidecar acceptance passed on Linux x86_64, Windows x64, and macOS arm64 in
  manual run [36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097).
  This does not close packaged-desktop, non-CPU, image-adapter, or Tuldok gates;
  see the linked cross-platform plan. Installed local releases remain visible
  when upstream discovery fails.
- The qualified 2.9.1 recipe remains a fixed CUDA 13.0 / CPython 3.12 bundled
  preset. The updated manager provisions its CPython 3.12 base from the pinned
  provider catalog. Other stable releases try stable native CPython candidates
  from newest to oldest (3.10 minimum, no upper minor cap) against exact official
  CPU/CUDA/ROCm wheels and complete dependencies. The UI has no Python selector
  and shows the Pumas-provisioned interpreter after preview. Provider/network failures remain
  inconclusive and cannot trigger a downgrade. Clean-host managed CPython
  provisioning and CPU/Core installation pass without host Python on Linux,
  Windows, and macOS. One installed build/interpreter/adapter combination is
  supported per upstream tag; a different combination requires a fresh preview.
- For the accepted v2.14.0 CPU/Core runs, dynamic install choices come from a
  bounded scan of the official PyTorch
  wheel directory and exact per-channel indexes for the selected release and
  managed CPython candidate tags. A release/build/Python pair is offered only when that
  exact Torch wheel exists for the target host and interpreter tags. Partial
  scans remain inconclusive;
  they do not turn an unobserved wheel into a confirmed absence. The desktop uses
  the manager recommendation without requiring build, Python, or image-profile
  selections, and advanced settings contain only returned exact pairs. On a
  NVIDIA+Intel host, CUDA and CPU may be shown while ROCm is filtered out.
  Automatic CUDA selection requires a complete scan and an installed NVIDIA
  driver meeting the documented Linux minor-compatibility floor for the channel;
  unknown/newer channel families remain advanced. Minor compatibility can limit
  features and PTX, so device and model checks still run after installation.
  ROCm remains an advanced choice because PCI detection and an `amdgpu` binding
  alone do not establish that a particular GPU supports a particular ROCm wheel.
  An incomplete scan recommends an exact CPU wheel provisionally when present.
- The desktop asks the manager for available choices and a retained preview
  before installation. Dynamic previews show every resolved wheel URL, version,
  and SHA-256. Core runtime (`none`) is the default dependency profile. Linux
  may offer Pumas' FLUX.2 image dependencies as an explicit advanced profile;
  they are not an upstream Torch component. Neither profile implies
  qualification for image generation on every release.
  The manager retains the exact pip resolution, Python distribution source and
  version, uv pin, target, and interpreter fingerprint; installation consumes
  that retained identity without resolving again.
  Previews expire after 30 minutes and the in-memory store retains at most 32;
  expired reports are discarded when read.
  Resolver results use a typed outcome across the RPC and desktop boundary:
  unsupported, invalid-report, network-inconclusive, and generic-inconclusive
  outcomes remain distinct, including timeouts. The closed qualification value
  is validated across the generated contract. The install request contract
  accepts both `app_id`/`appId` and optional `preview_id`/`previewId` forms.
  After a definite unsupported preview, the desktop can search a bounded set of
  official Torch indexes for wheel matches across the pinned managed CPython
  catalog. These discovery results are independent of host-installed Python.
  These leads are explicitly incomplete: dependencies and adapters are unchecked,
  and choosing one starts a fresh exact preview. Nunchaku's known wheel is
  confined to the fixed 2.9.1 preset.
- Installation stages a hash-locked environment, checks the exact installed
  Torch version and build, and runs core Torch/CPU/sidecar probes before
  publication. A selected adapter import failure is recorded as partial feature
  availability without invalidating an otherwise usable Torch environment.
  Installation never selects or defaults the runtime. A failed stage is removed;
  an unregistered published directory is quarantined for a safe retry. Cleanup
  keeps at most two manager-marked quarantine directories, removes a tag's old
  quarantine after successful retry, and logs cleanup errors without blocking
  installation. The version manager tracks cleanup tasks and drains them during
  RPC shutdown; direct `VersionInstaller` users have an explicit async drain.
  The shared completion survives cancellation of a drain waiter.
- The saved probe reports concrete capabilities separately: Torch import, CPU
  tensor operation, sidecar app construction and protocol, CUDA operation,
  selected image adapter imports, and untested socket startup/image generation.
  The saved probe is available through the explicit **Inspect active Torch
  runtime** action; that choice clears when the active tag changes or the
  version manager opens. Explicit selection and serving perform lightweight identity
  checks; model serving is the socket
  startup and image execution attempt. Probe evidence is tied to its recorded
  environment and hardware context, with staleness diagnostics.
- An explicit startup trial is available after selecting an installed runtime
  and an enabled managed TorchServe profile. It checks exact Torch identity,
  launches an owned sidecar, attributes its listener, and checks health and
  protocol. The trial returns scoped socket/protocol evidence to the desktop
  and releases the selection/removal lease after launch admission. Its client
  outcomes omit internal error details. The trial does not qualify image
  generation or persist an acceptance result. A failed or cancelled trial
  stops only its admitted process generation. The
  desktop exposes a generation-conditional Stop action after a successful trial.

## 2026-09-24 release-specific choices follow-up

The reported `v2.14.0` rejection was caused by applying a global `cu134` build
choice to every release. Official wheel discovery confirmed that `v2.14.0` has
exact CPU and CUDA `cu126`, `cu130`, and `cu132` wheel candidates and no
`cu134` candidate; Python changes cannot make the absent build/release pair
exist. The manager's release-options RPC now scans official channels and
returns exact installed-interpreter matches. It filters choices by detected PCI
display vendor, ranks Python 3.12 then 3.13, 3.11, 3.10 when several exact
matches are available, and uses conservative driver policy for automatic CUDA
recommendation. NVIDIA's published minor-compatibility floors are used for
CUDA 13.x, 12.x, and 11.x; unrecognized families and ROCm are not automatically
recommended. The current execution sandbox detects GPU vendors but cannot
communicate with the NVIDIA driver, so its automatic choice is CPU where an exact
CPU wheel exists. This is not a claim about the driver's availability to the
unsandboxed desktop process.

At that time the flow selected Pumas' FLUX.2 image dependency profile by
default. The current flow defaults to core Torch (`none`) and makes FLUX.2 an
explicit advanced profile on Linux. Exact package resolution still determines
whether FLUX.2 can be installed for a given Torch release, and a passing
package preview does not prove device use, image generation, or Tuldok
compatibility. The existing real Tuldok evidence remains limited to Torch
2.10 / CUDA 13.0 / Python 3.12 / FLUX.2; no v2.14.0 image result or all-release
inference claim is added by this implementation.

## Earlier evidence retained from the previous report

The counts and direct-resolver experiments in this subsection predate the
2026-09-24 upstream manager acceptance and are historical evidence, not the
current-tree verification summary.

- Deterministic Python resolver/probe fixtures: 20 passed. Ruff checks passed.
  The fixtures include wrong Torch and torchvision builds, origin/hash rejection,
  missing wheels, network-inconclusive failures, scoped adapter failure, and a
  FastAPI route object without a `path` attribute, and bounded probe identity
  inputs that exclude the virtual environment's source tree. New fixtures check
  bounded official-wheel alternatives without treating them as full resolution.
- Rust `version_manager::` tests: 102 passed, including legacy migration,
  cancellation, orphan recovery, selection, and preview process-group cleanup.
  Focused owned-launch cancellation and trial retrial tests passed. RPC
  install-contract tests: 7 passed. Frontend suite: 693 passed; Electron
  suite: 12 passed. Frontend and Electron typechecks/build and lint passed.
  Rust format, workspace check, Clippy, and no-default-features check passed
  (the latter emitted dead-code warnings for disabled feature paths).
  The full Rust gate then failed in 66 existing `pumas-library`
  core tests outside this branch's write set (1349 passed); a representative
  failed download test passed when rerun alone. The broad gate is not green.
  These checks do not substitute for desktop acceptance.
- **Real isolated CPU tuple:** official `torch-2.10.0+cpu` CPython 3.12 wheel
  (`manylinux_2_28_x86_64`, SHA-256
  `ee40b8a4b4b2cf0670c6fd4f35a7ef23871af956fecb238fbf5da15a72650b1d`)
  resolved with 25 binary wheels from the PyTorch/PyPI indexes. The hash-locked
  set installed into an isolated `/tmp` virtual environment. The exact Torch
  import, CPU tensor multiplication, in-process Pumas sidecar health/protocol
  probe, and a real sidecar socket `/health` request passed (`protocol: 3`).
  The probe result was `core_status: passed`, `adapter_status: not selected`.
  This was a direct isolated resolver/environment exercise, not a managed
  desktop installation or image inference run.
- **Additional real resolver checks:** official `torch-2.9.0+cpu` CPython 3.12
  Linux x86_64 resolved with 25 wheels; the Torch wheel SHA-256 was
  `28f6eb31b08180a5c5e98d5bc14eef6909c9f5a1dbff9632c3e02a8773449349`.
  It was not installed. For `v2.10.0` with an unavailable selected `cu118` build,
  bounded official-index discovery returned three `cu126` Torch wheel leads
  matching installed CPython 3.12, 3.10, and 3.11. Those leads were not full
  dependency resolutions or install trials.
- The host has an NVIDIA GeForce RTX 5090 Laptop GPU with 24,463 MiB and driver
  595.84. The default execution sandbox has no `/dev/nvidia*` device nodes, so
  `nvidia-smi` fails there; a read-only unsandboxed query confirms the host GPU
  is available. The existing Pumas `2.9.1+cu130` environment reports CUDA 13.0,
  detects the RTX 5090, and completed a CUDA tensor multiplication outside the
  sandbox. This check is evidence only for that existing runtime. The default
  sandbox also cannot resolve the wheel host, although elevated network access
  resolved the real CPU tuple.
- **Real upstream CUDA and image tuple:** the current Pumas RPC binary installed
  `v2.10.0+cu130` with Python 3.12 and the FLUX.2 adapter in an isolated root,
  selected it explicitly, passed a managed sidecar startup trial, loaded the
  real FLUX.2 Klein 9B KV FP8 checkpoint, and served a 1280×720 PNG to the
  actual Tuldok browser. Display, save, automatic temporary collection import,
  model unload, generation-specific stop, and restart/retrial passed. See the
  [exact environment and image evidence](upstream-v210-cu130-flux2-e2e.md).
  Pumas controls were exercised through RPC, so this does not complete the
  required Pumas desktop UI acceptance.

## Open implementation gaps

- Build choices are candidate official indexes, not a precomputed compatibility
  matrix. Wheel alternatives are discovered after an unsupported preview, but
  they remain leads until the selected combination passes full retained preview.
- The fixed 2.9.1 preset preview highlights three direct wheels; its full
  embedded hash lock is available in the preview report and is checked during
  installation. It does not expose one exact transitive wheel URL per package
  before installation.
- The saved deep probe is scoped to installation; there is no on-demand deep
  re-probe. The standalone startup trial checks socket health and protocol but
  does not execute a model. Read-only freshness checks compare the resolved lock,
  shipped sidecar Python files, interpreter binary, full installed distribution
  set, GPU identity, and available driver metadata. A changed context marks
  the saved result stale but does not rerun the expensive model operation.

## Runtime-manager acceptance status

The superseding [Torch upstream version management plan](../../torch-upstream-version-management/plan.md)
is the current PR acceptance gate for the authorized runtime-management scope.
Its U4 criterion permits a real older-release RPC lifecycle plus the composed
desktop control test, matching the user's approved “desktop or API” scope. The
actual desktop-driven FLUX.2/Tuldok repeat below remains a separate desktop
product claim; it is not required to prove the RPC installation path.

U1–U4 were accepted for the earlier Linux x86_64 flow using a host-installed
CPython interpreter. Managed-Python provisioning supersedes that setup, so this
record is historical evidence and does not satisfy the current clean-host
acceptance gate. On Linux x86_64, the Pumas RPC flow discovered 63 stable tags,
previewed `v2.9.0` / CPU / CPython 3.12 as
25 exact wheel artifacts with URLs and SHA-256 hashes, installed it from the
retained resolution, inspected a passing core probe, and explicitly selected it.
A managed Torch profile then passed startup, health, and protocol 3 checks as
owned generation `1`; the generation-conditional stop succeeded. The host
interpreter was `/usr/bin/python3.12` on x86_64 Linux with glibc 2.39. The Torch
wheel was `2.9.0+cpu` with SHA-256
`28f6eb31b08180a5c5e98d5bc14eef6909c9f5a1dbff9632c3e02a8773449349`.
The adapter was `none`; no image generation was attempted. The probe result was
passed, non-stale, with core status passed and adapter status not selected.

The historical RPC lifecycle summary and complete 25-artifact manifest are
retained in the [acceptance evidence JSON](../../torch-upstream-version-management/reports/v2.9.0-cpu-rpc-acceptance.json).
The original isolated launcher root and temporary acceptance file were under
`/tmp/pumas-torch-upstream-e2e-20260924-accepted`. This closes PRG-I17's runtime
management acceptance for the earlier Linux/Python scope. It does not claim
clean-host managed-Python provisioning or a real interactive Pumas desktop run.

## Historical pre-U6 branch gates

These counts describe the earlier PR branch before the release-specific U6
changes below. They remain useful as the prior baseline, not as the current-tree
verification result.

- Python resolver: 18 tests passed, including Rust/Python ordered build-vocabulary
  parity; Ruff passed. The `pumas-app-manager` library suite passed 136 tests,
  including shutdown admission, manager/direct-installer drain, and cancelled-
  waiter retry coverage.
  The focused `pumas-library` configuration test passed. The focused GitHub
  module: 17 passed, including pagination limits, failure/no-partial-list,
  cache completeness, request coalescing, cancellation takeover, and stale
  completed-channel replacement. Rust format and `git diff --check` passed.
- `pumas-rpc` passed 253 unit tests, 17 integration tests, and 2 intent
  integration tests (12 tests are ignored); the install-alias export test passed
  with `export-contract`. Frontend passed 125 test files / 699 tests, typecheck,
  and lint. Electron passed all 12 test files, including install alias coverage;
  its generated contract check, 41-test contract-conformance suite, and build
  passed.
  The desktop composed projection test covers discovery failure, preview,
  install, explicit selection, startup health, and generation-owned stop.
- The original PR #3 GitHub Build run at `3688c0c` failed four gates: workspace
  Clippy type complexity, a desktop projection timing race, Ruff formatting, and
  a headless observer test ordering race. The current tree fixes each cause. The
  desktop now waits for the selected release to render; the observer test cancels
  only after the held read starts; Ruff formatting and the installer alias are
  corrected.
- Python source QA now passes Ruff check/format and all 77 tests in
  `unittest discover -s torch-server/tests`. The executor-sensitive lifecycle
  fixtures use test-owned pools (and an event-loop poll for the model-load fixture)
  while retaining real worker execution and the lease/cancellation assertions.
  Frontend passes 125 test files / 699 tests, typecheck, and lint.
- `cargo fmt`, workspace `cargo check`, and workspace Clippy pass. The
  `pumas-library --no-default-features` and `pumas-rpc --no-default-features`
  package test gates also pass. The full default-feature workspace test phase is
  not locally qualified: this sandbox run recorded 1,361 passed, 66 failed, and
  6 ignored. Representative failures are loopback binds returning `EPERM` and
  SQLite writes rejected as read-only, so this environment cannot establish the
  status of those existing core tests. GitHub Build run 274 for commit `9b76db7`
  completed successfully on 2026-09-24; all five executed jobs passed, including
  the default-feature Rust quality and no-inference headless gates.

## 2026-09-24 U6 release-specific gates

- The live official-index command
  `python3.12 -I torch-server/resolve_runtime.py --release-options --version 2.14.0 --interpreter python3.10 --interpreter python3.11 --interpreter python3.12`
  completed with
  `completeScan=true` for installed CPython 3.10, 3.11, and 3.12. Exact
  candidates were CPU, `cu126`, `cu130`, `cu132`, `rocm7.2`, and `rocm7.14`
  for each interpreter; there was no `cu134` candidate. The manager filters
  those raw upstream results against detected display vendors, so an
  NVIDIA-plus-Intel host retains CPU/CUDA and does not offer ROCm. CUDA
  auto-recommendation also requires a complete scan and a recognized NVIDIA
  driver floor. This sandbox cannot query the NVIDIA driver, so it does not
  establish which CUDA channel the unsandboxed desktop will recommend.
- Full Python suite: 84 tests passed; `ruff check torch-server` and
  `ruff format --check torch-server` passed. `pumas-app-manager`: all 149 tests
  passed, including the 14 release-options, driver-floor, incomplete-scan, and
  legacy-alternatives filtering tests. `pumas-rpc`: 256 unit tests, 17
  integration tests, and 2 intent integration tests passed; 10 integration and
  2 live-network intent tests were ignored. Rust formatting and manager Clippy
  with warnings denied passed.
- Frontend: all 126 test files and 708 tests passed; typecheck and lint passed.
  The focused preview tests cover automatic recommendation, exact-release
  advanced choices, a null recommendation, Pumas-owned FLUX.2 labeling, and
  rejection of CPU-host CUDA/ROCm or mismatched alternatives. Electron's
  generated desktop contract check passed. The independent Sol xhigh review
  found no issue in the repaired host/release filtering or retained-preview
  boundary; the Astra high final review found no remaining implementation
  finding and requested this historical/current inventory split.
- The live scan confirms wheel availability only. A direct full resolver attempt
  for `v2.14.0` / `cu132` / FLUX.2 ran for several minutes without returning a
  result and was interrupted; this establishes neither success nor a dependency
  incompatibility. A completed retained dependency preview, install, managed
  startup, and Tuldok image generation have not been established here. The
  existing actual image result remains limited to Torch 2.10 / CUDA 13.0 /
  Python 3.12 / FLUX.2.

## Separate desktop and image-serving claim

1. The real FLUX.2 GPU/Tuldok result is exercised through RPC and remains scoped
   to the exact `v2.10.0` / CUDA 13.0 / Python 3.12 / FLUX.2 tuple in the linked
   report. It does not prove the same actions through the actual Pumas desktop.
2. A real desktop-driven repeat of preview, install, selection, startup trial,
   and model load would close that separate desktop claim. It is not required by
   the upstream runtime-manager U4 gate, which was accepted through RPC plus the
   composed desktop projection test.
3. General user-defined PyTorch model plugins, all-release image generation, and
   image-adapter qualification remain out of scope.

The first serving scope is the existing Nunchaku Z-Image and FLUX.2 Klein image
models. A general user-defined PyTorch model plugin contract is a separate
decision. Merge or source presence alone does not establish Tuldok inference.

## Managed CPython provider follow-up — 2026-09-25

Torch 2.14.0 CPU/Core passed the native Linux RPC install and managed sidecar
lifecycle using Pumas-provisioned CPython 3.14.7. A fresh RPC process reopened
the same data root and reverified the exact Python identity and active profile,
a fresh CPU tensor operation through the persisted managed venv, the retained
RPC probe report, the sidecar trial/stop, and graceful shutdown; see the
[restart acceptance evidence](../../torch-cross-platform-runtime-management/reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md).
The acceptance script's cold-start release-options wait now covers managed
CPython provisioning as well as the subsequent wheel scan.
This does not expand the
existing Tuldok/image qualification beyond its recorded Torch 2.10 tuple.
The uv 0.12.18 MIT/Apache notices are included in release attribution. The
Linux, Windows, and macOS CPython 3.14.7 full-archive license collections are
retained in the
[cross-platform runtime plan](../../torch-cross-platform-runtime-management/reports/managed-python-license-collection/README.md).
The Windows archive has native selection evidence from
[run 36214227835](https://github.com/MrScripty/Pumas-Library/actions/runs/36214227835)
on commit `a0658131`: that install selected CPython 3.14.7, and its full-archive
SHA-256 matches the retained Windows full-archive manifest. The current
Windows and macOS installs also selected CPython 3.14.7; their install-artifact
identities and separate full-archive license manifests are retained in their
acceptance reports below. Windows/macOS selected-provider notices still need
release-attribution integration. Native RPC installation now passes on all
three targets; packaged desktop Torch installation remains unproven. Local v0.7.0
Linux AppImage and deb packages were rebuilt
with the updated uv notices and passed extracted-resource and bundled RPC
`/health` smoke checks; this did not update the public toolbar-linked release.

## 2026-09-25 — Exact Torch wheel and cross-platform preflight follow-up

Manual native run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
on `07f7e9f6` passed v2.14.0 CPU/Core resolution, install, fresh CPU operation,
retained-probe revalidation, second-session identity check, and sidecar
trial/stop on Linux x86_64, Windows x64, and macOS arm64. Each platform retained
25 hashed artifacts and passed graceful shutdown using managed CPython 3.14.7.
The [Linux](../../torch-cross-platform-runtime-management/reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md),
[Windows](../../torch-cross-platform-runtime-management/reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md),
and [macOS](../../torch-cross-platform-runtime-management/reports/v2.14.0-macos-cpu-rpc-restart-acceptance/README.md)
reports retain the exact wheel/dependency artifacts and managed runtime identity.
The macOS release preflight honored a 913-second Retry-After and found the
requested tag on its second attempt within the 1800-second total budget.

PR run
[36223095054](https://github.com/MrScripty/Pumas-Library/actions/runs/36223095054)
passed all required PR checks, including native QA; RPC E2E is skipped on PR
events. Provider notice integration, packaged desktop Torch installation,
CUDA/MPS execution, and v2.14.0 Tuldok/image generation remain open. The
separate v2.10 image qualification is unchanged.

## 2026-09-25 — Torch metadata and lifecycle review repairs

The current PR branch now serializes Torch metadata reads and writes across
independent manager processes with a permanent `.torch-versions.lock`. The
guard covers install/removal, explicit and default selection, validation, and
startup cleanup/normalization. A snapshot uses one metadata generation; when a
writer owns the lock, readers return the last coherent cached generation, and
startup avoids reading a transitional active-version marker. Blocking workers
retain their lock lease until writes finish even if their async caller is
canceled. Read-only Astra high and Sol xhigh reviews found no remaining
blocking issue in these concurrency, cache, startup, or Windows lock-error
paths. A failed metadata write followed by a failed active-marker rollback is
still a best-effort error case.

Local verification on this branch passed 200 `pumas-app-manager` tests and the
Rust default-member suite, all-target/all-feature Clippy with warnings denied,
Rust formatting, Windows GNU app-manager test compilation, 713 frontend tests,
48 desktop-contract tests, 179 Electron tests (one platform-dependent skip),
53 managed-Python acceptance fixtures, Ruff, Python compilation, and
release-attribution validation. PR run
[36223095054](https://github.com/MrScripty/Pumas-Library/actions/runs/36223095054)
passed all required PR checks; RPC E2E is skipped on PR events. Manual run
[36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
passed current-revision CPU/Core RPC install/restart on all three targets.
Packaged desktop Torch installation, provider-license integration, CUDA/MPS
execution, and v2.14.0 Tuldok/image generation remain unverified. X1–X7 retain
their partial/pending states until their full evidence gates pass, and the local
Linux packages have not updated the public toolbar-linked release.

## 2026-09-26 — Torch metadata lock contention follow-up

A PR review identified a user-visible race: status snapshots briefly hold the
exclusive `.torch-versions.lock`, so a selection, removal, or installation
could fail immediately with `WouldBlock` during normal UI polling. Mutations
that acquire this shared lock now retry only `WouldBlock`, asynchronously, every
20 ms for at most 500 ms. A longer-lived owner still causes a bounded failure;
other I/O errors return immediately. The change covers install, active/default
selection, and removal. Read snapshots remain nonblocking, and startup
normalization defers only expected lock contention. Existing stage and detached
worker lock leases are unchanged.

Regression coverage exercises short lock handoff, the 500 ms bound, propagation
of non-contention errors, startup error classification, and manager mutation
call sites. The current local `pumas-app-manager` library suite passes 205 tests;
workspace check and Clippy, Rust formatting, 44 Torch resolver tests, Ruff, and
`git diff --check` pass. The full local workspace test could not complete cleanly
in this environment: 1432 `pumas-library` unit tests pass with four test threads,
but the `intent_api_tests` fail while creating their local API TempDirs with
`PermissionDenied`/temporary-storage errors. Astra high and Sol xhigh read-only
reviews found no lock-lifecycle blockers. The three-platform native RPC evidence
above predates this lock follow-up, so native verification of this change remains
pending. PR run 36225003150 passed on `66ccbfe7` before this change; a new PR run
must verify the follow-up.

The Torch QA job now disables checkout credential persistence. Security review
confirmed the job has `contents: read` and that PR caches use the PR merge-ref
scope; no cache-mode change was needed ([GitHub cache scope](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)).

## 2026-09-26 — CPython release attribution and desktop RPC diagnosis

The release attribution generator now includes target-scoped CPython full-
archive license supersets for Linux x86_64 GNU, Windows x86_64 MSVC, and macOS
arm64. It derives the target set from `artifact-plan.json`, links each native
restart runtime report to its separate Python Build Standalone full-archive
manifest, preserves the selected install-only URL and uv identity separately,
and records the raw 19-file license set and `PYTHON.json` for each platform.
The generated inventory contains 371 package entries and 78 hashed inputs;
all 57 legal files are covered by the source manifests. Windows CRLF text is
preserved in the generated UTF-8 notice. Generation and packaging checks bind
each target to its exact Rust uv enum arm, pinned archive hash, and reviewed
Python Build Standalone release/flavor; they reject incomplete or duplicate
legal-file lists and malformed archive hashes. Attribution integrity, release
tests, Ruff, the Electron packaging hook, and the focused Electron RPC allowlist
regression pass. The broad local Electron suite hits `EPERM` when one test
binds its loopback HTTP server inside this sandbox; the same failure does not
affect the focused RPC or packaging tests.

The reported `Unknown API method: get_torch_release_options` is confirmed to
come from Electron's `api:call` allowlist before Rust dispatch. The public
`v0.7.0` tag lacks that method in `electron/src/rpc-method-registry.ts`; the
current branch contains it and the regression test passes. The toolbar-linked
public release is still v0.7.0, so it has not received the branch's updated
bridge. Current-branch local Linux AppImage/deb candidates include the updated
bridge and generated CPython notices, pass extracted-resource hash checks, and
start their packaged backends through `/health`. Their SHA-256 values are
`1398ef0a9da1c0aab90681d3c91674ef88c6229a84984047938e7bd6eb350acd` (AppImage)
and `468b6f7c2af00ff8785f80e5486cd5133979e875dd351b4b9f5284ddfd195043` (deb).
The packaged Linux RPC backend then passed Torch 2.14.0 CPU/Core install and
restart using managed CPython 3.14.7, the 25-hash official artifact resolution,
CPU operation, probe, protocol 3 sidecar start/stop, and graceful cleanup; see
the [packaged acceptance report](../../torch-cross-platform-runtime-management/reports/v2.14.0-linux-packaged-cpu-acceptance/README.md).
This is backend acceptance rather than GUI-driven installation. These are local
candidates and have not been uploaded to the toolbar-linked release. Packaged
Electron UI interaction, Windows/macOS packaged installation, provider
cancellation/tamper/retry, CUDA/MPS execution, and v2.14.0 Tuldok image
generation remain open.
