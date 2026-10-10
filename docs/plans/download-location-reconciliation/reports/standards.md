# Coding standards basis and review obligations

Authority: [plan](../plan.md). Source: **coding standards MCP**, routed for the
actual proposed remediation rather than a generic repository checklist.

## Route and snapshot

Final snapshot: `snapshot:v1:5a493abf-68f1-4651-95b0-ab92891b2f3d`,
schema version 5; **42 selected policies, zero unresolved fact categories**.
Both content pages were captured. This supersedes planning's initial 41-policy
route by adding tooling for the small package acceptance helper.

Review amendments rerouted the same scope at
`snapshot:v1:f7ec5a4e-affb-4206-91ad-a4b1f74260fe`, schema 5: 42 selected
policies, zero unresolved categories, both content pages read. Compatibility
contracts now require exact older direct-consumer and upgraded-server RPC tuples;
concurrency acceptance now includes durable-to-live claim settlement and stale
inventory fencing. These are required evidence, not claims already satisfied.

Facts: planning, uncertainty reduction, implementation, verification,
documentation, build, release, commit and tooling; library/frontend/launcher;
persistence/IPC/generated-contract boundaries; Rust, Rust async/API/cross-platform
and TypeScript/async; architecture, contracts, concurrency, resilience,
diagnostics, security and cross-platform; explicit schema/evolution/protocol,
design, discovery, GUI/oracle and release-operations details. No framework or
concurrent-integration profile is asserted.

The installed router defines no Python-specific language profile or standalone
CLI application category. The Python acceptance helper is governed by generic
core/tooling/build/verification/security and existing repository Python
requirements; do not invent an unsupported profile or claim missing routing
facts were resolved by a guessed enum value. Its standard-library implementation
has no new runtime dependency and uses the repo's ignored `.venv`.

The MCP route and its canonical policy owners are normative inputs. This report
records application to the proposed design, not certification of code that has
not been written. Re-read/reroute at implementation admission; material changes
must update applicability and the concrete dispositions below.

## Applied obligations

| Policy owners | Concrete consequence in this plan | Required implementation evidence |
| --- | --- | --- |
| Core; Planning/Discovery/Proportionality; Documentation | Evidence-backed direction, scoped uncertainty, one canonical plan and next slice; no implementation started by writing a plan. Keep execution facts here and enduring decisions in one ADR/contract. | Admission record, slice state, exact write-set review and honest acceptance statuses. |
| Architecture; Code Design; Library; Rust API | One deep core resolution interface hides custody/ordering/storage. Distinct attempt/artifact/receipt owners; no new daemon, umbrella recovery store or caller-visible effect grants. The eight-part probe examines the complete produced artifact. | Actual owner/call inventory, representative change locality, deletion review and C1/C2/C4. |
| Contracts; Evolution; Schemas | Canonical meaning is core policy; wire declarations project it. HF format 6 scopes only attempt-resolution meaning; envelope 7 and actual receipt versions keep their independent promises. Exact old/new reader and generic-writer combinations are named. | C3/C5; strict duplicate/unknown handling, lossless prior snapshots, no downgraded semantics or indefinite backward writer. |
| Persistence; Resilience | Recheck under native authority and canonical transaction; resolution history and admission release publish atomically. No SQL repair journal, false Completed or success on uncertain durability. Bind idempotency to exact request meaning; retain history without automatic expiry. | C2/C4/C6/C8 through real stores and process interruption. |
| Concurrency; Rust Async; TypeScript Async | Reuse existing bounded task/effect owners. No sync guard across await, blocking work on async request threads, callbacks under locks, detached recovery or abandoned effects on disconnect. Close admission and drain before release. | Controlled interleavings, blocking-work classification, shutdown and lost-reply evidence C1/C4/C6. |
| Security; Rust Security; IPC; Protocols | Decode unknown inputs at each trust boundary using the declared contract; use server-side exact identity/physical capabilities, not raw paths, caller claims or permissive JSON casts. Preserve loopback/admission/sandbox/allowlist policy and reject escapes. | Exact negative diagnostics C3/C4; actual Electron IPC and RPC decoder/handler path. |
| Generated Contract; Build; Dependencies | Change the existing canonical RPC declaration, derive six affected outputs deterministically, reject stale outputs, preserve all reachable outcome semantics. No hand-edited generated output or parallel schema interpreter. Declare tools/dependencies before tests. | Freshness **and** semantic/consumer evidence C3/C5/C7/C9; generator dialect/keyword inventory and independent reference evidence for standardized semantics it interprets. |
| Frontend; Accessibility; TypeScript | Renderer is a projection with explicit user choice, accessible controls/focus/status and real refresh/reopen convergence. Show unverified local acceptance honestly; never authorize a backend transition from a UI guess. | Component interaction checks plus the real desktop workflow C7/C10. |
| Cross-Platform; Rust Cross-Platform; Platform Verification | Preserve root identity/containment and native exclusion on supported targets; unsupported capabilities stay typed. Linux package qualification cannot stand in for other required target behavior. | Native target map and observed results C1/C4/C9, Linux Debian evidence C10. |
| Diagnostics | Bounded operation/attempt context for operator and UI; one responsible boundary reports terminal failure. Preserve typed cause and distinguish unavailable, stale, failed and uncertain. Do not expose secrets, full live documents or unrestricted personal paths in retained evidence. | Structured diagnostic assertions and reviewed logs/UI disclosure C3/C6/C7. |
| Implementation; Verification; Independent Oracles | All runtime paths enforce the invariant. No production placeholders, accepted skipped tests, incidental negative failures or self-confirming projection oracle. Fixtures observe real filesystem/store/consumer effects against the contract. | C1–C10 and closed scoped MUST findings; no code acceptance based only on compilation. |
| Launcher; Tooling | Existing procedures own builds/releases; acceptance helper adds only missing fixture/workflow observations. Isolated fixture/config and bounded children; Python env remains in repo. No silent installs, host-state fallback, redundant installer framework or weakened normal runtime. | Measured execution scope, child cleanup and actual packaged path C8/C9/C10. |
| Commit | Stage only admitted coherent content, review exact staged diff, pass affected gates, conventional commit with active hooks. Preserve original dirt and published history; review branch range at PR/release boundaries. | Staged-scope/hook evidence, PR/base/range and protected worktree disposition. |
| Release; Release Operations | Distinctly identified review Debian candidate with final checksum, inputs and attribution; perform feature workflow through package. Rebuilding does not publish a version or grant live mutation. Recovery/rollback never copies weights or assumes unsafe downgrade. | C10; package provenance, install/extraction and normal desktop reopen; no inferred tag/publication authority. |

Every row is a blocking design/review obligation where applicable, not a
collection of advisory preferences. At milestone review, identify exact code
evidence for each selected applicable MUST; record unavailable evidence or
findings and their responsible owner. No accepted implementation may leave an
unresolved relevant MUST finding.

## Selected normative owners

These identifiers resolve through the pinned MCP snapshot. Canonical owner paths
are within the coding standards source, not this repository.

| Policy ID | Canonical source owner |
| --- | --- |
| `core` | `CORE-STANDARDS.md` |
| `router` | `STANDARDS-ROUTER.md` |
| `profile.language.rust` | `profiles/languages/rust/README.md` |
| `workflow.documentation` | `workflows/documentation.md` |
| `workflow.implementation` | `workflows/implementation.md` |
| `workflow.verification` | `workflows/verification.md` |
| `topic.accessibility` | `topics/accessibility.md` |
| `topic.dependencies` | `topics/dependencies.md` |
| `profile.language.typescript` | `profiles/languages/typescript.md` |
| `topic.concurrency` | `topics/concurrency.md` |
| `topic.contracts` | `topics/contracts.md` |
| `topic.contracts.evolution` | `topics/contracts/evolution.md` |
| `topic.contracts.protocols` | `topics/contracts/protocols.md` |
| `topic.contracts.schemas` | `topics/contracts/schemas.md` |
| `topic.cross-platform` | `topics/cross-platform.md` |
| `topic.resilience` | `topics/resilience.md` |
| `topic.security` | `topics/security.md` |
| `workflow.build` | `workflows/build.md` |
| `workflow.development-proportionality` | `workflows/development-proportionality.md` |
| `workflow.planning` | `workflows/planning.md` |
| `workflow.release` | `workflows/release.md` |
| `profile.application.library` | `profiles/applications/library.md` |
| `topic.code-design` | `topics/code-design.md` |
| `workflow.commit` | `workflows/commit.md` |
| `profile.application.frontend` | `profiles/applications/frontend.md` |
| `profile.language.rust.async` | `profiles/languages/rust/async.md` |
| `profile.language.typescript.async` | `profiles/languages/typescript/async.md` |
| `profile.language.rust.api` | `profiles/languages/rust/api.md` |
| `topic.architecture` | `topics/architecture.md` |
| `profile.boundary.persistence` | `profiles/boundaries/persistence.md` |
| `profile.language.rust.cross-platform` | `profiles/languages/rust/cross-platform.md` |
| `topic.diagnostics` | `topics/diagnostics.md` |
| `profile.application.launcher` | `profiles/applications/launcher.md` |
| `profile.boundary.ipc` | `profiles/boundaries/ipc.md` |
| `profile.language.rust.security` | `profiles/languages/rust/security.md` |
| `profile.boundary.generated-contract` | `profiles/boundaries/generated-contract.md` |
| `workflow.planning.discovery` | `workflows/planning/discovery.md` |
| `workflow.release.operations` | `workflows/release/operations.md` |
| `workflow.tooling` | `workflows/tooling.md` |
| `workflow.verification.gui` | `workflows/verification/gui.md` |
| `workflow.verification.oracles` | `workflows/verification/oracles.md` |
| `workflow.verification.platforms` | `workflows/verification/platforms.md` |

## Review result at plan creation

The plan applies the systemic-finding rule to the shared-workspace mutation
family instead of patching three individual path strings. It includes the full
composed-design artifact probe, scoped persisted overlap, exact production write
sets, one next slice, typed negative/failure outcomes and direct user/package
acceptance. It avoids invented completion authority and payload backups.

Planning review disposition: ready for implementation selection; **no code
compliance or objective acceptance is claimed**. At start, current state or
standards changes can require replanning. Final scoped code compliance requires
actual implementation review and all C1–C10 evidence, not this planning result.
