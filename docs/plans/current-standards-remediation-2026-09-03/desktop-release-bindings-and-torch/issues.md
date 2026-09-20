# Issues: Desktop, Release, Bindings, and Torch

**Plan:** [plan.md](plan.md)

## Current State

- The product/release owner accepted the Milestone 0 decision bundle. Its
  former unavailable issues are resolved and implementation/evidence work is
  routed to the named downstream milestones.
- `2b081fba` accepts the selected generated desktop contract, bundled preload,
  and coordinated root/bootstrap renderer integration. Historical integration
  holds below are superseded by the current dispositions, not extended into
  all-route, stream, Torch, or release acceptance. `2b9553a0` separately accepts
  the Rust-owned classification correction on Linux.

## Active Issues

### DRBT-I10 — Launcher-root authority failures collapse into discovery

- Severity: High; blocks DRBT-A3 and safe desktop startup.
- Evidence: persisted absence, read failure, malformed JSON/shape, and an
  unusable configured root all become `null`; the resolver then selects a
  portable, discovered, packaged-default, or development-default path. Explicit
  environment and argument roots also bypass the existing-root validator.
- Relationship: a startup that appears healthy can display or mutate a library
  other than the persisted owner; an invalid explicit path can be created by
  backend startup and become new authority.
- Owner/boundary: Milestone 2 launcher-root Module; the resolver is its public
  Interface and filesystem inspection is hidden implementation.
- Disposition: the focused state/provenance slice was accepted and integrated
  as `1964760d`.
  Invalid, unavailable, and present-but-empty explicit authority no longer
  reaches discovery or backend startup. Authoritative selection accepts only
  an exact root or its two declared chooser forms; ancestor walking is
  discovery-only. Backend initialization is observed before window creation can
  delay failure consumption, and typed recovery diagnostics stay path-free.
  Atomic replacement was implemented by the next focused slice.
- Verification: real temporary-filesystem public resolver tests with a valid
  discovery decoy plus invalid/unavailable persisted and invalid explicit
  authorities; marker-file rejection; and deterministic injected I/O failure
  across all authoritative sources. Include malformed-path/oversized-name
  invalid classification, arbitrary-descendant rejection, and an immediate
  recovery rejection while window creation is delayed. Required-real target
  filesystem evidence remains pending.
- Atomic-persistence status: accepted and integrated as `767e71f0`. Direct
  truncation is replaced by an exclusive adjacent temp, synchronized close,
  namespace replacement, and parent synchronization with explicit pre/post-
  publication failure states. Local Linux old-or-new evidence cannot stand in
  for power-loss or required-real Windows/macOS filesystem evidence.
- Recovery/bootstrap integration status: accepted in `2b081fba`. The corrected
  single-flight selection owner preserves explicit-authority policy and locks
  ambiguous/published terminals. The renderer consumes decoded outcomes;
  root-scoped cached rows carry no action authority. The synchronous bootstrap
  returns precomputed state, and presentation retains the accepted causal
  marker barrier. Earlier uncommitted/absent-consumer holds are superseded.
- Sandboxed-preload status: accepted for the selected integration. The build
  bundles generated standalone validators and the preload, retaining Electron
  as the external runtime dependency without weakening sandbox preferences.
  Current-source freshness, actual-producer conformance, the enabled real
  sandbox oracle, and built-app Linux verification are recorded in the parent
  ledger. No Windows/macOS or packaged-runtime acceptance is inferred.
- Revisit trigger: required-real target filesystem/runtime evidence for
  DRBT-A3; independent stream ownership proceeds under DRBT-I11.

### DRBT-I11 — Desktop stream transitions lack one terminal owner

- Severity: High for DRBT-A4 lifecycle acceptance.
- Evidence: five bridge stream owners coexist with process-global renderer
  counters; the model-library stream has no renderer subscription owner;
  window close omits model-download/model-library cleanup; most preload
  subscription and all unsubscription promises are unobserved; request destroy
  is not joined to terminal socket close; and a stale request callback can
  overwrite newer stream state.
- Relationship: renderer close, backend restart, transport failure, and app
  shutdown can produce silent failure, duplicate transitions, retained work,
  or a false-clean terminal result.
- Owner/boundary: Milestone 2 main-process stream Module and preload Interface;
  Milestone 1 remains the event-decoder authority.
- Disposition: next bounded Linux-capable investigation is the model-library
  stream's per-renderer custody and terminal outcomes. Revalidate the recorded
  inventory against current source, then admit its exact implementation write
  set and real Electron oracle. Do not re-own Rust event semantics or infer
  that the selected generated RPC contract closes streamed-event decoding.
- Verification: Electron lifecycle harness with duplicate subscribe/unsubscribe,
  renderer destruction, late transport callbacks, delivery failure, backend
  restart, and shutdown oracles; required-real Electron evidence remains
  pending.
- Revisit trigger: persisted-authority integration is accepted; proceed with
  the plan's single next investigation, then the admitted stream write set.

### DRBT-I7 — No accepted Torch runtime/model/device tuple

- Severity: High; blocks DRBT-A5 and any shipped Torch promise.
- Evidence: the current Linux x64/Python 3.12.3 environment has no Torch,
  Transformers, safetensors, FastAPI, Uvicorn, or Accelerate installation. The
  current 18-test suite installs fakes and never traverses a real ASGI,
  production loader, tokenizer, model generation, or device path.
- Relationship: the inference work owner, thread/process suitability,
  responsiveness budget, dependency resolution, shutdown behavior, and real
  usage semantics cannot be accepted without this tuple.
- Owner/boundary: Milestone 3 owns the Torch request/work Module and evidence;
  dependency and release owners must accept the resolved production stack.
- Disposition: keep Torch non-shipped. Narrow the independently provable
  request contract now, but do not select an executor, worker process, model
  architecture, or device from fake evidence.
- Verification: resolve one production stack and execute real ASGI requests,
  production loading/generation, control responsiveness, overload,
  cancellation, disconnect, failure, and bounded shutdown on its accepted
  local model/device fixture.
- Revisit trigger: an accepted resolved environment and local offline fixture.

**2026-09-20 current disposition:** The evidence above remains a dated finding,
not a description of the current image stack. Later image-serving work established
real Linux/GPU/runtime evidence within its recorded scope. The
[Torch Image Provider Contract Correction](../../torch-image-provider-contract/plan.md)
now owns the reusable generation lifetime plus image-specific compatibility,
result, and projection work and its exact-candidate handoff. Milestone 3 still
owns text shapes/semantics, usage, responsiveness, and shutdown; TIPC acceptance
cannot by itself satisfy DRBT-A5 or erase this issue's historical evidence
boundary.

### DRBT-I8 — Managed Torch installation does not install the sidecar

- Severity: High; current GUI-managed install/launch cannot establish a valid
  Pumas Torch deployment.
- Evidence: `launcher-data/plugins/torch.json` selects GitHub repository
  `pytorch/pytorch`; the generic Python installer downloads and installs that
  source tree, while `BinaryLaunchConfig::torch` expects Pumas `serve.py` in the
  installed version directory. It also hard-codes POSIX `venv/bin/python`, and
  plugin Python 3.10 conflicts with repository `.python-version` 3.12.3.
- Relationship: blocks DRBT-A5's required-real process/deployment path and the
  tuple required to move Torch out of non-shipped state.
- Owner/boundary: Rust process/version-management and plugin manifest owners;
  this plan records the dependency but does not mutate those concurrent write
  sets without a serialized cross-plan handoff.
- Disposition: `invalid`, not an alternate supported installer. Define one
  sidecar source/version/dependency identity and cross-platform interpreter
  path before enabling managed deployment.
- Verification: install from the selected source into an isolated root, prove
  exact dependency identity, launch the installed `serve.py`, traverse health
  and one accepted real request, and stop with no owned work remaining.
- Revisit trigger: Rust and plugin owners admit the deployment repair.

### DRBT-I9 — Shared OpenAI-compatibility claims exceed the accepted subset

- Severity: Medium now; High if Torch becomes shipped.
- Evidence: source, the accepted request-contract slice, and bounded official-
  reference comparison support only a narrow OpenAI-shaped text subset.
  `torch-server/README.md` now states that local boundary accurately, but
  `launcher-data/plugins/torch.json` and `frontend/src/config/apps.ts` still
  advertise an unqualified “OpenAI-compatible API”; they are outside this
  slice's write ownership.
- Relationship: leaves DRBT-A9 pending and can mislead users even after the
  local request decoder rejects unsupported behavior.
- Owner/boundary: Torch boundary docs, frontend product copy, and plugin
  manifest owners must project the same accepted capability state.
- Disposition: narrow or remove the shared claim after the focused source
  contract is reviewed; do not add compatibility shims for absent consumers.
- Verification: search all advertised/registered surfaces and compare each
  statement to the executable accepted subset and real evidence.
- Revisit trigger: serialized frontend/plugin ownership or an accepted broader
  real contract.

### DRBT-I6 — Windows and macOS launcher evidence unavailable locally

- Severity: High for release promotion; no effect on bounded Linux
  implementation.
- Evidence: the current execution environment is Linux x64. Source review or
  Linux process behavior cannot prove Windows console/process-tree semantics or
  macOS runtime behavior.
- Relationship: leaves DRBT-A6 pending and blocks promotion of the accepted
  Windows x64 and macOS arm64 artifacts until required-real evidence exists.
- Owner/boundary: Milestone 4 owns the shared launcher contract and per-OS
  Adapters; Milestone 6 supplies accepted target runners and release gates.
- Disposition: implement one portable launcher contract with platform-specific
  mechanisms and record non-local target outcomes as `unavailable`, not passed.
- Verification: execute the same wrapper/platform/process integration suite on
  accepted Windows x64 and macOS arm64 runners and observe no orphaned child or
  false success.
- Revisit trigger: target CI execution or an accepted target-matrix change.

## Resolved Issues

### DRBT-I1 — Release channel, target matrix, and promotion authority unavailable

- Severity: High.
- Evidence: six public GitHub releases are all marked prerelease; current CI
  packages Linux x64, Windows x64, and macOS arm64, but only an unpackaged Linux
  launcher smoke exists. SemVer `0.x`, workflow shape, and prior publication do
  not select a channel or support promise.
- Relationship: blocks Milestone 0, `PRG-A6`, and `DRBT-A8`/`DRBT-A9`; fixes the
  required environments for `PRG-A3`, `PRG-A4`, `DRBT-A3`, `DRBT-A4`, and
  `DRBT-A6`.
- Owner/boundary: product/release owner at the release artifact-plan seam.
- Disposition: re-plan/accept-now. Decide the proposed GitHub preview channel,
  three desktop target tuples, manual promotion authority, and support/retention
  policy before encoding a release plan.
- Verification: reviewed decision followed by required-real exact-package
  evidence for every accepted tuple.
- Revisit trigger: explicit owner acceptance or a new real channel/target
  consumer.
- Resolution: accepted GitHub preview-only channel, the three current desktop
  tuples, and repository-maintainer manual promotion. Evidence remains owned by
  Milestones 2, 4, and 6.

### DRBT-I2 — Host binding disposition unavailable

- Severity: High.
- Evidence: public releases contain Python, Kotlin, Swift, Ruby, and C# bundles,
  but no real host consumer or release host suite was found. The C# smoke proves
  only a Linux debug native `version()` call and record construction. Rustler
  has no core Pumas interface or BEAM host evidence, and Go has only a source
  comment.
- Relationship: blocks Milestone 0, `PRG-A6`, and `DRBT-A7`/`DRBT-A9`.
- Owner/boundary: product owner selects the host promise; Rust owns semantic
  Adapter source and this plan owns host cohort/release projection.
- Disposition: re-plan/accept-now. Proposed removal of all binding ZIPs from
  releases, Rustler/Go removal, and either internal-experimental or removed
  UniFFI machinery.
- Verification: absence across advertised/registered/released surfaces, or a
  new accepted tuple with exact-cohort real-host evidence.
- Revisit trigger: explicit owner decision or a named real host consumer.
- Resolution: remove all host-binding releases plus UniFFI, Rustler/Elixir, and
  Go surfaces. Rust owns source-seam removal; this plan owns release/script/doc
  projections.

### DRBT-I3 — Published Rust `.crate` role unavailable

- Severity: Medium.
- Evidence: Pantograph is the real direct Rust consumer and pins an immutable
  Git revision; no consumer of the GitHub `.crate` asset or package registry was
  found.
- Relationship: blocks exact release-artifact closure in Milestone 0/6 and
  `PRG-A6`.
- Owner/boundary: product/release owner at Rust source distribution.
- Disposition: proposed removal from GitHub releases while preserving the
  producer-owned source contract and Pantograph exact-revision evidence.
- Verification: release-plan absence plus Pantograph's locked exact-revision
  consumer build/tests.
- Revisit trigger: accepted direct `.crate` or registry consumer.
- Resolution: remove the GitHub `.crate` artifact while preserving Pantograph's
  exact Git-revision source contract.

### DRBT-I4 — Torch deployment target unavailable

- Severity: High.
- Evidence: Torch has lower-bounded production requirements and source-only
  development instructions; it is not present in current desktop, Rust, or
  binding release artifacts. No supported Python/platform/device tuple or
  deployment channel exists.
- Relationship: fixes the environment for Milestone 3, `PRG-A4`, `DRBT-A5`,
  and any later `PRG-A6` release claim.
- Owner/boundary: product owner selects deployment; this plan owns the Torch
  request/work Module and evidence.
- Disposition: proposed non-shipped status until Milestone 3 proves the smallest
  accepted real stack.
- Verification: real resolved ASGI/model/device evidence for the accepted tuple,
  or absence from every release promise.
- Revisit trigger: explicit owner disposition or a real deployment consumer.
- Resolution: Torch remains non-shipped until Milestone 3 proves an accepted
  real tuple.

### DRBT-I5 — Third-party license/notice acceptance authority unavailable

- Severity: High.
- Evidence: current releases lack an accepted final dependency notice/SBOM
  closure; a local historical Debian package exposes Electron/Chromium license
  files but not a separately identifiable Pumas MIT license or reviewed final
  third-party inventory.
- Relationship: blocks `DRBT-A8`, `DRBT-A9`, and `PRG-A6` release acceptance.
- Owner/boundary: designated licensing/release authority.
- Disposition: block promotion until the owner and per-artifact obligation
  review are named.
- Verification: final extracted-byte license/notice/SBOM inspection with
  recorded reviewer authority.
- Revisit trigger: named authority and accepted artifact plan.
- Resolution: repository maintainer owns final third-party notice review and
  acceptance.
