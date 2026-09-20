# Plan: Trustworthy Desktop, Torch, Binding, and Release Boundaries

**Plan status:** `Active`

**Current phase:** Milestones 1 and 2 incremental acceptance reconciliation
after the committed desktop integration `2b081fba` and independent Linux
classification correction `2b9553a0`. The selected generated contract, bundled
sandboxed preload, root-scoped display bootstrap, and Linux presentation are
accepted; broader transport and lifecycle objectives remain open.

**Next slice:** Bound DRBT-I11's model-library stream ownership through the
existing Electron main/preload boundary on Linux: inspect subscribe, renderer
close, late callback, and shutdown custody; admit an exact implementation write
set and real Electron terminal-outcome oracle before changing source. Preserve
the accepted generated contract and roughly one-second causal marker barrier;
do not broaden to all RPC routes, Torch, release, or cross-target acceptance.

**Acceptance status:** `pending`

**Composed-design review:** `not-applicable` to this documentation-only
reconciliation and the next read-only stream investigation: neither changes
product composition. The investigation must record applicability and any
required artifact probe before admitting stream implementation; this field
does not waive review for that later design.

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Reports:** Planned evidence reports are indexed in the [execution ledger](execution-ledger.md#reports).

**Audit source:** [Desktop, release, bindings, and Torch](../../../audits/current-standards-2026-09-03/desktop-release-bindings-and-torch.md)

## Objective

Make every shipped desktop, Torch, and host-binding outcome derive from an
accepted contract and fail explicitly when input, capability, or evidence is
invalid, unsupported, or unavailable. A desktop caller observes decoded RPC
responses and events, a launcher user keeps the selected library authority
through failure, a Torch client receives only implemented nonblocking
semantics, a binding consumer loads a version-matched native cohort, and a
release reviewer can prove that the exact promoted bytes match the declared
artifact, dependency, license, version, and target promises.

## Baseline

- Audit code baseline: `a33c8c0efa7cd8783c7deeac9e608db205290d43`.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Audit standards baseline: `52b096ded9c53afd439a3cf0efc4cc85252da570`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Planning used current Core and Router authority plus Planning,
  Development Proportionality, Implementation, Verification, Documentation,
  Build, Tooling, Release, Launcher, Security, Dependencies, Licensing,
  Contracts, Cross-Platform, Concurrency, Resilience, Diagnostics,
  Architecture, IPC, Interop, Generated Contract, Language Bindings,
  TypeScript and TypeScript Async, and the applicable Rust binding, tooling,
  target, and release profiles. The C# async profile governs C# host evidence
  if C# remains in the accepted support matrix.
- No install, release build, GUI run, real Torch inference, foreign-host run,
  or cross-platform runtime suite was performed during this planning pass.
- The planning baseline already removed the obsolete
  `docs/contracts/release-artifacts.md`, `scripts/dev/generate-sbom.sh`, stale
  checked-in SBOMs, and `docs/THIRD-PARTY-NOTICES.md`; it also corrected public
  docs to call the generated bindings experimental. Findings P-03 through P-05
  therefore remain as missing accepted contracts and current release evidence,
  not as authority to restore those deleted artifacts.
- Current inventory evidence includes 152 desktop RPC registrations with
  deferred validation as the default, five generated UniFFI host archives in
  CI, a synchronous-only C# native smoke, lower-bounded Torch production
  requirements, three desktop packaging targets, and one final Linux release
  assembly job. These counts describe the implementation only; they do not
  select a support or release contract.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| DRBT-A1 | Every accepted desktop RPC request, response, and event shape is generated from the Rust/server contract; generated output is fresh, and malformed, unknown, unsupported, or unavailable values have explicit decoded outcomes. | `contract` | `representative` (canonical Rust declaration and generated TypeScript projection) | `automated` | `pending` | Pending Milestone 1 |
| DRBT-A2 | A real `pumas-rpc` process and the Electron main/preload path agree on correlation, status, result, error, and stream-event semantics; malformed producer data is surfaced and sensitive request data is absent from desktop diagnostics. | `integration` | `required-real` (built RPC process and Electron boundary harness) | `automated` | `pending` | Pending Milestone 1 |
| DRBT-A3 | An absent launcher-root record can initialize, while invalid or unreadable persisted authority cannot silently select another library; updates are atomic and interruption-safe. | `system` | `required-real` (each accepted desktop filesystem/OS target) | `either` | `pending` | Pending Milestone 2 |
| DRBT-A4 | Every Electron stream transition has one owner and an observed terminal outcome for subscribe, event delivery, unsubscribe, renderer close, failure, and shutdown. | `integration` | `required-real` (Electron main/preload boundary harness) | `automated` | `pending` | Pending Milestone 2 |
| DRBT-A5 | The declared Torch HTTP subset rejects unimplemented inputs, reports truthful results and usage, and keeps health/control traffic responsive under inference, overload, disconnect, cancellation, failure, and shutdown. | `system` | `required-real` (resolved production dependencies and an accepted local model/device fixture) | `automated` | `pending` | Pending Milestone 3 |
| DRBT-A6 | Each accepted launcher action has equivalent shell and PowerShell routing, rejects unknown platforms, and reaches a bounded observed result after graceful termination or escalation on every supported OS. | `system` | `required-real` (each accepted launcher OS target) | `automated` | `pending` | Pending Milestone 4 |
| DRBT-A7 | Every advertised binding host/target tuple loads and exercises the exact packaged native cohort through its real host, including asynchronous completion, cancellation, and typed failures when those operations are in that host contract. | `integration` | `required-real` (every accepted host/runtime and native target tuple) | `either` | `pending` | Pending Milestone 5 |
| DRBT-A8 | A release dry run produces exactly the accepted artifact set for every release target, rejects tag/manifest or cohort mismatch and missing/unexpected output, and emits final-byte checksums, dependency/SBOM provenance, and required license/notices evidence. | `release-artifact` | `required-real` (every accepted packaging target and final assembly environment) | `either` | `pending` | Pending Milestone 6 |
| DRBT-A9 | Durable release, security, Torch, launcher, and binding documentation states only the accepted support, operation, dependency, and evidence contract. | `contract` | `not-applicable` | `manual` | `pending` | Pending Milestone 6 |

## Scope

### In Scope

- Generated Electron/TypeScript projections of the canonical Rust/server RPC
  request, response, error, and event representations.
- Runtime decoding and negative cross-process tests at the Electron IPC, HTTP,
  preload, and streamed-event boundaries.
- Electron stream subscription ownership and launcher-root persistence,
  recovery, and authority semantics.
- Shared launcher action routing, target selection, child-process deadlines,
  termination, and wrapper equivalence.
- Torch request validation, supported OpenAI-compatible subset, inference-work
  admission/lifecycle, truthful responses, resolved production dependencies,
  and real ASGI/inference evidence.
- Binding host/target/support decisions, pinned generators, native/generated
  cohort packaging, real-host tests, and Rustler host classification.
- Release unit, channel, artifact composition, version/tag, final-byte SBOM,
  checksum, license/notice, target, and evidence automation.
- Canonical documentation and plan evidence required by those changes.

### Out Of Scope

- Canonical Rust/server DTO definitions, Rust-side error semantics, core versus
  adapter placement, Rust runtime ownership, or Rust feature-graph repair;
  those belong to the [Rust library and RPC plan](../rust-library-and-rpc/plan.md).
- Renderer presentation of decoded outcomes, recovery choices, and library-only
  UI behavior; those belong to the [frontend and UI plan](../frontend-and-ui/plan.md).
- Repository-wide standards routing and permanent-gate policy, owned by the
  [governance and verification plan](../governance-and-verification/plan.md).
- Model-library domain semantics, migration behavior, or unrelated persistence.
- Expanding public host, target, distribution-channel, or OpenAI compatibility
  promises merely because current tooling can generate or compile them.
- Code signing, notarization, publishing to a package registry, or publishing a
  GitHub release unless a later accepted release contract explicitly adds it.
- Editing generated host output by hand or moving domain behavior into an
  Electron, Torch, or binding Adapter.

## Owned Outcomes

| Module | Small Interface at the deliberate Seam | Hidden implementation and owned result |
| --- | --- | --- |
| Desktop RPC projection | Decode one named operation's request, response, error, or event against the canonical schema/version. | Generated TypeScript representation, runtime validation, correlation checks, and safe failure mapping. |
| Desktop lifecycle | Read/update launcher-root authority and own a stream subscription through one terminal result. | Atomic persistence, explicit recovery state, subscription counts, renderer closure, and shutdown cleanup. |
| Launcher process execution | Execute one validated action on one accepted platform with a declared deadline/termination contract. | Wrapper delegation, structured spawning, process-tree ownership, grace/escalation, and diagnostics. |
| Torch request execution | Admit one validated supported request and return/stream one observable terminal outcome. | Model scheduling, bounded work, cancellation, usage accounting, ASGI responsiveness, and redacted errors. |
| Binding cohort | Generate, identify, package, and verify one accepted host/native tuple. | Generator provisioning, metadata, native placement, real-host loading, async/error adaptation evidence, and removal of unsupported output. |
| Release assembly | Validate and assemble one accepted release plan from exact input cohorts. | Artifact collection, version/tag/channel rules, final-byte SBOM/checksum/notices, and missing/unexpected-output rejection. |

## Constraints And Assumptions

### Constraints

- The Rust/server plan owns the canonical RPC DTO and error contract. This plan
  may consume and project that contract but may not fork or reinterpret it.
- The generated TypeScript Adapter is derived output, never a second semantic
  authority; generated types do not replace runtime decoding.
- Security-sensitive boundary failures use stable redacted outcomes. Raw
  payloads, credentials, paths, dependency errors, and source chains are not
  release or desktop diagnostics.
- A build, generated file, TypeScript assertion, Rust-only test, fake Python
  module, or host compilation proves only its own claim. Required-real evidence
  cannot be replaced by nearby or simulated evidence.
- Required generators and audit tools are selected, pinned, and provisioned by
  the workflow before use. Generation does not install ambient mutable tools.
- Generated source and native libraries form one identified cohort; cross-run,
  cross-version, cross-target, or cross-profile mixing is invalid.
- Missing consumer, channel, target, host, licensing, toolchain, or environment
  authority produces `unavailable` and blocks the affected promise. It does not
  select an incumbent CI matrix or best-effort tier.
- Changes to `.github/workflows/build.yml` start only after governance plan CI
  edits are integrated or an explicit non-overlapping revision is admitted.
- No implementation slice may publish external artifacts; verification uses
  local output, CI artifacts, or draft/dry-run release assembly.

### Assumptions

- The planning baseline's experimental-binding language is the current public
  promise until Milestone 0 records a narrower or promoted matrix.
- Existing Electron hardening, RPC allowlisting, structured non-shell launcher
  spawning, Torch loopback/token/path controls, model-slot locking, and frozen
  Cargo/pnpm inputs remain useful and should be preserved unless evidence
  invalidates them.
- A small local model artifact can provide deterministic production-loader and
  generation evidence without network access. Milestone 3 must validate that
  assumption before selecting its fixture or evidence claim.
- Hosted Linux, Windows, and macOS runners are real for the OS/runtime behavior
  they execute, but cross-compilation and packaging alone are not runtime proof.

## Binding Decisions

| Decision | Owner | Evidence | Supersedes |
| --- | --- | --- | --- |
| Rust/server DTOs and typed errors are the canonical desktop RPC semantic owner; Electron owns only generated projections, runtime decoding, transport correlation, and cross-process evidence. | Rust plan plus this plan | Cross-plan ownership decision and P-01/P-02 | Hand-maintained deferred Electron schemas and static response assertions |
| The renderer consumes decoded outcomes but does not own transport decoding or library-root recovery semantics. | Frontend plan plus this plan | IPC and generated-contract routing | Renderer/preload type declarations treated as proof |
| Release artifact facts and binding support facts are selected from real consumers and channels before automation; current workflow matrices and available generators are inventory only. Accepted facts are recorded in `scripts/release/artifact-plan.json` and `bindings/support-matrix.json`; reports explain decisions but do not re-own them. | Milestone 0, with product/release owner acceptance | P-03/P-04, existing Node-based release tooling, and Release/Language Binding standards | Incumbent-output and generator-capability inference |
| Each accepted release uses `scripts/release/artifact-plan.json` as the exact artifact-plan authority consumed through the `scripts/release/artifacts.mjs` Interface by validation, assembly, SBOM/checksum production, and release review. | Milestones 0 and 6 | P-03, current Node toolchain, multiple real release consumers, and release artifact-plan rule | Narrative/workflow disagreement |
| A binding package contains generated host material and native binaries only from one recorded build cohort, and every advertised tuple requires real-host load/call evidence. | Milestone 5 | P-04 and Rust Language Binding profile | Source-generation or native-only evidence as host support |
| Torch exposes an explicitly selected subset rather than accepting and ignoring fields. Its work owner must keep ASGI control traffic schedulable and observe admission, completion, cancellation, failure, and shutdown. | Milestone 3 | P-06 and Concurrency/Resilience standards | Silent field dropping and synchronous event-loop inference |
| The [Torch Image Provider Contract Correction](../../torch-image-provider-contract/plan.md) solely owns the reusable no-elapsed-deadline generation lifetime plus image live compatibility, private result evolution, and gateway projection. This plan retains text shapes/semantics, usage, control responsiveness, and shutdown; focused TIPC evidence is necessary but not sufficient for DRBT-A5. | Focused TIPC plan, then Milestone 3 consumption | Explicit 2026-09-20 ownership reconciliation and user clarification | Duplicate generation-lifetime implementation or whole-DRBT-A5 acceptance from a subset |
| Launcher-root states distinguish absent, valid, invalid, and unavailable authority; environment override precedes argument override, and either explicit source must name only an exact root, `shared-resources`, or `shared-resources/models`; only the accepted recovery path may replace a bad persisted owner. | Milestone 2 | P-08, current invocation precedence, and persistence/resilience rules | Corrupt/unreadable state mapped to absence or arbitrary descendant discovery |
| Launcher wrappers delegate the same validated actions to one process-execution Module; target and termination behavior remain explicit per accepted OS. | Milestone 4 | P-09 and Launcher/Cross-Platform standards | Unknown-to-Linux fallback and wrapper-specific release path |
| Obsolete release documents, snapshots, and notice inventories stay deleted. Current evidence is derived from accepted inputs and final shipped artifacts. | Milestone 6 | Planning baseline `d84e2b35` and P-05 | Restoring stale checked-in evidence |
| Pumas has one lockstep product version projected by root, frontend, Electron, and Cargo manifests; the source-level Rust contract is consumed by exact Git revision and is not a standalone GitHub `.crate` artifact. | Product/release owner plus Milestones 0 and 6 | Accepted 2026-09-03 decision and real Pantograph consumer inventory | Unused `.crate` release asset and independently inferred workspace versions |
| GitHub Releases exposes only a `preview` channel. Its desktop matrix is Linux x64 AppImage/Debian, Windows x64 NSIS/portable, and macOS arm64 DMG; a repository maintainer alone manually promotes an evidence-complete draft. | Product/release owner plus Milestones 0, 4, and 6 | Accepted 2026-09-03 decision | `v0.*`-derived prerelease state and build-only support inference |
| UniFFI, Rustler/Elixir, Go, and all generated host-binding release bundles are removed because no real host consumer exists. A later binding is a new consumer/matrix decision, not retained experimental machinery. | Product/release owner; Rust owns source seams and this plan owns release/host projections | Accepted 2026-09-03 decision and bounded consumer search | Five hypothetical UniFFI release seams, non-core Rustler surface, and false Go claim |
| Torch remains a non-shipped source capability until Milestone 3 proves and records one real runtime/platform/device tuple. | Product/release owner plus Milestone 3 | Accepted 2026-09-03 decision | Development source or fake tests treated as deployment support |
| Until a real consumer and runtime tuple prove a broader contract, Torch exposes only an OpenAI-shaped, text-only, non-streaming request subset: exact known fields; `system`, `user`, and `assistant` text messages; bounded prompt/message/model/token values; implemented temperature and nucleus sampling; and no stop sequences. Unsupported streaming and stop behavior is rejected and its dead implementation is removed. | Milestone 3 | Bounded consumer inventory, current implementation inventory, and official OpenAI API reference comparison in `reports/torch-runtime-evidence.md` | General “OpenAI-compatible” inference claim, silently ignored inputs, and retained unreachable streaming behavior |
| The repository maintainer owns manual draft promotion and third-party notice interpretation/acceptance for each final artifact. | Product/release owner plus Milestone 6 | Accepted 2026-09-03 decision | Ambient GitHub permissions or generated inventory treated as approval |

## Evidence And Oracle Plan

| Claim | Domain | Deciding oracle | Independent authority | Unsupported domain | Intended negative failure |
| --- | --- | --- | --- | --- | --- |
| DRBT-A1 | Generated freshness and wire semantics | Generator freshness check plus decoder fixtures covering every selected shape and attribute | Accepted Rust/server schema and error contract | Business/domain correctness inside an operation | Stale output or unmapped shape/variant fails generation or decoding with typed `invalid`, `unsupported`, or `unavailable` |
| DRBT-A2 | Real producer/consumer agreement | Spawned built RPC process exercised through Electron's HTTP/SSE and preload Interfaces | Rust producer behavior, transport status/correlation, and generated schema are compared independently | Renderer presentation and full user workflow | Wrong ID/status/envelope, malformed JSON/SSE, unknown event, or leaked credential makes the test fail; no log-and-drop path passes |
| DRBT-A3 | Persisted authority and interruption | Cold-process filesystem scenarios on each accepted OS, including interrupted replacement and permission/corruption cases | On-disk bytes plus accepted launcher-root format and recovery policy | Renderer wording | Invalid/unreadable state returns its explicit outcome and cannot fall through to discovery |
| DRBT-A4 | Async stream lifecycle | Electron lifecycle harness observes subscribe/event/unsubscribe/close/shutdown results through the stream Interface | Main-process owner state and transport outcome | UI rendering of the result | Rejection, duplicate close, omitted model-download owner, late event, or leaked task fails rather than relying on global logging |
| DRBT-A5 | Torch subset and responsiveness | Real ASGI requests against resolved production dependencies and production loading/generation path with bounded concurrency scenarios; consume TIPC-01–TIPC-12 for delegated generation lifetime and image-provider behavior | Accepted local OpenAI subset, production dependency graph, model/device fixture, and focused generation/image contract | Unclaimed model architectures, fields, devices, or OpenAI behavior | Unsupported field/value, overload, disconnect, cancellation, loader failure, or incomplete shutdown returns the selected observable outcome; event-loop starvation fails the responsiveness budget; TIPC alone cannot pass this broader claim |
| DRBT-A6 | Launcher target/process behavior | Wrapper and process integration tests executed on each accepted OS with controllable child fixtures | Accepted launcher action/target/deadline contract and real OS process semantics | An OS absent from the target contract | Unknown platform, hung graceful exit, orphaned child, divergent wrapper action, or false success fails the suite |
| DRBT-A7 | Host binding support | Real host loads the exact staged native cohort and invokes selected synchronous/async/error paths | Accepted host matrix, adapter contract, generator identity, and cohort manifest | Any unadvertised host/target/capability | Wrong library, mismatched cohort, async failure/cancellation loss, unknown error, or unsupported host prevents package promotion |
| DRBT-A8 | Release artifact closure | Release-plan verifier compares exact staged inputs and final bytes before draft/promotion | Accepted release unit/channel/consumer plan, manifests, dependency resolvers, license authority, and target evidence | Publication/signing claims outside the accepted release contract | Tag/version mismatch, missing or extra artifact, wrong cohort, incomplete SBOM/checksum/notices, or absent target evidence blocks assembly |
| DRBT-A9 | Public truthfulness | Cross-review of durable docs against accepted matrices and executable release plan | Accepted product/release decisions and final evidence | Historical audit text | Any unsupported or evidence-free claim remains experimental/unavailable and blocks documentation acceptance |

Freshness checks do not prove semantic correctness; local producer/consumer
agreement does not prove an external compatibility claim; a warm in-process
fixture does not prove cold-process recovery; and a generated source archive
does not prove host loading.

## Systemic Finding Audit

- Invariant family and canonical owner: RPC wire semantics belong to the
  Rust/server contract; release artifacts belong to the accepted release plan;
  binding promises belong to the accepted host matrix; Torch behavior belongs
  to its selected HTTP subset and work owner; persisted library authority
  belongs to the launcher-root contract; and process termination belongs to the
  launcher action contract.
- Bounded authority, representation, and reachable consumer population:
  `electron/src/{rpc-method-registry,ipc-validation,python-bridge,preload,main,launcher-root}.ts`,
  Electron boundary tests, `launcher.sh`, `launcher.ps1`,
  `scripts/launcher/**`, `torch-server/**`, `bindings/**`, binding generation and
  packaging scripts, the UniFFI/Rustler host adapters, root/electron manifests,
  `.github/workflows/build.yml`, and current release/security/binding docs.
- Expansion facts: expand only for a new semantic owner, a newly accepted
  host/target/channel, a reachable consumer of the same generated or persisted
  authority, a new final artifact, or a material security/lifecycle risk.
- Consumer dispositions: the Rust plan supplies canonical DTO/error input and
  owns Rust-side semantics; the frontend plan renders decoded/recovery outcomes;
  this plan owns Electron projection/decoding, desktop/launcher/Torch lifecycle,
  binding host cohorts, and release proof; governance owns gate policy. Unknown
  consumers and unsupported tuples are recorded and removed from claims/output
  rather than silently retained.
- Deletion, consolidation, smaller-Interface, stronger-proof, and evidence-replacement alternatives:
  delete unsupported host packages and unimplemented Torch fields before adding
  machinery; replace deferred/manual desktop schemas with one projection;
  remove wrapper-specific launcher paths; consolidate release collection behind
  one artifact-plan Interface; replace source-text/native-only/fake-runtime
  checks with real consumer evidence; do not add pass-through adapters.
- Evidence-backed stopping condition: every owned reachable consumer has one
  accepted canonical input and disposition, every selected host/target/channel
  has claim-matched evidence, every unknown fact is typed `unavailable`, and no
  current output or public claim depends on an unclassified tuple or stale
  snapshot.
- Repaired-composition comparison: six deep Modules replace duplicated policy
  and ambient fallback; the Rust and frontend plans remain referenced owners,
  generated code remains an Adapter, and no second RPC/release/binding semantic
  registry or generic orchestration framework is added.

## Simplicity And Ownership Review

**Applicability:** `applicable`

- Independent concepts and dimensions: RPC shape/correlation, persisted library authority, stream lifecycle, launcher target/process behavior, Torch protocol/work scheduling, binding host/native compatibility, and release composition evolve independently and remain separate Modules.
- State, identity, value, time, policy, and mechanism: request and cohort identities, decoded values, subscription/process/inference time, accepted support policy, and generator/transport/package mechanisms are recorded separately so no mechanism selects policy.
  - Canonical authority scope and referenced authorities: Rust/server owns RPC
    semantics; accepted matrices own host/release promises; Torch owns its
    declared subset; launcher-root and launcher action contracts own their
    persisted/process state. Electron and CI only project or execute them.
  - Version roles and owned promises: product SemVer, RPC schema/protocol
    version, generator/tool version, native cohort identity, dependency
    resolution, and release-channel meaning are separate facts and are compared
    only where the accepted consumer contract requires equality.
  - Supported compatibility overlaps and consumer matrix: every release target
    and binding host/native tuple is explicit; desktop, Torch, bindings, and
    source/native artifacts do not inherit support from one another.
  - Material identity-invalidation effects: a DTO/schema, generator, Rust
    revision, target/profile, dependency lock, tag, or final-byte change
    invalidates only the projections/cohorts/evidence that name that identity.
- Caller and composition-root knowledge: Electron callers learn one decoded operation result, host callers learn one generated binding contract, launcher callers learn one action result, and release automation learns one artifact plan; generator dialects, process signals, paths, and SBOM tools remain hidden in their implementations.
- Representative change paths and forced owners: an RPC field change updates the Rust declaration then regenerates/decodes Electron; a host promotion updates the matrix then generator/test/package evidence; a release target change updates the artifact plan then target build/assembly/SBOM/docs; a Torch field changes its subset and work-owner tests without touching launcher or binding policy.
- Stable Interfaces versus hidden knowledge: stable Interfaces are decoded RPC outcomes, launcher-root state, owned stream transition, validated launcher action result, admitted Torch request result, binding cohort identity, and release-plan verification; file discovery, schema dialect, generator command lines, native suffixes, signal mechanics, and archive traversal stay hidden.
- Independent evolution, testing, failure, and replacement: each Module has focused tests through its Interface and can return its own typed failure; real OS, host, RPC, Torch, and release Adapters provide independent integration evidence without exposing test seams to callers.
- Necessary complexity and containment: generation, native-host loading, OS process control, model inference, and final-artifact provenance are inherent; each is contained behind the smallest existing ownership seam, with an internal test Adapter only where a real production/test variation exists.
- Deletion and cumulative machinery result: deleting any Module would push schema decoding, root recovery, process deadlines, inference lifecycle, cohort validation, or artifact knowledge into multiple callers, so each earns its Interface; unsupported host/output/field paths and superseded source-text/fake-runtime checks are removed rather than layered under new registries.

## Milestones

### Milestone 0: Establish Release and Host-Support Authority

**Goal:** Produce accepted, evidence-backed release and binding matrices that
make every later automation decision valid or explicitly unavailable.

**Development decision:** `investigate`. Consumer, distribution-channel,
publication, target, and legal-authority facts can materially change public
promises and expensive host/platform machinery. Stop when every current and
proposed artifact/host tuple has an accepted disposition and evidence
obligation or a typed `unavailable` owner; do not start a generator, package, or
release implementation while those facts are missing.

**Allowed write set:**

- `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md`
- `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/execution-ledger.md`
- `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/issues.md`
- `docs/plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/reports/release-and-host-contract-decision.md`
- `.gitignore`
- `scripts/release/artifact-plan.json`
- `bindings/support-matrix.json`

**Tasks:**

- [x] Inventory actual release consumers and distribution channels; distinguish
  desktop installers, Rust source/native/RPC artifacts, generated host sources,
  host packages, and optional Torch deployment by role rather than file suffix.
- [x] For each artifact, record release unit, consumer, channel, target,
  composition, version/tag relationship, promotion rule, compatibility promise,
  expected evidence, and missing/unexpected-output behavior.
- [x] For Python, Kotlin, Swift, Ruby, C#, Rustler/Elixir, and any currently
  documented Go surface, record host runtime, native OS/architecture/profile,
  generator/provisioning authority, supported operations, async/error/event
  behavior, package channel/tier, real-host oracle, and retain/remove/unavailable
  disposition. Generator availability is not support evidence.
- [x] Decide the release-channel and prerelease rule from product facts rather
  than `v0.*` syntax, and identify who may promote a draft.
- [x] Record the required resolved dependency graphs, SBOM/provenance,
  checksum, license, notice, vulnerability, and retention evidence per final
  artifact. Route unresolved license interpretation to the designated authority.
- [x] Record the accepted facts in `scripts/release/artifact-plan.json` and
  `bindings/support-matrix.json`; keep decision reasoning in the report and run
  the deletion/pass-through test before admitting parser or release machinery.
- [x] Cross-review the decisions with Rust, frontend, governance, and product/
  release owners; record contradictory facts as `invalid` and absent facts as
  `unavailable` in `issues.md`.

**Acceptance gate:** Reviewed decision report covers every current generated or
released output and every public consumer claim, with no support inferred from
incumbent workflow shape. The report fixes exact downstream Interfaces,
matrices, evidence environments, and write sets or blocks the affected tuple.

**Status:** `Accepted`

### Milestone 1: Generate and Enforce the Desktop RPC Contract

**Goal:** Replace permissive hand-maintained desktop transport shapes with one
generated projection and observable request, response, and event failures.

**Dependencies:** Rust plan acceptance of the canonical DTO/error declaration
and a supported projection mechanism; governance CI edits integrated before any
shared workflow change.

**Allowed write set:**

- `electron/src/generated/rpc-contract.ts`
- `electron/src/rpc-method-registry.ts`
- `electron/src/ipc-validation.ts`
- `electron/src/python-bridge.ts`
- `electron/src/preload.ts`
- `electron/tests/ipc-validation.test.mjs`
- `electron/tests/preload-rpc-contract.test.mjs`
- `electron/tests/python-bridge.test.mjs`
- `electron/tests/rpc-contract.integration.test.mjs`
- `electron/package.json`
- `scripts/generate-electron-rpc-contract.mjs`
- `package.json`
- this plan, ledger, issues, and `reports/rpc-contract-conformance.md`

**Tasks:**

- [ ] Consume the accepted Rust/server declaration without duplicating domain
  semantics; block with `unavailable` if it cannot express any required request,
  response, error, event, version, optionality, or unknown-field rule.
- [ ] Generate the TypeScript Adapter deterministically, identify its generator
  and canonical input, and add a freshness check that never hand-edits output.
- [ ] Replace deferred/generic request records with closed operation decoders
  and validate nested arrays, records, enums, optionality, and unknown fields.
- [ ] Decode HTTP status, envelope, correlation ID, result, and typed error before
  returning; decode every SSE event and deliver malformed/unknown/unsupported
  data as an observable typed outcome instead of logging and dropping it.
- [ ] Test positive and negative shapes through the public decoder Interface,
  delete superseded tests that only reconstruct implementation details, and add
  real RPC-to-Electron cases for malformed envelopes, wrong IDs/statuses,
  unknown variants, partial events, cancellation, and unavailable capability.
- [ ] Assert that credential-bearing inputs and redacted producer failures do
  not enter Electron logs or test diagnostics.

**Acceptance gate:** DRBT-A1 and DRBT-A2 are satisfied; Electron lint,
typecheck/build, focused decoder tests, generator freshness, and built-RPC
cross-process tests pass in their declared environments.

**Status:** `Active` — `2b081fba` accepts the selected catalog, FTS, download,
and ticket-recovery projection from canonical Rust declarations, with generated
standalone decoding, actual-producer conformance, and current-source freshness
in CI. Its accepted generator/output paths are documented in
[Electron's contract boundary](../../../../electron/README.md); they supersede
the proposed generator/output names above for that completed slice. Remaining
Legacy routes, transport envelopes, and event contracts are not accepted by
this checkpoint. DRBT-A1/A2 remain pending their complete gates.

### Milestone 2: Preserve Desktop Authority and Stream Lifecycle

**Goal:** Make persisted library selection and every desktop stream transition
complete, explicit, and safe across startup, renderer close, and shutdown.

**Dependencies:** Milestone 1 decoded failure/event representation; frontend
plan consumes but does not redefine recovery and stream outcomes.

**Allowed write set:**

- `electron/src/launcher-root.ts`
- `electron/src/launcher-root-recovery.ts`
- `electron/src/main.ts`
- `electron/src/startup-task.ts`
- `electron/src/preload.ts`
- `electron/src/window-presentation.ts`
- `electron/src/python-bridge.ts`
- `electron/tests/launcher-root.test.mjs`
- `electron/tests/launcher-root-recovery.test.mjs`
- `electron/tests/preload-rpc-contract.test.mjs`
- `electron/tests/window-presentation.test.mjs`
- `electron/tests/python-bridge.test.mjs`
- `electron/tests/desktop-lifecycle.integration.test.mjs`
- `electron/package.json`
- this plan, ledger, issues, and `reports/desktop-lifecycle-evidence.md`

**Tasks:**

- [ ] Express absent, valid, invalid, and unavailable launcher-root states at one
  Interface; prevent invalid/unreadable persisted state from invoking discovery
  or changing the library owner without an explicit recovery result.
- [ ] Replace direct persistence with same-directory atomic replacement,
  durability behavior selected for each accepted filesystem/OS contract, and
  permission/interruption diagnostics that do not expose sensitive paths.
- [ ] Give every serving, runtime, telemetry, and model-download stream one
  main-process owner with explicit capacity/ordering/drop semantics and observed
  subscribe, unsubscribe, delivery failure, renderer close, and shutdown results.
- [ ] Observe every preload/main promise locally, make close/unsubscribe
  idempotent, and remove reliance on process-global rejection logging as the
  lifecycle result.
- [ ] Test cold start, corrupt/truncated/unreadable state, interrupted update,
  duplicate transition, late event, transport rejection, renderer close, and
  shutdown through the owned Interfaces on every accepted OS.

**Acceptance gate:** DRBT-A3 and DRBT-A4 are satisfied; no startup case silently
switches library authority and no selected stream task can outlive shutdown
without an observed typed result.

**Status:** `Active` — selected recovery/bootstrap/presentation integration is
accepted in `2b081fba`, including the bundled preload and actual Linux GUI.
This supersedes the producer/renderer integration hold, not stream ownership
or required-real target filesystem and packaged-runtime evidence. DRBT-A3/A4
remain pending their complete gates.

### Milestone 3: Make the Torch Surface Truthful and Schedulable

**Goal:** Expose a bounded, validated inference contract whose real production
path cannot starve control traffic or lose accepted work at shutdown.

**Allowed write set:**

- `torch-server/openai_api.py`
- `torch-server/control_api.py`
- `torch-server/serve.py`
- `torch-server/model_manager.py`
- `torch-server/device_manager.py`
- `torch-server/validation.py`
- `torch-server/loaders/**`
- `torch-server/requirements.txt`
- `torch-server/requirements-dev.txt`
- `torch-server/tests/**`
- `torch-server/README.md`
- `.github/workflows/build.yml`
- `RELEASING.md`
- `docs/SECURITY.md`
- this plan, ledger, issues, and `reports/torch-runtime-evidence.md`

**Tasks:**

- [ ] Define the locally supported OpenAI-compatible request/response/stream
  subset, input/resource bounds, unknown-field policy, usage semantics, and
  invalid/unsupported/unavailable outcomes. Compare an authoritative external
  contract where compatibility is claimed; otherwise narrow the claim.
- [ ] Reject or implement every currently accepted role, prompt, sampling,
  `stop`, token, and streaming behavior; never accept and ignore a value or
  manufacture zero usage as success.
- [ ] Select one composition-owned text-inference work Interface after validating
  model, tokenizer, loader, and device threading/process constraints. Record
  admission capacity, overload, fairness, disconnect, result delivery, and
  bounded cleanup/shutdown before implementing it. Consume `TIPC-GEN-01` for
  generation lifetime: elapsed duration or silence never fails admitted text
  generation. Do not prescribe or implement a second lifetime policy here.
- [ ] Keep health and control routes schedulable during inference; make accepted
  work reach one observed result, failure, cancellation, or typed incomplete
  shutdown outcome, without detached work or an alternate runtime.
- [ ] Return redacted typed control/inference failures and distinguish configured
  next-start state from the effective listener/model/device state.
- [ ] Select and generate a reproducible production dependency resolution for
  every accepted Torch target; make development fakes explicitly lower-fidelity
  tests rather than substitutes for production-runtime evidence.
- [ ] Add real ASGI, middleware/auth, production loader, model generation,
  stream disconnect, overload, cancellation, dependency, and shutdown tests
  using the smallest accepted local artifact/device fixture and no network.
- [ ] Re-run the composed-design review before adding a worker process, queue,
  executor, or new package; re-plan if the selected model/runtime constraints
  require materially different composition.

**Acceptance gate:** DRBT-A5 is satisfied only after its broader text, usage,
responsiveness, overload, failure, and shutdown claims pass and TIPC-01–TIPC-12
are consumed for delegated generation lifetime and image-provider behavior.
Ruff/unit evidence remains green,
and a resolved real-runtime suite proves the supported subset and responsiveness
budget on each accepted runtime/device class.

**Status:** `Active`

### Milestone 4: Complete Launcher Platform and Process Outcomes

**Goal:** Route every accepted action consistently and bound child-process
termination without platform fallback or orphaned work.

**Allowed write set:**

- `launcher.sh`
- `launcher.ps1`
- `scripts/launcher/actions.mjs`
- `scripts/launcher/cli.mjs`
- `scripts/launcher/commands.mjs`
- `scripts/launcher/context.mjs`
- `scripts/launcher/contract.mjs`
- `scripts/launcher/errors.mjs`
- `scripts/launcher/platform-service.mjs`
- `scripts/launcher/platform-posix-process.mjs`
- `scripts/launcher/platform-linux.mjs`
- `scripts/launcher/platform-macos.mjs`
- `scripts/launcher/platform-windows.mjs`
- `scripts/launcher/actions.test.mjs`
- `scripts/launcher/commands.test.mjs`
- `scripts/launcher/wrappers.test.mjs`
- `package.json`
- `README.md`
- `docs/DEVELOPMENT.md`
- this plan, ledger, issues, and `reports/launcher-platform-evidence.md`

**Tasks:**

- [x] Consume the accepted OS/action matrix; return typed unsupported or
  unavailable outcomes for unknown target/platform facts instead of selecting
  Linux.
- [x] Remove wrapper-specific release behavior so Bash and PowerShell delegate
  the same parsed actions, validation, environment semantics, and exit codes to
  the shared launcher Module.
- [ ] Define process and process-tree ownership, normal completion, signal/
  console behavior, graceful deadline, escalation deadline, forced termination,
  partial cleanup, and diagnostic mapping separately for each accepted OS.
- [x] Implement bounded termination only for launcher-owned processes and
  observe `spawn`, `error`, `exit`, and `close` without double completion.
- [ ] Test wrapper equivalence, invalid platform/action/environment, spawn
  failure, graceful exit, ignored termination, forced kill, child trees,
  cancellation, and cleanup on each accepted OS with controlled fixtures.

**Acceptance gate:** DRBT-A6 is satisfied; launcher unit tests and required-real
OS wrapper/process suites pass with no unresolved child or false success.

**Status:** `Active`

### Milestone 5: Generate, Test, and Package Accepted Binding Cohorts

**Goal:** Turn the accepted host matrix into deterministic generated/native
cohorts that are promoted only after real consumers exercise them.

**Dependencies:** Milestone 0 host matrix; Rust plan acceptance of core/adapter
placement and Rust-side error/async semantics. Do not edit `pumas-core` or
re-own `pumas-uniffi/src/bindings.rs` semantics in this milestone.

**Allowed write set:**

- `bindings/**`
- `scripts/generate-bindings.sh`
- `scripts/check-uniffi-surface.sh`
- `scripts/check-uniffi-csharp-smoke.sh`
- `scripts/package-uniffi-csharp-artifacts.sh`
- `scripts/README.md`
- `rust/crates/pumas-uniffi/Cargo.toml`
- `rust/crates/pumas-uniffi/src/bin/uniffi_bindgen.rs`
- `rust/crates/pumas-uniffi/uniffi.toml`
- `.github/workflows/build.yml`
- `README.md`
- `RELEASING.md`
- `docs/native-bindings.md`
- this plan, ledger, issues, and `reports/binding-host-matrix.md`

**Tasks:**

- [ ] Remove generation, packaging, and public-doc paths for every unsupported
  host/target tuple; retain experimental outputs only when their non-release
  role and evidence limit are explicit.
- [ ] Pin accepted generator products and versions through declared toolchain
  inputs; provision them before generation and remove implicit `cargo install`,
  mutable tag, ambient PATH, and source-tree output behavior.
- [ ] Make generated wrapper/native metadata identify the Rust revision,
  product/interface version, generator, target, profile, and checksums needed to
  reject cohort mismatch before load or packaging.
- [ ] Generate into a clean staging/build location, detect stale/unexpected
  output, and preserve the Rust Adapter as the only binding conversion/error/
  lifecycle semantic owner.
- [ ] For every accepted tuple, run the real host against the staged native
  library and exercise discovery/load, one representative value round trip,
  invalid/unsupported/unavailable errors, and selected async completion,
  cancellation, event, and shutdown behavior. Extend C# beyond textual and
  synchronous-only smoke if it remains accepted.
- [ ] Package exactly the host/native tuples selected by the matrix, verify the
  archive after extraction, and emit cohort evidence consumed by release
  assembly. A missing host runtime blocks that tuple rather than weakening it.

**Acceptance gate:** DRBT-A7 is satisfied for every accepted tuple; generated
freshness, native Adapter checks, real-host suites, extracted-package tests, and
cohort comparison pass independently.

**Status:** `Planned`

### Milestone 6: Assemble and Prove the Exact Release

**Goal:** Make a dry-run/draft release a deterministic projection of the
accepted release plan, with evidence derived from the final promoted bytes.

**Dependencies:** Milestones 0 through 5, accepted Rust/frontend build inputs,
and governance ownership of permanent gate schedules.

**Allowed write set:**

- `.github/workflows/build.yml`
- `scripts/release/artifact-plan.json`
- `scripts/release/artifacts.mjs`
- `scripts/release/artifacts.test.mjs`
- `scripts/dev/check-release-version-alignment.mjs`
- `scripts/package-uniffi-csharp-artifacts.sh`
- `package.json`
- `electron/package.json`
- `README.md`
- `RELEASING.md`
- `docs/SECURITY.md`
- `docs/native-bindings.md`
- `bindings/README.md`
- `torch-server/README.md`
- this plan, ledger, issues, `reports/release-evidence.md`, and
  `reports/final-acceptance.md`

**Tasks:**

- [ ] Make local validation and CI consume the same release plan for required
  inputs, artifact names/roles, target composition, binding cohorts, versions,
  channel/prerelease policy, and missing/unexpected-output rejection.
- [ ] Require every accepted Rust/RPC/renderer/native/Torch/package input rather
  than ignoring absence, and inspect packaged desktop/binding artifacts after
  extraction for the exact target-matched contents.
- [ ] Compare tag, root/frontend/electron/Cargo/package versions and schema/
  cohort versions according to their distinct accepted roles; do not infer
  prerelease state from a `v0.*` prefix.
- [ ] Resolve the shipped dependency closure per final artifact, produce SBOM
  and provenance with pinned tools, scan current dependencies on the accepted
  schedule, and derive required license/notices material with designated review.
- [ ] Generate checksums only after final names and bytes are immutable; verify
  that SBOM, notices, and checksum coverage are complete and contain neither
  obsolete nor build-only dependencies unless explicitly required.
- [ ] Execute release dry runs on every accepted packaging target and final
  assembly environment, record toolchains and immutable artifact identities,
  and keep publication disabled until all objective claims are satisfied.
- [ ] Reconcile canonical release, security, binding, Torch, launcher, and root
  documentation to the accepted contracts; do not rewrite historical audits.
- [ ] Run all DRBT-A1 through DRBT-A9 gates, cross-review the final interfaces
  with the Rust/frontend/governance plans, and move the plan through
  `Implemented` and `Verifying` to `Accepted` only on adequate evidence.

**Acceptance gate:** DRBT-A1 through DRBT-A9 are satisfied and linked from the
final report; final release bytes, extracted contents, checksums, SBOMs,
licenses/notices, target evidence, and durable docs all agree with one accepted
artifact plan.

**Status:** `Planned`

## Blockers

- Milestone 0 is accepted; the report and canonical matrices agree and are
  visible versioned inputs.
- Milestone 1's selected generation prerequisite is resolved by `2b081fba`.
  Unselected Legacy routes and handler-owned events still require canonical
  producer contracts and complete transport evidence; their absence does not
  invalidate the accepted selected projection or block independent lifecycle
  investigation.
- Required-real Windows/macOS runtime evidence remains pending. Existing CI
  configuration or cross-build success is neither runtime proof nor evidence
  of failure; collect matching runner results before accepting those claims.
- Any binding host, desktop target, Torch device/runtime class, or release
  channel without a real consumer and adequate environment remains blocked or
  removed from the accepted matrix.
- The repository maintainer owns license/notice interpretation; missing or
  contradictory final-artifact notice evidence still blocks promotion.
- Shared CI edits remain blocked until the governance plan's earlier CI write
  set is integrated or explicit serial ownership is recorded.

## Re-Plan Triggers

- The Rust contract cannot project the complete request/response/error/event
  representation, or its version/unknown-field policy changes materially.
- Milestone 0 selects different release-plan paths or a materially different
  distribution unit, channel, host matrix, target matrix, or publication role.
- A claimed host or target lacks required-real execution capability; do not
  weaken the claim without product-owner acceptance.
- Real model/runtime evidence disproves safe thread/executor use or requires a
  worker process, queue, alternate deployment, or materially different shutdown
  composition.
- Atomic persistence or forced process-tree termination has different required
  semantics on an accepted OS than the common Interface can express.
- Final artifact dependency closure requires target-specific lock/resolver or
  SBOM mechanisms not represented in the accepted artifact plan.
- Another reachable consumer or public promise expands the systemic population.
- A proposed Adapter becomes pass-through, caller knowledge increases, or
  cumulative generator/manifest/orchestration machinery exceeds the repaired
  composition.

## Cross-Plan Integration

Implementation is serial at shared write sets; no concurrent revision is
admitted by this plan.

| Owner | Primary output consumed here | Shared or forbidden surface | Integration order |
| --- | --- | --- | --- |
| Governance and verification plan | Current CI/gate ownership and schedules | `.github/workflows/build.yml`, package scripts, `docs/DEVELOPMENT.md` | Integrate its relevant CI cleanup before this plan edits shared workflow registrations |
| Rust library and RPC plan | Canonical DTO/error declaration, Rust-side lifecycle/error semantics, core/Adapter placement | This plan must not edit `pumas-core` or re-own semantic code in `pumas-uniffi/src/bindings.rs` | Accept canonical contract before Milestone 1; accept binding semantics before Milestone 5 |
| Frontend and UI plan | Renderer use of decoded/recovery outcomes and library-only UI behavior | This plan stops at preload/main decoded Interfaces | Integrate generated outcome Interface before renderer adoption; include the adopted path in final system review |
| [Torch Image Provider Contract Correction](../../torch-image-provider-contract/plan.md) | Accepted shared generation lifetime, image live compatibility, result/projection, and exact-candidate handoff evidence | It solely owns generation duration policy and the image correction; this plan must not duplicate either implementation | Consume TIPC evidence when evaluating generation/image portions of DRBT-A5; text shape/usage work may proceed independently |
| This plan | Desktop projection/decoding, launcher/Torch lifecycle, host cohorts, release artifact proof | Owns the write sets above after prerequisites | Integrates last at shared release/CI surfaces |

## Final Acceptance

- Acceptance status: `pending`
- Deferred follow-ups: `none`; unsupported or unavailable hosts, targets,
  channels, and compatibility features are explicit contract outcomes, not
  deferred acceptance debt.
- Final status: `Active`
