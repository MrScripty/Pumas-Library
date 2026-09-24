# Upstream Torch version manager progress

Date: 2026-09-23 (America/Vancouver). This is an implementation and evidence
inventory, not a completed Pumas desktop acceptance. The prior
[A1 packaged acceptance](a1-packaged-acceptance.md) applies only to its original
`v2.9.1` / CPython 3.12 / CUDA 13.0 / Linux x86_64 combination. The prior Tuldok
image evidence used a different packaged runtime and does not qualify a newly
resolved upstream release.

## Current branch behavior

- Stable upstream `vMAJOR.MINOR.PATCH` releases are discovered independently of
  Pumas qualification, across up to ten GitHub release pages. Prereleases,
  nightlies, source builds, and managed installation on platforms other than
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
  An unsupported combination and a network-inconclusive resolution have distinct
  errors. After a definite unsupported preview, the desktop can search a bounded
  set of official Torch indexes for wheel matches for installed interpreters.
  These leads are explicitly incomplete: dependencies and adapters are unchecked,
  and choosing one starts a fresh exact preview. Nunchaku's known wheel is
  confined to the fixed 2.9.1 preset.
- Installation stages a hash-locked environment, checks the exact installed
  Torch version and build, and runs core Torch/CPU/sidecar probes before
  publication. A selected adapter import failure is recorded as partial feature
  availability without invalidating an otherwise usable Torch environment.
  Installation never selects or defaults the runtime. A failed stage is removed;
  an unregistered published directory is quarantined for a safe retry.
- The saved probe reports concrete capabilities separately: Torch import, CPU
  tensor operation, sidecar app construction and protocol, CUDA operation,
  selected image adapter imports, and untested socket startup/image generation.
  The desktop can inspect installed runtime results. Explicit selection and
  serving perform lightweight identity checks; model serving is the socket
  startup and image execution attempt. Probe evidence is tied to its recorded
  environment and hardware context, with staleness diagnostics.
- An explicit startup trial is available after selecting an installed runtime
  and an enabled managed TorchServe profile. It checks exact Torch identity,
  launches an owned sidecar, attributes its listener, and checks health and
  protocol. The trial returns scoped socket/protocol evidence to the desktop;
  it does not qualify image generation or persist an acceptance result. A
  failed or cancelled trial stops only its admitted process generation. The
  desktop exposes a generation-conditional Stop action after a successful trial.

## Checks and real configuration

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

## Acceptance still required before a PR is accepted

1. Through the actual desktop, discover a non-2.9.1 upstream release, review
   the resolved artifacts, install it, inspect results, explicitly select it,
   start it, and exercise basic use. Cover an older release and unavailable
   Python/build combinations with visible actionable alternatives.
2. Repeat the successful FLUX.2 GPU/Tuldok flow through the actual Pumas
   desktop controls for preview, install, selection, startup trial, and model
   load. The RPC-driven real-image path and its exact tuple are recorded in the
   linked report. Check that a failed optional adapter remains scoped to that
   feature in the real desktop.
3. In the real desktop, exercise cancellation, failed trial, switching back to
   the qualified runtime, restart, and preservation of active/default/installed
   state. Recheck probe freshness across dependency, sidecar, driver, and
   hardware changes. Keep expensive image tests separate from activation checks.
4. Resolve the program inventory's
   [PRG-I17 deployment gate](../../current-standards-remediation-2026-09-03/issues.md):
   managed Torch stays non-shipped until an accepted source/dependency/interpreter/
   sidecar tuple completes isolated install, launch, health, request, and shutdown
   evidence. This branch does not claim that release disposition from fixtures
   or the isolated CPU experiment.

The first serving scope is the existing Nunchaku Z-Image and FLUX.2 Klein image
models. A general user-defined PyTorch model plugin contract is a separate
decision. Merge or source presence alone does not establish Tuldok inference.
