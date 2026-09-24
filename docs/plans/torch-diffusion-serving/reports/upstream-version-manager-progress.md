# Upstream Torch version manager progress

Date: 2026-09-24 (America/Vancouver). This is an implementation and evidence
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
on Linux x86_64 with an already installed CPython 3.10–3.13. Pumas does not
provision Python or compile Torch from source; XPU and other operating systems
remain outside this provider contract. A release with no compatible wheel for a
supported interpreter is discoverable but correctly rejected for that tuple.

The fixed qualified `v2.9.1` / CUDA 13.0 / Python 3.12 bundled preset remains a
separate recipe. Other exact combinations, including alternate `v2.9.1`
combinations, use dynamic retained previews and do not inherit the preset's
qualification. The serialized follow-up and acceptance contract live in the
[Torch upstream version management plan](../../torch-upstream-version-management/plan.md);
the current-standards coordination entry is [PRG-I17](../../current-standards-remediation-2026-09-03/issues.md).

## Current branch behavior

- Stable upstream `vMAJOR.MINOR.PATCH` releases are discovered independently of
  Pumas qualification. Torch pagination continues until a short end-of-list page,
  with a 20-page / 120-second budget; a page error, deadline, or exhausted page
  budget fails without returning a partial list as complete. Non-Torch release
  listing restores the target base's one-page / 100-item budget. A new cache records
  completeness; an old unmarked Torch cache containing exactly 100 or 1,000
  entries is refreshed because these counts match earlier one-page and
  ten-page caps. Prereleases, nightlies, source builds, and managed installation
  on platforms other than
  Linux x86_64 remain outside this scope. Installed local releases remain visible
  when upstream discovery fails.
- The qualified 2.9.1 recipe remains a fixed CUDA 13.0 / CPython 3.12 bundled
  preset. Other stable releases can use an installed CPython 3.10–3.13
  interpreter and an official CPU/CUDA/ROCm wheel index. Pumas does not provision
  interpreters. One installed build/interpreter/adapter combination is supported
  per upstream tag; the UI must disclose the installed combination before a
  different combination is attempted.
- The desktop asks the manager for available choices and a retained preview
  before installation. Dynamic previews show every resolved wheel URL, version,
  and SHA-256, with `none` or FLUX.2 adapter dependencies selected separately.
  The manager retains the exact pip resolution and a fingerprint of the chosen
  interpreter; installation consumes that retained lock without resolving again.
  Previews expire after 30 minutes and the in-memory store retains at most 32;
  expired reports are discarded when read.
  Resolver results use a typed outcome across the RPC and desktop boundary:
  unsupported, invalid-report, network-inconclusive, and generic-inconclusive
  outcomes remain distinct, including timeouts. The closed qualification value
  is validated across the generated contract. The install request contract
  accepts both `app_id`/`appId` and optional `preview_id`/`previewId` forms.
  After a definite unsupported preview, the desktop can search a bounded set of
  official Torch indexes for
  wheel matches for installed interpreters.
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

U1–U4 are accepted for the scope in the plan. On Linux x86_64, the current Pumas
RPC flow discovered 63 stable tags, previewed `v2.9.0` / CPU / CPython 3.12 as
25 exact wheel artifacts with URLs and SHA-256 hashes, installed it from the
retained resolution, inspected a passing core probe, and explicitly selected it.
A managed Torch profile then passed startup, health, and protocol 3 checks as
owned generation `1`; the generation-conditional stop succeeded. The host
interpreter was `/usr/bin/python3.12` on x86_64 Linux with glibc 2.39. The Torch
wheel was `2.9.0+cpu` with SHA-256
`28f6eb31b08180a5c5e98d5bc14eef6909c9f5a1dbff9632c3e02a8773449349`.
The adapter was `none`; no image generation was attempted. The probe result was
passed, non-stale, with core status passed and adapter status not selected.

The accepted RPC lifecycle summary and complete 25-artifact manifest are now
retained in the [acceptance evidence JSON](../../torch-upstream-version-management/reports/v2.9.0-cpu-rpc-acceptance.json).
The original isolated launcher root and temporary acceptance file were under
`/tmp/pumas-torch-upstream-e2e-20260924-accepted`. This closes PRG-I17's runtime
management acceptance for the authorized Linux/Python scope. It does not claim a
real interactive Pumas desktop run.

## 2026-09-24 current-tree gates

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
- The historical full workspace Rust test gate remains non-green: 66 existing
  `pumas-library` core failures were previously recorded alongside 1,349 passes,
  and are outside this change's write set. Do not treat that aggregate result as
  acceptance evidence for this implementation.

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
