# Execution Ledger: Desktop, Release, Bindings, and Torch

**Plan:** [plan.md](plan.md)

## Baseline

- Code: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`
- Standards: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`
- Audit code: `a33c8c0efa7cd8783c7deeac9e608db205290d43`
- Audit standards: `52b096ded9c53afd439a3cf0efc4cc85252da570`

## Current State

- The explicit `start` operation was accepted on 2026-09-03 and moved the plan
  from `Planned` to `Active`.
- Milestone 0 is `Accepted`; the report and two canonical matrices agree and are
  visible versioned inputs.
- Milestone 1 is `Active`: selected catalog/FTS/download/ticket-recovery
  generation and consumption are accepted in `2b081fba`; unselected Legacy
  operations, transport, and streamed-event gates remain pending.
- Milestone 4 is locally green but remains `Active`/pending until required-real
  Windows x64 and macOS arm64 launcher evidence exists.
- Milestone 3 has one accepted request-contract slice but remains
  `Active`/pending; runtime scheduling and deployment are blocked on a real
  tuple and fixture.
- Milestone 2's selected bundled preload, root bootstrap, renderer recovery,
  and Linux presentation integration is accepted in `2b081fba`. Stream
  lifecycle and required-real cross-target evidence remain pending.
- Next slice: DRBT-I11 model-library stream ownership investigation on Linux,
  with exact implementation admission and terminal-outcome oracle before
  source changes. No whole milestone is accepted by the selected checkpoint.

## 2026-09-20 — Delegate the image-provider correction

- Planning-only reconciliation delegated the image lifetime, live compatibility,
  private result evolution, and gateway projection subset to the focused
  [Torch Image Provider Contract Correction](../../torch-image-provider-contract/plan.md).
- Milestone 3 retains broader text semantics, usage accounting, health/control
  responsiveness, overload, and shutdown. TIPC evidence is necessary for the
  image subset but cannot satisfy whole DRBT-A5.
- The current DRBT-I11 next slice and all unrelated milestone priorities remain
  unchanged. No source, tests, runtime, dependencies, installation, or release
  state changed.

### Generation-wide lifetime clarification

- The user clarified that elapsed time never makes any admitted generation fail.
  The focused plan now owns one reusable generation transport/lifecycle seam and
  TIPC-12 covers a representative non-image generation route.
- Milestone 3 consumes `TIPC-GEN-01` for text generation while retaining text
  shapes, sampling, usage, responsiveness, and shutdown. It no longer selects a
  text-generation deadline. Launcher/process deadlines are unrelated and remain
  unchanged.

## Entries

### 2026-09-03 — Start Milestone 0 authority investigation

- Operation: `start`.
- Prior/current plan state: `Planned` -> `Active`.
- Development decision: `investigate`.
- Coherent slice: determine the consumer, channel, artifact, host/target,
  version, promotion, dependency, licensing, and evidence facts needed before
  release or binding automation can be valid.
- Current exact write set:
  - `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md`
  - `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/execution-ledger.md`
  - `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/issues.md`
  - `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/reports/release-and-host-contract-decision.md`
- Shared-file handoff: `scripts/release/artifact-plan.json`,
  `bindings/support-matrix.json`, package scripts/manifests, generated output,
  CI, and durable shared docs remain held for serial ownership by the program
  coordinator.

### 2026-09-03 — Milestone 0 investigation reached the decision gate

- Inventory method: current repository/configuration review, historical package
  inspection, authenticated GitHub release/API inventory, and public-code search
  for concrete consumers.
- Result: the bounded investigation stopping condition passed. Every current
  released/generated output and public consumer claim has an observed consumer
  classification or proposed conservative disposition.
- Real consumers found: independent desktop release users, Pantograph through an
  exact Cargo Git revision, and Pixapillars transitively through Pantograph.
- No real direct host consumer was found for Pumas Python, Kotlin, Swift, Ruby,
  C#, Elixir/Erlang, or Go surfaces. Torch is source-only and not currently
  shipped.
- Evidence/report:
  [release-and-host-contract-decision.md](reports/release-and-host-contract-decision.md).
- Outcome: `unavailable`. Product/release-owner acceptance is required for the
  proposed preview channel, desktop tuples, `.crate`/binding removal,
  UniFFI/Rustler/Go dispositions, Torch non-shipped state, promotion owner, and
  licensing authority.
- Program impact: `PRG-A6` is blocked directly; `PRG-A3` and `PRG-A4` cannot
  fix their complete required-real environment matrices until these decisions
  are accepted.
- No source, tooling, configuration, shared manifest, CI, package, generated
  artifact, or durable shared-document file was changed.

### 2026-09-03 — Product/release contract accepted

- Operation: explicit user `continue` after the external decision resolved the
  `Blocked` state; plan and Milestone 0 returned to `Active`.
- Accepted channel: GitHub `preview` only, with manual promotion by a repository
  maintainer after all required evidence passes.
- Accepted desktop tuples: Linux x64 AppImage/Debian, Windows x64
  NSIS/portable, and macOS arm64 DMG. Build-only evidence cannot promote them.
- Removed release roles: standalone `.crate` and all Python, Kotlin, Swift,
  Ruby, and C# binding ZIPs.
- Removed surfaces: UniFFI, Rustler/Elixir, and Go; no hypothetical binding seam
  remains without a real consumer.
- Torch disposition: non-shipped source capability until Milestone 3 proves a
  real runtime/platform/device tuple.
- Version/consumer contract: one lockstep Pumas product version; Pantograph
  consumes the Rust interface by immutable Git revision.
- Licensing authority: repository maintainer reviews and accepts final
  third-party notice evidence.
- Next write set is the two Milestone 0 canonical matrix files plus this plan's
  plan, ledger, issues, and decision report, under granted serial ownership.
- Review reconciliation: release promotion explicitly requires program security
  `PRG-A1`, governance `PRG-A7`, all other `PRG-A*` claims, and all
  `DRBT-A*` claims. The versioned top-level third-party notice is the reviewed
  authority and each desktop artifact embeds byte-identical content under a
  stable internal filename.

### 2026-09-03 — Re-plan canonical binding-matrix visibility

- Trigger: `git check-ignore -v bindings/support-matrix.json` proved that the
  intended canonical matrix was excluded by `.gitignore`'s `bindings/*` rule;
  `git status --short --untracked-files=all` showed only the release artifact
  plan.
- Classification: systemic authority/integration defect. An ignored canonical
  file cannot own or project the accepted binding disposition.
- Composition result: unchanged. The same two accepted matrices remain the
  Interfaces; no additional registry, parser, or Adapter was introduced.
- Re-plan: `.gitignore` was admitted under explicit serial ownership solely to
  add `!bindings/support-matrix.json` while retaining every generated-output
  ignore. The Milestone 0 allowed write set now records it.
- Intended oracle: Git reports both canonical matrices as untracked change-set
  inputs, `git check-ignore` reports the support matrix as not ignored, and both
  documents parse and cross-reference the removed binding release role.

### 2026-09-03 — Accept Milestone 0

- Files: `.gitignore`, `scripts/release/artifact-plan.json`,
  `bindings/support-matrix.json`, and this plan's authority/evidence artifacts.
- Cross-review: program integration review accepted the preview/manual
  promotion contract, three target/five artifact closure, seven removed host
  surfaces, zero accepted host tuples, `.crate`/binding/Torch exclusions, all
  `DRBT-A1`–`DRBT-A9` and `PRG-A1`–`PRG-A7` gates, and notice identity rule.
- Visibility oracle: `git check-ignore` reports the support matrix is not
  ignored; `git status --short --untracked-files=all` shows both matrices.
- Contract oracle: Node JSON parsing plus relationship checks prove reciprocal
  references, unique/closed target and artifact IDs, complete removed-host
  inventory, acceptance-claim inclusion, and byte-identical notice embedding.
- Supporting checks: current plan structure and scoped diff checks pass.
- Result: Milestone 0 `Accepted`. Shared ownership of `.gitignore` and both
  matrices is released; later changes require a new serial handoff.

### 2026-09-03 — Start Milestone 4 launcher outcome slice

- Operation: `continue`; plan remains `Active`, Milestone 4 moves from
  `Planned` to `Active`, and Milestone 1 is explicitly `Blocked` rather than
  projecting an incomplete producer contract.
- Development decision: `build`, after bounded inspection confirmed three
  related failures at one existing ownership seam: Bash has a wrapper-specific
  release path, unknown platforms default to Linux, and bounded commands have
  no forced-termination deadline.
- Coherent slice: make both wrappers delegate every action to the shared Node
  launcher, make the platform factory closed over accepted OS values, and make
  launcher-owned child trees reach one observed completion after graceful then
  forced termination through an OS Adapter.
- Exact initial write set:
  - `launcher.sh`
  - `launcher.ps1`
  - `scripts/launcher/actions.mjs`
  - `scripts/launcher/commands.mjs`
  - `scripts/launcher/contract.mjs`
  - `scripts/launcher/platform-service.mjs`
  - `scripts/launcher/platform-linux.mjs`
  - `scripts/launcher/platform-macos.mjs`
  - `scripts/launcher/platform-windows.mjs`
  - `scripts/launcher/actions.test.mjs`
  - `scripts/launcher/commands.test.mjs`
  - `scripts/launcher/wrappers.test.mjs`
  - this plan, ledger, issues, and
    `reports/launcher-platform-evidence.md`
- Shared-file handoff: root serialized `README.md` and
  `docs/DEVELOPMENT.md` for launcher-only updates, but their mutation is held
  until the focused Linux source contract passes. `package.json` is excluded.
- Initial oracle order: first demonstrate the wrapper fast path, unknown-to-
  Linux fallback, and unbounded ignored-termination behavior; then implement.
- Evidence boundary: Linux x64 runs locally as required-real evidence. Windows
  x64 and macOS arm64 remain `unavailable` until the same suite executes on
  accepted real target runners; no result is inferred across OS boundaries.
- Deep-module review: keep validation, timers, and one-terminal-result policy
  inside `commands.mjs`; platform modules are mechanism-only Adapters for
  process-tree termination. No parallel action registry or generic process
  framework is admitted.

### 2026-09-03 — Re-plan typed platform failure at the CLI boundary

- Trigger: the initial unsupported-platform oracle showed that
  `createPlatformService()` is invoked before `cli.mjs` enters its
  `LauncherError` boundary. Fixing only the factory would emit an unhandled
  stack rather than the declared typed diagnostic and exit code.
- Re-plan: add the already-Milestone-4-owned `scripts/launcher/cli.mjs` to the
  current exact write set and move platform composition inside the existing
  error boundary. This is not a new Adapter or registry.
- Claim impact: DRBT-A6 only. No governance claim matrix or schedule changes.

### 2026-09-03 — Re-plan shared POSIX and bounded Windows mechanisms

- Trigger: focused cross-review found identical Linux/macOS process-group
  signal implementations and an unbounded Windows `taskkill.exe` helper nested
  inside the otherwise bounded command lifecycle.
- Re-plan: add `scripts/launcher/platform-posix-process.mjs` to the Milestone 4
  and current write sets, move the identical POSIX mechanism there, and give
  the Windows helper its own execution and forced-close observation deadlines
  inside the command's force window.
- Oracle correction: replace the loaded-runner-sensitive `<900ms` assertion
  with a controlled child self-exit marker that must remain absent after forced
  termination.
- Composition result: one process policy owner (`commands.mjs`), one shared
  POSIX mechanism Adapter, and one Windows mechanism Adapter. No target-specific
  behavior is hidden by duplication and no generic process framework is added.
- Claim impact: DRBT-A6 only. Windows/macOS required-real outcomes remain
  `unavailable` until target execution.

### 2026-09-03 — Reach focused Linux M4 source/test boundary

- Source result: Bash and PowerShell are policy-free delegates with the same
  dependency exit; the platform factory is closed over `linux`, `darwin`, and
  `win32`; the CLI catches unsupported-platform construction; bounded smoke
  commands own a per-platform tree through one max/grace/force/close policy.
- Design correction: Linux and macOS now share the single
  `platform-posix-process.mjs` process-group mechanism. Windows explicitly
  reports graceful tree termination unavailable, escalates to `/t /f`, and
  bounds plus force-observes the `taskkill.exe` helper inside the outer force
  window.
- Stable diagnostics: command arguments and absolute command paths are absent
  from launcher-generated child failure messages; spawn/termination failures
  expose stable codes rather than raw dependency text.
- Outcome-marker oracle: the force test proves the controlled child's
  self-exit marker remains absent, replacing the original wall-clock margin.
- Local real evidence: the Linux process test terminates a real SIGTERM-
  resistant parent and descendant process group; the native Bash wrapper
  returns exit 0 for help and exit 2 for invalid usage.
- Focused/full oracle: `npm run test:launcher` passes 41 tests outside the
  restricted sandbox required for nested Bash execution. `bash -n launcher.sh`
  and the scoped whitespace diff check pass.
- Composed ownership oracle: a controlled child closes before a deferred
  termination helper; the public command result remains pending until the
  helper completes. An ineffective Adapter produces the typed incomplete-
  cleanup result around the declared max/grace/force boundary and well before
  the fixture's five-second self-exit, after which the test explicitly removes
  the fixture.
- Windows helper seam: controlled spawn tests cover observed exit 0, stable
  nonzero mapping, helper timeout, forced helper kill, and observed helper
  close. The outer runner joins that Adapter outcome before settling.
- Cross-target boundary: Windows helper unit evidence is green, but it is not a
  substitute for real Windows process semantics. Windows x64 and macOS arm64
  remain `unavailable`; Milestone 4 and DRBT-A6 remain `Active`/`pending`.

### 2026-09-03 — Start Milestone 3 Torch investigation

- Operation: `continue`; plan remains `Active` and Milestone 3 moves from
  `Planned` to `Active` while Milestone 4 retains its pending cross-target
  evidence state.
- Development decision: `investigate`. The current compatibility claim,
  accepted fields, ignored values, usage accounting, actual consumers,
  dependency tuple, model/device fixture, and async work owner are facts needed
  before a safe implementation shape can be selected.
- Bounded investigation: inventory the public ASGI request/response/stream
  Interface, every accepted-but-ignored input, usage behavior, task/thread/
  process ownership, control-route schedulability, production and development
  dependency facts, model/device support, shutdown behavior, and reachable
  consumers. Compare only current claimed compatibility to official OpenAI
  primary documentation.
- Initial exact write set:
  - this plan, ledger, issues, and `reports/torch-runtime-evidence.md`
- Held files: all `torch-server/**` source/tests, both requirements files,
  `.github/workflows/build.yml`, `RELEASING.md`, `docs/SECURITY.md`, root
  `README.md`, and root package files remain read-only until investigation
  selects a coherent slice and any shared ownership is serialized.
- Stopping condition: every reachable Torch consumer and every accepted field,
  value, output, error, usage, lifecycle, dependency, runtime, platform, and
  device claim has a supported, rejected, or typed `unavailable` disposition;
  otherwise the implementation slice remains blocked.
- Skill note: the research workflow's background delegation could not start
  because all five worker slots are occupied by the standards program. The
  platform owner performs the same primary-source comparison directly.

### 2026-09-03 — Complete M3 investigation and admit the first source slice

- Investigation result: the stopping condition passed for current facts.
  `reports/torch-runtime-evidence.md` inventories all registered routes,
  request fields and ignored values, response/stream/usage behavior, task and
  resource ownership, dependency/runtime/device evidence, and reachable
  consumers. Unknown runtime claims remain explicit `unavailable` outcomes.
- Consumer decision: the only concrete first-party consumers use `/health`
  and `/api/*` control routes. No Pumas, Pantograph, Pixapillars, or puma-bot
  path was found that targets the sidecar's port or consumes its chat/text
  completion contract. The general OpenAI compatibility promise therefore has
  no retention authority.
- External authority: current official OpenAI API documentation confirms that
  chat messages, legacy prompt forms, stop behavior, streaming usage, finish
  reasons, and request ranges are broader and more precise than this server.
  The report links the exact primary sources; local similarity is not treated
  as conformance.
- Runtime result: this Linux x64/Python 3.12.3 environment lacks Torch,
  Transformers, safetensors, FastAPI, Uvicorn, and Accelerate. The 13 passing
  tests install local dependency fakes and prove only their focused control
  logic. No production model/device tuple, ASGI path, inference result, or
  responsiveness budget is accepted.
- Cross-plan deployment blocker: the managed Torch installer downloads the
  upstream `pytorch/pytorch` source release, while launch expects Pumas
  `serve.py` and a POSIX `venv/bin/python`; the configured Python version is
  3.10 while the repository pins 3.12.3. Rust/process/plugin owners must repair
  that deployment composition before it can prove an accepted tuple.
- Development decision: `implement` one reversible contract-narrowing slice.
  No real inference consumer requires retention of silently ignored or
  unschedulable behavior, and validation/deletion can be proved without
  pretending the fake environment is a production runtime.
- Exact source/test write set:
  - `torch-server/openai_api.py`
  - `torch-server/tests/test_validation_and_app.py`
  - this plan, ledger, issues, and `reports/torch-runtime-evidence.md`
- Slice contract: retain the existing request-model Interface as the one
  inbound decoder; close it over exact known fields, text-only supported roles,
  bounded inputs, implemented sampling values, and positive bounded token
  counts. Treat non-null `stop` and `stream: true` as well-formed but
  unsupported, then delete the unreachable stream implementation.
- First red oracle: construct the public Pydantic request models with one valid
  request and one independently changed invalid/unsupported field per case.
  Before implementation, unknown fields, ignored roles, empty/unbounded input,
  invalid ranges, non-null stop, and streaming are all accepted.
- Held files: `torch-server/README.md` remains held until the source/test
  boundary is reviewed. Requirements, shared CI, release/security/root docs,
  root package files, Rust, frontend, and plugin configuration remain excluded
  and require their owning handoff.
- Claim boundary: this slice supplies focused contract evidence toward
  DRBT-A5. It cannot satisfy DRBT-A5's required-real system claim, select an
  inference worker/thread/process owner, or close runtime/deployment issues.

### 2026-09-03 — Accept the first M3 request-contract slice

- Operation: `continue`; Milestone 3 and DRBT-A5 remain `Active`/`pending`.
  Root accepted this boundary only as incremental request-validation evidence,
  not as real ASGI, inference, device, scheduling, usage, or deployment proof.
- Initial red oracle: 14 of 15 focused public-model tests failed before the
  decoder was closed. Unknown fields, unsupported roles, empty and over-budget
  inputs, invalid sampling/token values, non-null `stop`, and `stream: true`
  crossed the boundary.
- Review red oracle: an empty generated assistant response failed construction
  because the response reused the stricter inbound message type; nonblank model
  identity and the `temperature=0`/`top_p` no-op relationship were not closed.
- Implementation: one shared closed request model rejects extra fields and
  coercion, centralizes model/sampling/token validation, and gives chat and
  completion their bounded text shapes. A distinct assistant response model
  permits empty generated output. `temperature=0` requires `top_p=1`, blank
  model identifiers are rejected, and the unreachable stream implementation
  is deleted.
- Module result: the existing Pydantic request decoder remains the single
  Interface and owns the supported subset. The slice adds no worker, queue,
  executor, framework, dependency, schema artifact, or compatibility shim.
- Documentation: `torch-server/README.md` now states the exact OpenAI-shaped,
  text-only, non-streaming request subset; Torch's non-shipped state; the
  fake-test boundary; and the known real-runtime and managed-deployment
  blockers.
- Green evidence:
  - focused validation/application test module: 16/16 passing, including the
    five new public request/response contract regressions;
  - full fake-backed Torch unit suite: 18/18 passing;
  - `python3 -m py_compile torch-server/openai_api.py
    torch-server/tests/test_validation_and_app.py`: passed;
  - Ruff check and format check over the two changed Python files: passed;
  - scoped whitespace diff check: passed.
- Held claims: placeholder usage, terminal-reason truth, redacted HTTP
  failures, event-loop responsiveness, admission, overload, cancellation,
  disconnect, shutdown, production dependencies, managed deployment, and one
  required-real model/device tuple remain unresolved under DRBT-I7 through
  DRBT-I9. Milestone 3 and DRBT-A5 therefore remain open.

### 2026-09-03 — Start M2 persisted-authority slice

- Operation: `continue`; plan remains `Active`, and Milestone 2 moves from
  `Planned` to `Active` while Milestones 1, 3, and 4 retain their recorded
  blockers.
- Investigation result: the launcher-root resolver returns a bare path and
  collapses missing, malformed, unreadable, and invalid persisted records to
  `null`. Every collapsed result then permits portable/discovery/default
  selection, so corrupted authority can silently select a different library.
- Explicit-authority result: environment and argument overrides bypass the
  canonical existing-root validator. The backend can then create the selected
  working directory, turning an invalid override into new authority.
- Persistence result: selection writes directly to the authoritative file
  after creating its parent. There is no same-directory temporary file,
  exclusive preparation, file synchronization, atomic rename, directory
  synchronization, or interruption oracle. This is held for a later slice.
- Stream result: five bridge stream owners exist, but the model-library stream
  is always-on; four others use process-global renderer counters; window close
  omits model-download and model-library cleanup; most preload subscribe and
  every unsubscribe promise are unobserved; stream destroy/close is not joined;
  and malformed events remain log-and-drop. Cursor/event schema repair stays
  blocked on Milestone 1, while ownership can be sliced independently later.
- Confirmed public seam: `resolveLauncherRoot` is the caller/test Interface.
  It will return a closed discriminated resolution carrying persisted-state
  provenance. Only `absent` may discover or initialize; `valid` identifies the
  accepted root; `invalid` and `unavailable` return stable path-free recovery-
  required results; `not-consulted` records explicit override precedence.
- Exact write set:
  - `electron/src/launcher-root.ts`
  - `electron/src/main.ts`
  - `electron/src/startup-task.ts` (admitted by focused re-plan after startup-order review)
  - `electron/tests/launcher-root.test.mjs`
  - this plan, ledger, issues, and `reports/desktop-lifecycle-evidence.md`
- TDD oracle order: first malformed persisted state with a valid discovery
  decoy; then unavailable persisted state with the same decoy; then invalid
  argument and environment overrides. Each red is captured through the public
  resolver before its minimal implementation. Successful absent/valid and
  override-precedence states are asserted at the same Interface.
- Held files and claims: preload, PythonBridge, Electron package/manifests,
  generated output, frontend, Rust/RPC projection, atomic persistence, and
  stream ownership are excluded. This slice is incremental DRBT-A3 evidence,
  not required-real cross-platform or Milestone 2 acceptance.

### 2026-09-03 — Reach focused M2 persisted-authority review boundary

- First red: with a valid discovery decoy present, malformed persisted JSON
  resolved to that decoy instead of an authority failure. The new result shape
  also exposed absence and valid persisted selection through the same public
  Interface.
- Second red: making the user-data path a regular file caused the prior
  existence probe to report absence and select the discovery decoy instead of
  reporting unavailable persisted authority.
- Explicit-authority reds: a nonexistent `--launcher-root` and a nonexistent
  `PUMAS_LAUNCHER_ROOT` were each returned as resolved. After validating those
  paths, four presence edge cases remained red: missing argument value, next
  token another flag, blank inline value, and blank/whitespace environment
  value all collapsed to absence and discovery.
- Filesystem-validation reds: a regular file at the canonical
  `shared-resources/models` marker was accepted as a launcher root, and an
  injected deterministic `EIO` during validation was ignored because the
  resolver used `existsSync` rather than a typed filesystem result.
- Final contract reds: NUL-bearing explicit and persisted paths were classified
  as unavailable rather than invalid, and arbitrary/nonexistent descendants of
  a valid launcher root climbed to that ancestor for argument, environment, and
  persisted authority. Startup also logged the resolved absolute path and
  logged the typed recovery Error object with its stack; selection persistence
  logged the absolute root or raw filesystem message.
- Implementation: `resolveLauncherRoot` now returns one correlated
  discriminated result. Explicit roots are normalized by the canonical
  existing-root validator and record persisted state as `not-consulted`;
  persisted records are `valid`, `invalid`, or `unavailable`; only `absent`
  permits portable/discovery/default selection. Main-process composition throws
  only the stable code/message and never starts the backend on a recovery-
  required result.
- Filesystem Adapter: authoritative validation uses a narrow injected
  `readFileSync`/`statSync` seam, requires marker directories, classifies only
  known missing or malformed path-domain codes as invalid, and projects access
  or I/O failures as path-free unavailable results for environment, argument,
  and persisted authority. Best-effort discovery does not weaken that
  authoritative proof.
- Selection boundary: authoritative normalization checks only the exact root,
  exact `root/shared-resources`, or exact
  `root/shared-resources/models`. Ancestor walking is reserved for discovery
  after persisted absence. `ERR_INVALID_ARG_VALUE`, `EINVAL`, and
  `ENAMETOOLONG` join `ENOENT`/`ENOTDIR` as invalid path-domain outcomes;
  access and I/O failures such as `EACCES`/`EIO` remain unavailable.
- Precedence and diagnostics: a present environment override precedes and
  suppresses argument/persisted discovery. Main-process startup logs only the
  resolved source or the typed recovery code/source/message; it does not log
  the absolute root or raw recovery Error. Selection persistence emits one
  stable path-free success/failure diagnostic.
- Persisted JSON policy: the top-level JSON value must be a runtime-checked
  record, not a TypeScript assertion. `launcherRoot` is the sole authority-
  bearing field and must be a string resolving to an existing launcher root.
  `selectedPath`, `updatedAt`, and unknown fields are non-authoritative metadata
  and are ignored by resolution, so they cannot change the selected owner.
- Green focused evidence: fourteen public resolver cases pass on the real local
  Linux temporary filesystem, including absence/discovery, valid persisted
  precedence, malformed and unavailable records with a discovery decoy,
  invalid and valid explicit overrides, explicit precedence/normalization, and
  every present-but-empty form. They also prove marker-directory type and the
  deterministic I/O-failure Adapter outcome, malformed-path classification,
  and descendant rejection for all three authoritative sources.
- Review status: focused source/test boundary is green and pending independent
  acceptance. Atomic replacement, interruption/durability behavior, explicit
  renderer recovery, and required-real Windows/macOS evidence remain open;
  stream/RPC/package/shared-doc files did not enter the slice.

### 2026-09-03 — Correct immediate backend-initialization observation

- Review red: `initializeBackend()` was started before `createWindow()`, but its
  rejection handler was attached only after window creation completed. An
  immediate launcher-root recovery rejection could therefore reach the global
  `unhandledRejection` handler first and log the raw typed Error/stack.
- Focused re-plan: root admitted only `electron/src/startup-task.ts` beyond the
  recorded write set. The seam is domain-specific to backend initialization;
  it does not introduce a generic task framework.
- TDD red: the delayed-window/immediate-rejection public regression failed at
  module resolution because no owned observation seam existed.
- Implementation: startup now converts backend initialization immediately into
  one never-rejecting `fulfilled` or `rejected` outcome before awaiting window
  creation. The original error remains the typed terminal value. Normal startup
  projects one failure diagnostic and quits; release-smoke rethrows that same
  failure instead of reporting startup success.
- Diagnostic boundary: launcher-root recovery failures project only the stable
  code, authority source, and message. Unexpected backend failures retain their
  Error for the existing general diagnostic path; this slice does not claim
  repository-wide raw-error removal.
- Green evidence: the new regression rejects before a delayed window turn,
  observes no process-level `unhandledRejection`, preserves the exact typed
  failure, and projects a diagnostic with no raw Error field. The full Electron
  build plus all six test files, lint, and `tsc --noEmit` pass.
- Review status: the corrected boundary is frozen for final cross-review. The
  remaining DRBT-A3/DRBT-A4 and required-real evidence gates are unchanged.

### 2026-09-03 — Admit M2 atomic-persistence slice

- Operation: `continue`; the prior persisted-authority/startup-observation
  boundary was accepted and integrated as `1964760d`.
- Investigation result: the writer validates the selection and then directly
  truncates `launcher-root.json`. It has no exclusive adjacent temporary file,
  file synchronization, atomic namespace replacement, parent-directory
  synchronization, owned-temp cleanup, or typed partial-publication result.
- Selected contract: under the Electron single-instance writer, validate and
  serialize once; create one unpredictable same-directory temporary file with
  exclusive `wx` and mode `0600`; write, synchronize, and close it; rename it
  over the authority; synchronize and close the already-open parent directory;
  report success only then. There is no retry, copy, unlink-target, rollback,
  or alternate publication mechanism.
- Failure contract: all pre-rename failures preserve prior authority bytes and
  clean only this operation's owned unpublished temp. Rename failure is
  `replacement-visibility-unknown`. Any parent sync/close failure after rename
  is `published-durability-unavailable`: the complete new destination remains
  visible, no success/relaunch occurs, and the writer does not retry or roll
  back. Cleanup incompleteness is bounded metadata and never replaces the
  primary stage/cause.
- Evidence boundary: real local Linux reopen/bytes/mode and subprocess
  termination barriers may prove old-or-new namespace publication on that
  filesystem. They do not prove power-loss behavior, every Linux filesystem,
  Windows/macOS parent flushing, remote/removable filesystems, concurrent
  writers, or orphan garbage collection. Those claims remain unavailable.
- Exact write set:
  - `electron/src/launcher-root.ts`
  - `electron/tests/launcher-root.test.mjs`
  - this plan, ledger, issues, and `reports/desktop-lifecycle-evidence.md`
- Held files/claims: `main.ts` retains its accepted call shape and path-free
  error projection; preload, PythonBridge, package/manifests, renderer,
  streams, Rust/RPC/generated source, frontend, CI, and shared docs are excluded.
- TDD order: named injected failures for parent open, partial/full temp write,
  temp sync/close, rename, parent sync/close, and cleanup; then real Linux
  success and subprocess termination before/after publication.

### 2026-09-03 — Reach focused M2 atomic-persistence review boundary

- TDD red: the focused test module could not import the selected typed
  persistence error because the atomic writer Interface did not exist. At the
  baseline public Interface, the persistence Adapter was absent and writes
  directly truncated the authority.
- Module result: `persistLauncherRootOverride` retains its existing success
  shape and main-process consumer. Its narrow stage-named Adapter owns only the
  filesystem mechanics needed to prove exclusive temp preparation, publication,
  parent synchronization, cleanup, and partial failure without exposing a
  general filesystem facade.
- Publication sequence: ensure the data directory, open its directory handle,
  create an unpredictable typed temporary name, exclusively open that adjacent
  temp at `0600`, write and fsync all bytes, close the temp, rename it over the
  authority, fsync the directory, and close that directory handle. Success is
  returned only after the complete sequence.
- Failure result: `LauncherRootPersistenceError` has one stable path-free
  message plus the failed stage, authority state, cleanup state, and preserved
  cause. Pre-publication failures are `unchanged`; rename failure is
  `replacement-visibility-unknown`; post-rename parent sync/close failures are
  `published-durability-unavailable`. Cleanup never removes the authority,
  retries publication, rolls back a visible replacement, or replaces the
  primary cause.
- Injected green evidence: directory ensure, parent open, temporary-name
  creation, temp open, partial write, full write, temp sync, temp close, rename,
  parent sync, parent close, and secondary cleanup failures all reach their
  selected outcome. Cleanup may legally throw a non-Error value without masking
  the primary typed stage/cause. Public resolver reopen proves the old authority
  through pre-publication failures and the complete new authority after publication.
- Real local Linux evidence: the successful default Adapter writes readable
  JSON with mode `0600`, follows the exact nine-stage order, leaves no owned
  temp, and reopens through the public resolver. Separate subprocesses killed by
  `SIGKILL` immediately before and after rename leave complete old and new
  authority respectively, never partial target bytes.
- Verification: all 30 focused launcher-root/startup cases pass, including 15
  new persistence cases; the full Electron build plus all six test files, lint,
  and `tsc --noEmit` pass.
- Claim boundary: this is local process-interruption and namespace-publication
  evidence on the exercised Linux temporary filesystem. It does not prove
  power-loss durability, every Linux filesystem, Windows/macOS parent flushing,
  remote/removable filesystems, concurrent writers, or orphan cleanup. DRBT-A3
  and Milestone 2 remain open. The existing main-process consumer fails closed
  without success/relaunch/retry, but renderer projection of replacement-unknown
  and published-durability-unavailable states remains future recovery work.

### 2026-09-03 — Admit M2 renderer-recovery producer tranche

- Operation: `continue`; the atomic publication boundary was accepted and
  integrated as `767e71f0`.
- Investigation result: typed startup authority failure is logged and quits the
  app, so the native chooser is unreachable. Selection persistence outcomes are
  collapsed into an optional-field `{success,error}` bag, forwarded by preload
  without runtime decoding, and discarded into renderer logs.
- Producer ownership: `launcher-root.ts` retains detailed resolution,
  validation, persistence stage/cause, and authority state. A new pure
  `launcher-root-recovery.ts` Module owns only the closed JSON-safe renderer DTO,
  projection, and runtime decoder. Main owns lifecycle state and native IPC;
  preload owns invocation and trust-boundary decoding.
- Closed startup contract: `initializing`, `ready`, or `recovery-required` with
  reason `invalid|unavailable`, source `persisted|environment|argument`, and
  action `select-library|correct-launch-input`. Persisted failure may invoke the
  chooser; an environment/argument failure may not, because persisted state
  cannot outrank the same explicit input on relaunch.
- Closed selection contract: `cancelled`, `restarting`, or `recovery-required`
  with reason `invalid-selection|persistence-unavailable` and authority state
  `unchanged|replacement-visibility-unknown|published-durability-unavailable`.
  Paths, selected values, persistence stages, cleanup state, raw causes, and
  source chains never cross preload.
- Lifecycle decision: normal startup keeps the window open only for the typed
  launcher-root recovery state; other backend failures and release smoke remain
  fatal. A selection failure never reports success, retries, rolls back, or
  relaunches. Only a fully persisted selection retains the existing automatic
  relaunch.
- Exact producer write set:
  - `electron/src/launcher-root.ts`
  - `electron/src/launcher-root-recovery.ts` (new)
  - `electron/src/startup-task.ts`
  - `electron/src/main.ts`
  - `electron/src/preload.ts`
  - `electron/tests/launcher-root-recovery.test.mjs` (new)
  - `electron/tests/preload-rpc-contract.test.mjs`
  - this plan, ledger, issues, and `reports/desktop-lifecycle-evidence.md`
- Held frontend tranche: `frontend/src/types/{api-window.ts,
  api-bridge-utilities.ts}`, `frontend/src/hooks/{useLauncherRootRecovery.tsx,
  useLauncherRootRecovery.test.tsx,useAppWindowActions.ts,
  useAppWindowActions.test.ts}`, `frontend/src/components/{
  LauncherRootRecoveryView.tsx,LauncherRootRecoveryView.test.tsx}`, and
  `frontend/src/index.tsx` require separate root/frontend-owner serialization.
  They do not overlap the active catalog DTO files.
- Atomic integration constraint: the Electron tranche must not integrate while
  its recovery UI is unreachable. Producer and frontend consumer require
  coordinated acceptance. No catalog, App root, ModelManager/list, adapter,
  package, Rust/RPC/generated, CI, or shared-doc file enters this tranche.

### 2026-09-03 — Reach initial M2 renderer-recovery producer boundary

- TDD red: the new public recovery test could not import the absent recovery
  Module; the preload source oracle showed launcher-root IPC values were still
  forwarded undecoded. A later lifecycle red could not import the absent
  backend-outcome classifier needed to distinguish normal desktop recovery from
  release-smoke failure.
- Module result: `launcher-root-recovery.ts` owns one closed JSON-safe startup
  state, one closed selection result, exact-shape decoders, source/action
  correlation, and path-free projections. Detailed resolution paths,
  persistence stages, cleanup state, and raw causes remain private to the
  launcher-root producer.
- Main/preload result: main publishes the projected startup state, permits the
  native chooser only when current authority can be superseded, returns exact
  terminal selection outcomes, and relaunches only after complete persistence.
  Preload decodes both IPC results before exposing them to a renderer.
- Lifecycle result: one pure classifier maps a fulfilled initialization to
  ready, typed launcher-root failure to normal-desktop recovery, and every
  release-smoke rejection or unrelated backend failure to fatal while
  preserving the original error. Main keeps the window open only for the
  selected recovery disposition.
- Focused green: 11 recovery/preload checks pass, including exact-shape and
  correlated-value negatives, removal of paths from projections, non-explicit
  chooser availability, selection authority outcomes, normal-versus-smoke
  lifecycle classification, and preload decoder wiring. The complete Electron
  build and all seven test files pass.
- Claim boundary: this is producer, local unit, source-contract, and build
  evidence. The frontend provider/view is still absent, so this tranche must
  remain unintegrated until the separately serialized consumer is accepted.
  Required-real packaged target evidence, stream ownership, and DRBT-A3 final
  acceptance remain open.
- Review result: `not accepted`. Resolved environment/argument authority was
  collapsed into chooser-capable ready state; the main handler had no
  single-flight/current-attempt owner; native dialog rejection escaped the
  closed DTO; and chooser admission remained open after ambiguous, published,
  or restarting outcomes.

### 2026-09-03 — Correct M2 renderer-recovery ownership boundary

- Operation: `re-plan`; the exact 11-file write set is unchanged. No frontend,
  catalog, package, Rust/RPC, shared-doc, or additional source/test file entered
  scope.
- TDD red: handler-level cases could not import the absent selection owner, and
  existing projection expectations showed that resolved environment/argument
  roots had no path-free selection policy. The red contract also selected
  truthful chooser-unavailable, explicit-authority, concurrent-attempt,
  retryable-unchanged, locked-publication, and restart-request outcomes.
- Contract correction: ready startup now contains only
  `selectionAction: select-library|correct-launch-input`. Resolved environment
  and argument authority selects correction; persisted/discovered/default
  authority selects the chooser. The main owner enforces the same policy even
  if a renderer invokes IPC directly.
- Closed result correction: `not-selectable/correct-launch-input` and
  `chooser-unavailable/unchanged` no longer masquerade as persistence failure.
  A failure to synchronously request relaunch after durable publication is
  `restart-unavailable/published`. Preload exact-shape decoding owns all new
  correlated variants.
- Lifecycle owner: one `createLauncherRootSelectionHandler` instance owns the
  current attempt. Concurrent callers share exactly one Promise and outcome.
  Cancel, chooser unavailable, invalid selection, and persistence failure with
  proven-unchanged authority return to idle for explicit retry. Restarting,
  replacement visibility unknown, published durability unavailable, and
  restart unavailable are locked terminals replayed without opening another
  dialog, persisting, or requesting another restart.
- Restart ordering: after persistence success, the injected restart Adapter
  calls `app.relaunch()` synchronously and schedules only the response-flush
  delay before quit. A thrown relaunch request or timer-scheduling failure is
  observed before `restarting` can be returned and reaches the locked published
  recovery outcome.
- Focused green: 18 recovery/preload checks pass, including resolved explicit
  policy, exact DTO correlation, no-window/dialog rejection, overlap sharing,
  unchanged re-entry, unexpected/ambiguous/published locking, successful
  terminal replay, synchronous restart-request failure, and main composition
  ordering. The complete Electron build and all seven test files pass.
- Claim boundary: frontend recovery remains absent and atomic integration is
  still blocked. This is incremental local producer evidence only; required-
  real packaged targets, streams, and DRBT-A3 acceptance remain open.

### 2026-09-03 — Repair sandboxed preload composition

- Operation: `re-plan`; a Critical composed-runtime oracle reopened the
  uncommitted producer. Production uses `sandbox:true`,
  `contextIsolation:true`, and `nodeIntegration:false`, while the TypeScript
  build emitted `require('./launcher-root-recovery')` from `dist/preload.js`.
  Electron 39.8.6 could not resolve that local module in a sandboxed preload,
  so `window.electronAPI` was absent.
- Exact write authority remains the same 11 files. The repair changes only the
  existing recovery/preload source and tests plus these four plan-local docs;
  it does not change package/build tooling, sandbox policy, frontend, catalog,
  Rust/RPC, or shared docs.
- TDD red: after a canonical Electron build, the compiled-preload dependency
  oracle found `./launcher-root-recovery` as an unsupported local runtime
  require. The preserved real frontend oracle independently emitted an unable-
  to-load-preload/module-not-found diagnostic and no bridge.
- Ownership correction: `launcher-root-recovery.ts` remains the sole producer
  owner for DTO types, resolution projection, selection policy, and attempt
  lifecycle. `preload.ts` is the sole runtime inbound decoder at the sandboxed
  IPC trust boundary and imports producer types only. The emitted JavaScript
  therefore requires only Electron; no parallel runtime decoder remains.
- Supporting green evidence: the compiled-output check accepts exactly the
  `electron` runtime require and rejects every other module. A compiled-preload
  VM harness exercises every closed startup and selection state plus
  malformed/extra negatives through the exposed API.
- Deciding green evidence: pinned Electron 39.8.6 ran a hidden BrowserWindow as
  uid 1000 on Linux with `DISPLAY=:0`, no `--no-sandbox`, and the exact
  production preferences. The actual built preload exposed `electronAPI`; all
  nine startup and nine selection values round-tripped; three malformed startup
  and six malformed/extra selection values rejected with stable messages; and
  no preload, load, renderer-gone, or unresponsive failure occurred. The owned
  child exited zero in 551 ms and the test removed its unique temporary state.
  The hardened harness gives Electron its own process group; success and forced
  timeout paths observe that the group is gone before removing that state.
- Full green: the Electron build and all 77 tests, including the enabled real
  sandbox oracle with no skip, pass; lint, `tsc --noEmit`, plan structure, and
  scoped tracked/untracked whitespace checks pass.
- Claim boundary: this is representative real Linux Electron evidence for the
  sandboxed preload loader/decoder contract. It is not packaged-artifact or
  required-real Windows/macOS evidence. The renderer consumer remains held, so
  producer integration is still blocked pending the composed UI oracle.

### 2026-09-03 — Hold first-visible-frame repair at proportionality gate

- Operation: `re-plan`; the Electron producer and serialized frontend consumer
  remain uncommitted. Product source is frozen pending a product decision; no
  stream, RPC, package, Rust, or shared-document file entered the slice.
- Composed red: a real hidden Electron 39 BrowserWindow retained a Checking
  compositor frame when the renderer sent its semantic acknowledgement after
  two animation frames. DOM/layout timing therefore could not authorize native
  show.
- Causal marker result: the bounded owner subscribes only for one current
  document challenge, inserts a fixed path-free checker, requires a matching
  NativeImage, removes the exact CSS, requires a strictly later marker-free
  NativeImage, re-correlates authority/document state, and only then permits
  show. Forty-three focused owner cases pass, including queued and pre-insert
  frames, navigation/fallback cancellation, late insertion cleanup, duplicate
  callbacks, every Adapter failure stage, and terminal continuation guards.
  The permanent real preload case also proves marker-present then marker-free
  while hidden with production sandbox preferences.
- Real proportionality result: the corrected default-app matrix showed
  commit-to-show durations of 1283.0 ms immediate and 1049.4–1247.9 ms with a
  250 ms authority reply; library-only measured 663.9 ms immediate and
  363.5–438.2 ms delayed. Every completed case showed content, never Checking,
  and exited cleanly, but the default cost conflicts with the immediate-list
  objective and is not accepted merely because correctness is green.
- Rejected direct-bootstrap prototype: a throwaway copy passed one versioned,
  bounded, path-free terminal state through `additionalArguments`, decoded it
  in the sandboxed preload, seeded the provider synchronously, and wrapped the
  initial React render in `flushSync`. The semantic acknowledgement preceded
  `ready-to-show` (576.5 ms versus 626.2 ms), but the only pre-show NativeImage
  was a uniform opaque fuchsia surface (1280×874; repeated interior BGRA
  `[138,0,255]`), not terminal application content. Ready-to-show plus layout
  acknowledgement is therefore rejected as frame proof.
- Rejected hidden-capture prototype: after the same terminal boot and
  `flushSync`, acknowledgement preceded ready-to-show (640.3 ms versus
  691.3 ms). Fixed marker insertion resolved at 719.8 ms, but
  `capturePage(undefined,{stayHidden:true})` returned only at 2019.4 ms and the
  image did not contain the marker. The mechanism was neither causal nor faster
  than the frame subscription and was stopped before further cases; no retry,
  delay, invalidation workaround, or product edit was added.
- Stopping condition: both faster mechanisms are rejected. Full recovery,
  no-preload, fatal, reload, and 9+9 producer-to-renderer conformance remain
  held rather than inferred from the completed ready matrix. DRBT-A3 and
  Milestone 2 remain active.

### 2026-09-05 — Reconcile committed desktop integration

- Operation: `continue` on this canonical plan; standards `1609c304`.
  Documentation-only ownership is this plan, issues, and ledger. Existing
  historical entries remain evidence of their earlier states.
- `2b081fba` accepted the coordinated selected Rust-to-desktop contract,
  bundled sandbox preload, root-scoped display bootstrap, and renderer
  integration. It supersedes the active generation-unavailable and
  uncommitted/absent-consumer holds, not all-route or stream acceptance.
- Deciding evidence is the parent ledger's
  [Coordinated Desktop Contract Verification](../execution-ledger.md#2026-09-05--coordinated-desktop-contract-verification):
  generator 5/5, actual-producer decoder 5/5 and renderer 2/2, Electron 129
  tests with one explicit runtime skip plus the separately enabled real
  sandbox oracle, frontend 520 tests and builds/types/lint, and built Linux
  GUI with 83 catalog models, two labelled paused activities, nine partial
  percentages, no Ready-to-finish state, zero renderer errors, and clean
  shutdown. Warm cache painting is display-only; it grants no stale actions.
- `2b9553a0` separately accepted RUST-I12's classification correction on Linux.
  Its source, no-loss evidence, and non-Linux runtime limits remain owned by
  the Rust plan. It does not close a desktop release or lifecycle claim.
- DRBT-A1 through A9 retain their complete gates. Windows/macOS runtime
  evidence is pending, not presumed failed; CI/build availability alone is
  not execution proof. Torch runtime/deployment, binding/release, broader
  transport, and stream ownership remain outside this acceptance.
- Next independent slice is DRBT-I11's model-library stream custody
  investigation on Linux. Preserve the accepted barrier and generated
  contract; admit the exact source set and real terminal oracle before edits.

## Reports

| Planned report | Milestone | Status |
| --- | --- | --- |
| `reports/release-and-host-contract-decision.md` | 0 | `accepted` |
| `reports/rpc-contract-conformance.md` | 1 | `pending` |
| `reports/desktop-lifecycle-evidence.md` | 2 | `active` |
| `reports/torch-runtime-evidence.md` | 3 | `active` |
| `reports/launcher-platform-evidence.md` | 4 | `pending` |
| `reports/binding-host-matrix.md` | 5 | `pending` |
| `reports/release-evidence.md` | 6 | `pending` |
| `reports/final-acceptance.md` | 6 | `pending` |
