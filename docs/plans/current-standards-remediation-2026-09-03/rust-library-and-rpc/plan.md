# Plan: Rust Library and RPC Standards Remediation

**Plan status:** `Active`

**Current phase:** Milestone 1 is accepted. Milestone 2's C3 core recovery,
cancellation-quarantine, network/queued pause, current-only cutover, and stable
persisted library identity checkpoints
are verified for incremental integration. Old partial relocation is retired.
Full C3 remains active and unaccepted. Milestone 2 remains producer
contract work that is valid for loopback desktop RPC and local core IPC.
`RUST-I1` is resolved: desktop RPC is loopback-only and LAN support is removed.

Selected reconciliation ownership, desktop catalog/download/ticket contracts,
and Linux artifact-preserving collision remediation are accepted at `f77f4bed`,
`2b081fba`, and `2b9553a0`. Desktop callers have migrated together; remaining
core/UniFFI ambient adapters do not reintroduce the removed desktop methods.
HF-unavailable regressions now pass, but complete M2J and all-route contract
acceptance remain open. RUST-I12 is resolved on Linux, not pending source work.

The Verified Ambient pre-settlement restore checkpoint is accepted in
`hf/download.rs` after focused review, dual core suites, and strict lint. Its regression
failed before the fix and now proves successful restore, read-only Error
history, successor progress, and retained settlement after caller cancellation.
Initial and returned inventory both reject unresolved custody; no filesystem
cleanup replay or new admission is inferred.

The matching Verified Recovery settlement is accepted after review and gates.
Strict store validation binds its same-ID cleanup to durable revocation and
the retained admission attempt. The regression proves exact settlement,
preserved hidden history/tombstone, follower progress, and repeat restore
without recreating execution authority. Pending and unquarantined Recovery
still refuse with unchanged custody and payload.

Capability-relative partial-file and marker deletion now syncs the exact held
parent before success, including an already-absent entry retry. The bounded
`download_recovery.rs` correction is accepted after review, dual package tests,
strict lint, and formatting. Callers retain their existing Interface.

Phase-aware cancellation preparation is accepted after independent review,
dual core suites, strict lint, and formatting. The existing store Interface
validates exact retained ownership and returns its confirmed cleanup phase;
terminal retries skip deletion, and missing admitted snapshots refuse effects.

Terminal intent restart confirmation is accepted after independent review,
dual core suites, strict lint, and formatting. Store reconciliation promotes
only VerifiedIntent through its durable barrier; existing restore settles the
exact queue entry without file deletion. Pending and publication-failure refusal
remain verified. Production HF and supported store format are unchanged.

Physical-root exclusion is accepted for the bounded Linux download population.
Active mutation phases share a grant; idle/search clients and queued waits do
not pin it. RUST-I13 retains the broader teardown and Pending replay limits.

Cancellation predecessor custody is accepted after independent review and dual
core gates. A retained observer keeps terminal observation incomplete until
predecessor effects drain; inherited failures remain observable without failing
successful new cleanup. Failed observation cannot synthesize absence.

Explicit download shutdown is accepted for the admitted Linux population:
public Rust and IPC preparation, retained task/effect drainage, final interrupted
state projection, and the RPC supervisor's shared result. Independent review,
dual core/RPC suites, strict affected lint, and SD-1 through SD-4 pass. That
checkpoint alone does not accept importer completion or whole-runtime teardown.

C4 awaited managed-download importer finalization is now accepted under the
[importer admission](#awaited-download-importer-admission). Real metadata/index
success precedes settlement, failures retain retry custody, and notifications
follow logical release. C4-1 through C4-4 and dual core/RPC and lint gates pass.

The [bounded grant](#physical-root-execution-grant-admission) passes G-1 through
G-4, independent reviews, dual core/RPC suites, strict lint and desktop diagnostic
conformance. The execution ledger records consumer corrections and exact limits.

**Next slice:** Admit the exact Pending cleanup replay contract against current
durable authority and retained root custody before implementing replay. Shutdown,
managed importer and grant prerequisites are accepted. Pending replay, relocation,
and whole-runtime teardown remain unaccepted; this is not C3/M4 completion.

**User-prioritized interruption accepted:** the canonical nullable library-model
association for download progress under the frontend plan's
[bug admission](../frontend-and-ui/plan.md#download-row-association-bug-admission).
The core destination capability owns this identity; RPC list/status/SSE project
it through one progress DTO and generated consumers migrated atomically. The
[execution ledger](execution-ledger.md#2026-09-05--exact-download-library-identity-projection-accepted)
records verification. This adds no persisted authority and does not begin
Pending cleanup replay.

## Physical-Root Execution Grant Admission

**Outcome:** independent cooperating HF clients cannot overlap protected download
mutation on the same physical library root. One client's active mutation phases
share exclusion, allowing its different destinations to progress concurrently.
This is advisory download-engine exclusion, not a lock against unrelated model
imports, catalog operations, external writers, or every library API.

**Mechanism:** `DownloadDestinationRoot` owns physical/logical identity checks
and a fresh readable capability-relative root open (`root.open(".")`), directory
verification, and nonblocking `fs2::FileExt::try_lock_exclusive`. Revalidate the
held physical identity and logical library UUID before and after acquisition.
Do not use `try_clone` as an independent contender: it shares an open description.
`open_dir(".")` is also unsuitable on the tested Linux adapter: its handle yields
EBADF when locked. Do not create a lock sidecar or fall back to ambient paths,
blocking acquisition, an in-process-only lock, or a different identity.
Keep existing identity-initialization locking separate and unchanged.

**Lifetime:** the existing `DownloadTaskOwner` weakly caches the live grant by
physical identity. Configured roots, idle clients, historical task receipts, and
persisted admissions do not strongly retain it. Acquire lazily before protected
work, outside synchronous guards, with acquisition and closure under existing
invocation custody. Same-client acquisition must join the same live grant rather
than race independent opens into false contention. Independently opened recovery
capabilities may join only after proving the same configured physical root.

Strong custody belongs to bounded mutation phases and their retained observation,
not merely to `TaskEntry` or an abortable work future. Actual blocking closures,
async import work, and their observers retain custody through completion and
failure recording. Transfer custody to an executing cancellation/projection
successor before predecessor release; no unlock/reacquire gap across active work.
Use the existing owned-effect machinery to distinguish abortable work from its
retained observation; no separate task registry or grant-release reaper.
Prepared mutation work remains protected through abandonment/drain, but a worker
waiting for destination eligibility must release its admission-phase custody
after effects are observed. Paused heads with queued followers must permit idle
handoff. Reacquire and revalidate before that worker starts destination effects.
Finished-but-unpolled entries must not pin the grant. Required terminal projection
is protected; public completion notification follows this task's grant release
as well as destination release, while shutdown still observes the notification.

**Protected consumer population:**

| Entry/owner | Required disposition |
| --- | --- |
| Shared core `api/hf.rs` start and ticket recovery, reached by public Rust and desktop RPC | Acquire before artifact destination preparation or index refresh; retain into immediate mutation work. Current local IPC exposes no download mutation route. |
| Direct HF start, resume, pause, cancellation and recovery admission | Protect durable admission/status/quarantine and destination effects, including operations without a worker; preserve existing recovery authority. |
| Restore and byte-complete finalization | Protect strict inventory reconciliation, Verified settlement, importer and terminal settlement even when no worker remains. Pending replay stays refused. |
| Worker/cancellation/projection phases | Retain through real file/metadata/index/store effects and observation; active handoffs cannot create a gap. |
| Progress/list/snapshot reconciliation | Acquire on demand for mutation; otherwise return only the observed runtime projection. |
| Inspection, search, idle client and no-root reads | No grant retained; no new root requirement for read-only behavior. |
| Explicit shutdown | Drain protected phases and transfers before releasing their custody; runtime-only final projection requires no invented filesystem authority. |

For existing-admission execution, validate current destination authority, exact
retained admission attempt/domain/destination/bound files, revocation state, and
applicable durable queue eligibility **before** the first destination effect.
Pre-admission destination preparation and ticket index refresh retain their
existing request/root/capability authority under the grant; this adds no new
payload, relocation, marker or cleanup authority where no admission yet exists.
Use the store's canonical policy, not copied HF inventory interpretation. Current
runtime generation/reservation checks remain necessary but are not durable proof
after idle handoff. Cancellation uses its quarantine/cleanup authority, not an
ordinary worker's execution permission. A stale client cannot restore its old
snapshot over another client's settled or revoked custody.

**Failure/read contract:** add core `DownloadRootBusy` for real contention, mapped
to existing RPC `-32011` conflict and partial-action `download_root_busy`. Do not
classify open, identity, or unsupported-lock errors as busy. Mutating Result
operations refuse without protected effects; no automatic retry is added. Restore
is a mutating Result operation, so a busy required restore fails startup rather
than pretending to have restored an empty inventory. Existing Option/Vec/snapshot
reads may return their last observed runtime projection without reconciliation
when busy; they promise neither fresh durable inventory nor successful repair.
Preserve existing no-root read behavior and non-busy failure handling. IPC already
preserves the wire conflict category but converts it back to `PumasError::Other`;
do not claim a new typed round-trip or introduce a protocol discriminator here.
The closed desktop partial-action reason enum must admit `download_root_busy`
and its generated Electron/frontend validators must migrate together; otherwise
the core outcome becomes an invalid-domain error at the RPC adapter. This is an
additive diagnostic contract change, not a new operation or persistence schema.
The existing UniFFI exhaustive conversion maps this busy outcome to Config,
consistent with its current busy errors; no FFI schema or public lock handle.

**Implementation ownership/write set:** root serially integrates shared contracts
and plans. Capability owner: `rust/crates/pumas-core/src/model_library/download_recovery.rs`.
Lifecycle owner: `src/model_library/hf/lifecycle.rs`. HF integration owner:
`src/model_library/hf/{mod,download,types}.rs` and `src/api/hf.rs` (all relative to
`rust/crates/pumas-core`). Store owner may add a canonical read-only eligibility
operation in `src/model_library/download_store.rs` if current operations cannot
express the existing policy without duplication. Root owns `src/error.rs`,
`src/tests.rs`, `src/ipc/protocol.rs`, `rust/crates/pumas-rpc/src/contract.rs`, and
`rust/crates/pumas-uniffi/src/bindings.rs` for diagnostic/consumer integration.
`src/model_library/mod.rs` may re-export the root capability under `cfg(test)`
for core consumer tests without expanding its production visibility.
Tests stay in these existing owners; README/Rustdoc and parent/Rust plan, issue,
and ledger records are allowed. Diagnostic integration also owns the existing
RPC contract export fixtures and all six `desktop-contract` generated artifacts
in `electron/src/generated` and `frontend/src/generated`, plus their existing
Electron/frontend conformance consumers to exercise the new busy fixture; regenerate through
`electron/scripts/generate-desktop-contract.mjs`, never hand-edit them. No
Cargo/dependency, persistence schema, live data, UI controls,
relocation, general importer, or Pending replay changes. Interface coordination,
Cargo gates, staged review and commits remain serial; workers escalate ownership
or contract conflicts rather than edit shared authority independently.

**Evidence** (accepted for the bounded slice; automated Linux representative filesystem, real child
processes for G-1; current source supports Unix root capabilities only):

- `G-1`, system: independent clients/processes using the actual root adapter
  contend across alias paths and different destinations; release after normal
  completion and process death permits a fresh contender. Root replacement and
  changed UUID refuse without touching replacement payload.
- `G-2`, integration: same-client destinations overlap; idle/search, paused with
  queued followers, and finished-but-unpolled clients do not pin exclusion.
  A stale client reacquires after handoff and refuses revoked/settled authority.
- `G-3`, integration/system: hold real pre-start destination/metadata work, final
  importer and cancellation cleanup; caller/client drop, shutdown and active
  cancellation transfer cannot let an independent contender acquire early.
  Include abandoned prepared work and success/error/panic observation.
- `G-4`, contract/integration: public Rust ticket recovery refuses busy before
  effects; ordinary start's pre-mutation acquisition ordering receives source
  review plus direct HF coverage (its public producer first requires remote HF
  metadata; no new offline seam is admitted). Restore and read reconciliation follow their distinct contracts;
  busy, unsupported/I/O and stale identity remain distinguishable. RPC/IPC wire
  conflict and existing UniFFI conversion are tested. Local IPC evidence covers
  its error adapter only, not an unexposed download operation. The partial-action
  busy reason must pass RPC projection and generated consumer conformance.

Reuse the existing store subprocess fixture pattern (current test executable,
exact ignored helper, pipe handshake), not a new test framework. Final gates:
both core/RPC configurations, focused binding conversion, affected strict
all-target Clippy, formatting and five plan contracts. The diagnostic contract
also requires generation freshness, both desktop conformance consumers, and
Electron/frontend TypeScript checks. Linux proves only its
tested filesystem semantics; Windows/macOS runtime and hard-process Pending
recovery remain unaccepted.

**Composed-design review: applicable.** Capability owns where/what is locked and
native identity; lifecycle owns when custody starts, transfers and ends; store
owns which durable admission authorizes an effect. Their reasons for change are
independent. Required order is grant, current authority, effects, observation,
release; client lifetime and destination queue waiting are accidental lock
lifetimes. Callers keep existing operations plus a busy result, not lock handles
or OS details; builder only supplies the configured root. A locking-adapter change
touches capability, a custody change lifecycle/HF, and a queue-policy change store
plus its typed consumer. Dependencies carry concrete capability/custody Interfaces,
not raw descriptors or copied durable layouts. Each owner has focused evidence;
cross-process and actual-effect tests prove their composition. Deleting the grant
would spread physical exclusion policy across consumers; deleting the weak cache
would break same-client reuse. No new generic adapter, scheduler, persistence
format or registry is justified. Necessary native exclusion and asynchronous
custody remain localized in existing owners, with phase scope preventing idle
pinning and retained observation preventing early release.

**Development decision:** implement. Two bounded traces identify the reachable
consumer and custody population. The real dependency probe resolves readable
handle selection; no further exploratory prerequisite is needed. Re-plan only
if implementation exposes an unowned protected effect, incompatible consumer
promise, unsupported required filesystem behavior, or inability to preserve both
idle handoff and active custody. Do not enable Pending replay merely because
the grant is present.

## Awaited Download Importer Admission

**Outcome and ownership:** configured HF clients await real auxiliary metadata
and final importer work before terminal settlement. The builder injects a
private concrete `ModelImporter` dependency, not an async public notification
callback or a new task scheduler. The existing lifecycle context retains the
actual async importer future through cancellation/shutdown, including its
internal filesystem and index work. Drop physical destination guards before
import; retain logical destination custody through import and settlement.
Public notification callbacks remain synchronous, owner-observed, and separate
from required mutation. Completion notifications follow logical release; a
held notification must not block a same-destination successor.
Only existing managed ordinary/restore paths gain importer ownership. Verified
Recovery-domain ticket tasks retain their no-import/no-callback policy; this
slice grants no extra ambient metadata authority to recovery capabilities.

**Failure and restoration:** strict download finalization atomically replaces
metadata through its existing writer without deleting the old file first, and
requires successful indexing. All download-finalization branches reject false
success, including metadata fast paths and Diffusers imports. Generic imports
retain their existing separately scoped contract. Failure/panic/cancellation
cannot produce Completed or settle admission prematurely; files and exact
durable admission remain available for the existing resume/restart path. No
new retry protocol or store format is introduced. Byte-complete restore uses
the same awaited importer before settlement, still returns completion records,
and does not newly invoke public callbacks. Operational import failure remains
a tracked recoverable Error, not global startup failure; corrupt authoritative
inventory still prevents startup. Remove builder's post-settlement import loop
and importer-enqueueing callbacks rather than retaining a second implementation.

**Write set and integration:** HF owner writes inline tests and
`rust/crates/pumas-core/src/model_library/hf/{download,mod,types}.rs`; importer
owner writes `rust/crates/pumas-core/src/model_library/importer.rs` and its inline
tests. Consumer reviewer owns builder regressions in the existing
`rust/crates/pumas-core/src/tests.rs`; root owns
`rust/crates/pumas-core/src/api/builder.rs` and contract documentation in
`rust/crates/pumas-core/README.md` and the shutdown Rustdoc in `src/api/hf.rs`,
plus these plan/issue/ledger records and the
parent plan/ledger. Shared interfaces, Cargo and commits integrate serially.
No lifecycle scheduler rewrite, dependency, schema, live-data, RPC/UI, generic
orphan import, relocation, physical-root grant, or Pending replay changes.

**Evidence** (accepted; automated, representative Linux; see execution ledger):

- `C4-1`, integration: real importer metadata/index success precedes ordinary
  Completed and durable settlement; held import prevents successor effects.
- `C4-2`, integration: cancellation and shutdown retain actual held importer
  work; its error/panic remains observable and cleanup cannot overtake it.
- `C4-3`, contract/integration: metadata/index failure and false import success
  preserve recoverable state and payload; byte-complete public restore retries
  successfully, reports completion once, and leaves a usable built API.
- `C4-4`, integration: after settlement, a real same-destination successor
  progresses while notification is held; notification panic cannot roll back
  Completed and shutdown still observes notification work.

Supporting gates are focused regressions, both full core configurations,
affected RPC consumer tests, strict affected all-targets Clippy, workspace
formatting and plan validation. No Windows/macOS runtime or new GUI claim.

**Composed-design review: applicable.** Importer owns metadata/index policy;
HF owns lifecycle ordering; builder only supplies the concrete dependency.
Required interleaving is import, settlement, release, notification. Enqueueing
and post-settlement mutation are accidental. Callers retain existing download
and callback Interfaces; they do not acquire task registries or index policy.
An import rule changes importer, a terminal-ordering rule changes HF, and
configuration changes builder. One private injected dependency carries an
owned Interface rather than representation knowledge; no generic adapter is
justified. Importer failure tests remain independently executable, and real HF
tests prove composition. Deleting the shared import path would spread indexing
success and ordering knowledge back across ordinary/restore callers. Necessary
mutation lifetime stays in the existing lifecycle owner; no extra scheduler,
public callback protocol, or parallel completion authority is retained.

**Development decision:** implement. Source tracing bounds all production
import consumers to the two builder paths plus auxiliary metadata. The strict
import mode and existing owner permit a reversible coherent correction.
Re-plan on missing recovery authority, an importer effect escaping observation,
or a new independently owned consumer; do not expand into general importer or
runtime remediation merely because adjacent code remains imperfect.

## Explicit Download Shutdown Admission

**Outcome:** `PumasApi::shutdown_downloads(&self) -> Result<()>` and the HF
equivalent close download admission permanently and observe owned work through
completion. Repeated waiters receive the same retained result; cancellation of
one waiter cannot cancel the drain. No configured HF client means nothing to
drain, not an initialization failure. Expected shutdown aborts are not failures;
unexpected task, effect, or observation failures remain bounded diagnostics.
Shutdown preserves recovery data; it does not authorize cancellation deletion,
Pending replay, queue release, importer completion, or whole-runtime shutdown.

**Owned population and coordination:**

- Use one lifecycle coordination contract for public invocation admission,
  prepared/installed/retired task custody, transfers, closing, and the retained
  result. Consolidate related registries behind owner state rather than adding
  an independent closed flag beside separately sampled maps. Registration and
  ownership transfer are atomic with closure. Gated Tokio observers may be
  spawned and registered under that guard because spawning does not poll them;
  gate release, effects, callbacks, and joins occur outside synchronous guards.
- Include start, restore, recovery transitions/admission, pause/resume/cancel,
  and reconciliation triggered by progress/list/snapshot reads. Admission starts
  before the first effect, including core `start_hf_download` destination
  preparation. Keep existing artifact-selection policy; move its blocking work
  off the async path and retain its actual join under the invocation owner.
- Already-running preparation remains owned; late task handoffs after closure
  refuse without inferring rollback of durable admission. A permit held only by
  the caller future is insufficient: its blocking closures and observers must
  remain owned after caller cancellation. No new handoff may escape the closing
  owner. Search and metadata access remain outside download shutdown.
- After closure, mutation methods return an explicit lifecycle-closed error.
  Progress/list/snapshot reads retain their existing shapes without launching
  reconciliation. After admitted work and projectors drain, shutdown projects
  interrupted active states to existing Error with a bounded shutdown-interrupted
  reason before resolving its receipt. Preserve already paused/terminal states
  and durable provenance. Expected interruption is not a drain failure; do not
  infer Paused, Completed, queue release, or cleanup from abort alone. Reads
  during closing remain last-observed state until that final projection.
- Replace untyped start senders with private Work/Custody gates, preserving
  their kind across predecessor transfers. Separate execution from observation.
  Abort execution, start required observers, settle affected projection receipts
  as an explicit shutdown outcome after entry drain, and join every retained
  effect. Preserve already-terminal receipt outcomes; never acknowledge failure
  as projected merely because shutdown observed it. Do not start cleanup merely
  to drain its predecessor or wait indefinitely for a paused destination head.
  Archive retired failures instead of discarding them during reaping.
- The shutdown driver retains production ownership and publishes one shared
  completion result. Merely spawning a detached Drop drain, holding a test-only
  strong reference, or joining an outer task does not satisfy this contract.
  Synchronous Drop is still request-only; runtime destruction is not drainage.

**Consumer:** retain `Arc<AppState>` in the RPC supervisor until HF and catalog
drains finish. Start both drains and observe both regardless of listener or
drain errors; preserve labelled failures instead of chaining `Result::and`.
Existing `ServerHandle` keeps drain execution alive after waiter cancellation,
but its consumed join handle does not retain an observable result. Give repeated
RPC shutdown waiters a shared receipt too; Drop requests shutdown without
claiming completion. No new RPC endpoint, frontend method, or plugin shutdown.

**Source/write ownership:** lifecycle owner writes
`rust/crates/pumas-core/src/model_library/hf/{lifecycle,download,mod}.rs` and inline
tests. Root integrates `rust/crates/pumas-core/src/api/hf.rs`, lifecycle errors in
`rust/crates/pumas-core/src/error.rs`, and their existing consumer projections.
The HF owner consolidates duplicate download entry points in
`rust/crates/pumas-core/src/api/state_hf.rs` onto those shared owned functions;
local IPC must not retain an independent pre-admission implementation.
RPC owner writes `rust/crates/pumas-rpc/src/server.rs` and inline tests; adjacent
`main.rs`, `contract.rs`, and `catalog_projection.rs` changes are limited to this
shutdown/result contract. The existing test-only
`rust/crates/pumas-rpc/src/handlers/test_support.rs` may share its API setup guard
with HF-enabled server fixtures: handler fixtures temporarily override the
process-wide registry path, so independent server construction is not isolated.
Its declaration in `handlers/mod.rs` may become crate-visible under `cfg(test)`
for those sibling fixtures, without changing production visibility.
Keep the real API/HF/server owners and concurrency assertions; no retry, new
global test lock, whole-test serialization, or production registry change.
The existing exhaustive conversion in
`rust/crates/pumas-uniffi/src/bindings.rs` may adapt errors without adding a host
shutdown surface. Root alone owns these plans/ledgers/issues, Cargo, and commits.
Root may extend `rust/crates/pumas-core/src/model_library/test_support.rs` under
its existing explicit fixture feature so RPC tests can hold a real HF-owned
blocking operation. This is not a production testing endpoint or new scheduler.
The existing `model_library/mod.rs` may re-export the invocation context within
the crate for core recovery preparation; it is not a public library export.
`rust/crates/pumas-core/Cargo.toml` may enable Tokio's existing multithread runtime
for dev-only fixtures that must acknowledge request cancellation while another
task is held at a synchronous projection hook; production features are unchanged.
No library relocation-policy, store/schema, importer, or live-data edits.

**Required evidence** (accepted 2026-09-05; automated, representative Linux;
commands and limitations in the execution ledger):

- `SD-1`, focused: closure versus prepared installation, retired transfer, and
  gated finalizer/projection; no missed effect, new execution, or stranded receipt.
- `SD-2`, integration: real held blocking preparation/write across shutdown and
  caller cancellation; success/error/panic drains before the shared result.
  Release test strong references and prove production ownership through a weak
  reference after explicit shutdown has been requested.
- `SD-3`, contract: repeated/cancelled shutdown waiters, no-client success,
  explicit closed mutation errors, and non-reconciling read snapshots whose
  final interruption projection cannot be overwritten by late projectors.
- `SD-4`, integration: real RPC supervisor waits for both HF and catalog owners;
  held work prevents completion, either/both failures survive, and cancelling a
  waiter does not lose drainage or the result available to another waiter.

Supporting gates: focused regressions, both core/RPC feature configurations,
strict affected all-targets Clippy, workspace formatting, error-conversion checks,
and all plan contracts. Windows/macOS runtime evidence remains unavailable.

**Composed-design review: applicable.** Invocation/task/effect lifetime belongs
to the HF lifecycle Module; download recovery policy stays in its existing
owner, and RPC owns process teardown. Admission-to-custody transfer and receipt
completion are required interleavings; separate-map gaps and read-triggered
post-close work are accidental. Callers know only admission failure and awaited
shutdown, not registry or gate layout. A new task role changes lifecycle tests;
a recovery rule changes download policy; public Rust and IPC download entry
points delegate the same owned implementation rather than duplicate it;
RPC error aggregation changes only
teardown. Core passes a lifecycle Interface, not maps or task handles. These
concerns can be tested independently, while the real supervisor path proves
composition. Deleting retained admission/receipt machinery would push joins,
closure races, and cancellation handling into callers; no new generic scheduler,
registry mirror, trait hierarchy, or dependency is justified. Necessary lifetime
coordination replaces scattered tracking inside the existing owner.

**Development decision:** accepted for incremental integration. The bounded
implementation and its required evidence are complete. Re-plan only if
a reachable effect falls outside the owned invocation/task population, a caller
cannot preserve the declared error/read contract, or completion requires C4
importer ownership. No full C3/M4 or root-lease acceptance is inferred.

Pending replay remains excluded. Investigation found three prerequisites:
cross-client/process exclusion through cleanup drain; distinguishing persisted
intent phases currently projected together as Pending; and parent-sync deletion
durability. Parent-sync durability, phase-aware cancellation preparation,
and terminal intent restart confirmation are accepted. Execution exclusion now
depends on bounded M4 shutdown and C4 importer coordination, rather than merely
adding a lock. Preserve fresh-client
refusal while an earlier cleanup is held. No snapshot-derived deletion/resume,
live library mutation, legacy support, or C4 importer change is admitted.

The current-only cutover and queued pause remain accepted. Full C3, admitted
relocation, guard-free effects, C4 awaited importer integration, and hard-process
crash evidence remain open. Slice B generation, terminal, cleanup, tombstone,
and sticky-failure semantics remain invariants. General client-drop draining
remains M4/RUST-A6; Slices D-E and wider source changes require their own admission.
Unavailable Windows/macOS runtime evidence does not block this Linux-capable
checkpoint or imply a failed GitHub Actions result.

**Acceptance status:** `partial`

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Reports:** Pending report slots are listed in the
[execution ledger](execution-ledger.md#reports).

**Audit source:** Rust library and RPC (`docs/audits/current-standards-2026-09-03/rust-library-and-rpc.md`, removed; see Git history at `0a4c0a8e`)

## Active PRG-I19 Slice C Boundary

### Checkpoints after the implementation audit

| Checkpoint | Owned result | Acceptance evidence |
| --- | --- | --- |
| C1 (internally verified) | Durable admission, restart reconciliation, quarantine, and exact queue settlement behind the store Interface | 38 store tests and atomic/HF regressions pass in both feature configurations; independent source review accepted |
| C2 (internally verified) | Held configured-root destination authority and marker publisher ready for runtime integration | Independent source review accepted; root reproduced 14 recovery, 22 atomic-publication, 38 store, and 155 HF tests in both feature configurations |
| C3 (active; incremental core checkpoint verified) | Start, pause, resume, cancellation, restore, and admitted relocation consume the store and destination Interfaces | Ticket recovery, cancellation quarantine, network/queued pause, and current-only cutover pass recorded review, dual package tests, and strict lint. Old partial relocation is retired. Full C3 remains open; ledger owns exact boundaries |
| C4 | Importer mutations are awaited before settlement; notifications follow release | Real async importer held during cancellation/completion; successor progress and terminal-state tests |

The complete producer/consumer boundary still requires the existing later
acceptance gates. Do not broaden C1 while its regression suite is failing.
Each checkpoint reports current evidence separately from historical passing
counts. The user authorized these checkpoints after reviewing the implementation
audit; they replace the single expanding implementation step, not its required
product outcomes.

**Persistence mechanism decision:** retain JSON for C1. SQLite is already an
available dependency and could simplify atomic updates and referential integrity,
but it would not remove download-to-filesystem recovery, destination authority,
or task ownership. The existing index database's path-based opening and
WAL/`synchronous=NORMAL` configuration do not prove this store's authority or
durability contract. A separate replacement requires a bounded real-file proof
of database/journal authority, interruption/reopening, and idempotent JSON
handover, and must demonstrate a net reduction in maintained machinery. C1
does not add a second authoritative store or a speculative database adapter.

**Restart reconciliation:** persisted bytes cannot reveal whether a previous
process observed a successful final sync. A fresh owner must explicitly confirm
the recorded transition through the store before using it to authorize work.
Tests must distinguish raw inventory from reconciled inventory; an in-memory
confirmation cache alone is not restart proof.

**C1 integration limits:** terminal settlement retains exact release records
without garbage collection. Queue-owned generic save/remove/revoke/relocate
operations now refuse mutation; C3 must use dedicated owned transitions rather
than bypass those guards. Runtime owners must complete destination effects
before store settlement and explicitly reconcile before restart admission.
The core integration checkpoint is lint-clean; that does not establish the
remaining end-to-end lifecycle claims.

**C2 integration limits:** the builder establishes the library directory before
opening its held download authority, and preserves HF search when that authority
is unavailable. Root-relative targets and absolute configured-root aliases share
identity; nested symlinks are rejected. Model, creation-anchor, and nested file
parent replacement cannot redirect capability effects. Marker publication uses
the existing atomic outcome algebra through a held parent, including directory
creation sync. C3 now routes ordinary start and admitted resume/cancellation
through this capability and rejects unconfigured starts. Runtime reservation
identity is integrated; old-record relocation is being retired. Admitted relocation and the
remaining lifecycle effects still need completion.
Only Linux execution is evidenced; this is not cross-platform acceptance or a
full builder-startup test.

**Current C3 limits:** real loopback transfer and orderly Error/reopen evidence
is not hard-process-crash or live Hugging Face evidence. Marker-failure and
seeded-file tests retain their narrower meaning. Held identities, network and
queued pause, and removal of old partial relocation are integrated. Unknown
admissions, Pending cleanup, and unresolved active Recovery custody still fail
closed without a complete recovery path. Verified Ambient cleanup settles its
exact remaining admission before restore; its focused checkpoint is accepted.
Verified Recovery settlement is also accepted. Runtime release facts remain
retained until owner drop. Physical
destination locking, admitted relocation, and C4 real awaited importer/callback
ordering remain unfinished. Selected producer/consumer GUI integration is
accepted separately, not evidence of full C3 or client-drop completion.

- **Exact source write set:**
  `rust/crates/pumas-core/src/model_library/hf/download.rs`,
  `rust/crates/pumas-core/src/model_library/hf/lifecycle.rs`,
  `rust/crates/pumas-core/src/model_library/hf/types.rs`,
  `rust/crates/pumas-core/src/model_library/hf/mod.rs`,
  `rust/crates/pumas-core/src/model_library/download_store.rs`,
  `rust/crates/pumas-core/src/model_library/download_recovery.rs`,
  `rust/crates/pumas-core/src/api/builder.rs`,
  `rust/crates/pumas-core/src/api/hf.rs` (additive ticket recovery and regressions only),
  `rust/crates/pumas-core/src/api/migration.rs`,
  `rust/crates/pumas-core/src/api/state.rs` (migration call-site arguments only),
  `rust/crates/pumas-core/src/model_library/partial_download.rs`,
  `rust/crates/pumas-core/src/metadata/atomic.rs`, and
  `rust/crates/pumas-core/src/metadata/mod.rs`.
- **Exact record set:** this plan, `execution-ledger.md`, `issues.md`, and
  `reports/rpc-contract-and-threat-model.md`.
- **Accepted base:** Slice B source hashes `abfe0382` (`lifecycle.rs`),
  `c69953b1` (`mod.rs`), `2e75638f` (`download.rs`), and `fdd00a3c`
  (`types.rs`). Additional admitted baselines are `020975b5`
  (`download_store.rs`), `c7244b5a` (`download_recovery.rs`), `95abe5fe`
  (`api/builder.rs`), `c838e184` (`metadata/atomic.rs`), and `15adcd4f`
  (`metadata/mod.rs`).
- **Admission invariant:** the builder opens the selected model-library root
  once and injects a crate-private held authority; an unconfigured client may
  search but destination mutation is typed unavailable. One caller-independent
  admission transition durably persists the complete request, non-authorizing
  destination identity, domain, FIFO ordinal, and predecessor/release proof
  before returning an ID, publishing active state, or performing an effect.
  Only a confirmed durable attempt promotes to the gated Worker in one no-await
  downloads-to-task commit; ambiguous publication parks hidden custody.
- **Destination invariant:** reservation identity and every effect derive from
  the same held configured-root capability plus validated relative target, not
  a raw/canonical path string or nearest-existing ancestor. Missing targets and
  aliases retain one identity; root/path replacement fails closed. The private
  state-lifetime queue retains Paused, recoverable Error, or Pending quarantine
  authority, orders restore by durable ordinal, and wakes only after exact
  generation plus durable/published terminal release. No physical async mutex
  is held across effect work, signals, or callbacks.
- **Persistence invariant:** version 4 is the only runtime format. It rejects
  old formats and ordinary resumable rows without exact admissions, owns
  ordinary row/admission/FIFO truth, and exclusively
  owns full-snapshot lifecycle quarantine. Pending cleanup is independent of
  sticky provenance. Clean Pending removal and sticky Pending-to-Verified use
  exact attempt/release proofs; Recovery quarantine preserves the durable
  revocation tombstone. Unknown publication never authorizes verification,
  cancellation, queue release, or empty restore. Stale status writers reject
  every quarantined ID. Old-format conversion is an explicit one-time operation
  outside the shipped library, with exact-byte backup, exclusive writer
  ownership, complete pre-publication validation, and durable atomic replacement.
- **Execution/publication invariant:** capability-relative marker staging,
  file sync, atomic rename, and parent sync use the accepted Slice A outcome
  algebra. Directory, marker, persistence, pause, restore, relocation, and
  callback work remains task-owned and drained. A private publisher linearizes
  immutable snapshot capture/revision/dispatch and signals outside all guards.
  Pause uses owner-visible wakeups for stalled headers/body/retry and only the
  exact started generation can durably project Paused. The real asynchronous
  importer mutation is owned and awaited while the logical destination claim
  is held; importer failure preserves resumable finalization state. Completion releases
  logical destination custody after durable/published terminal state and drain,
  before an owned callback-only phase whose panic cannot roll back Completed.
- **Earlier regression evidence:** a cached public ordinary start against a path
  occupied by a regular file returned setup `Err` after leaving a published
  ownerless `Queued` entry. A public ambient resume cancelled while awaiting
  authentication similarly left its prior `Paused` state as ownerless Queued.
  Store reds now also cover missing v1/v2 migration, exclusive quarantine,
  sticky-versus-clean Pending, typed removal proof, and ambiguous publication.
  Destination identity, durable FIFO admission/restore, capability marker,
  stalled pause, relocation, terminal rescue, and callback ordering remain
  red-first work before freeze.
- **Supporting finalization seam:** the queue-identity regression also reaches
  legacy restore's shared size-inference and finalization policy. The existing
  `partial_download.rs` owner is added to this slice solely to share that policy
  between its existing path adapter and the held-capability adapter. Existing
  library callers retain their behavior; HF restoration must not regain ambient
  filesystem authority or duplicate the policy. This replaces the prior
  nine-file count limit, not the product, consumer, or dependency boundaries.
- **Current-only cutover accepted:** schema 4 and exact ordinary admission are
  the only shipped runtime format. The two local paused records were converted
  once with durable backup and unchanged payload/marker bytes; the operator
  converter is retired. The ledger owns those commands and evidence. Old-record
  relocation and its unused move machinery are removed. `api/migration.rs`
  refuses partial-directory moves without effects; complete-model migration is
  unchanged. Admitted relocation remains separate C3 work. Do not reintroduce
  compatibility, infer historical FIFO, or rerun conversion during startup.
- **Held boundaries:** the next investigation admits no further source changes
  until its exact write set and deciding regression are recorded. The broader
  write set above records C3/C4 ownership, not permission
  to expand this checkpoint. Selected RPC/frontend/Electron/generated/CI changes
  are already accepted at `2b081fba`; no new wire or consumer change is needed.
  The metadata files expose only the existing atomic writer
  to a held capability-relative marker target. Builder changes inject the
  selected root in C2, propagate strict restore failure in C3, and wire owned
  asynchronous importer hooks in C4. The Rust restore method returns
  `Result<Vec<DownloadCompletionInfo>>`; its builder and test callers migrate
  together. Corrupt or uncertain authoritative download inventory prevents
  successful API initialization rather than being reported as empty restore.
  Slices D-E, further M2J work, full aggregate acceptance,
  general client Drop, consumer implementation, and standalone-shippable
  claims remain excluded. The [root incremental-commit decision](../plan.md#binding-decisions)
  permits coherent verified candidates with compatible reachable contracts;
  it does not permit incompatible producer-only integration or full C3
  acceptance from the narrower admission checkpoint.

## Objective

Bring the Rust library, its local IPC, the desktop RPC server, and the Rust side
of native adapters into compliance with the current security, contract,
persistence, concurrency, dependency, and verification standards. The result
must expose explicit invalid, failed, degraded, and shutdown outcomes; keep
secrets and internal locators out of public diagnostics; make durable model
mutation/event history recoverable; and make each accepted build capability
truthful.

This plan owns the canonical Rust/server desktop-RPC DTO and public
error/redaction contracts. It also owns the distinct local `pumas-core` IPC
contract. It does not own Electron/TypeScript projections or host-language
release support.

## Baseline

- Audit code baseline: `a33c8c0efa7cd8783c7deeac9e608db205290d43`.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Audit standards baseline: `52b096ded9c53afd439a3cf0efc4cc85252da570`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- The planning baseline changed documentation, not the audited Rust source; the
  finding evidence was rechecked against the planning baseline.
- No implementation or verification result is inherited from file counts,
  dependency counts, a green compile, or the audit's read-only inspection.

The routed authorities are `CORE-STANDARDS.md`, `STANDARDS-ROUTER.md`, the
Planning, Development Proportionality, Implementation, Verification,
Documentation, Tooling, and Build workflows; the Library Application, IPC, and
Persistence profiles; the Rust base, API, Async, Dependencies, Tooling,
Security, Cross-Platform, Interop, Language Bindings, and Unsafe standards;
and the Architecture, Contracts, Concurrency, Persistence, Resilience,
Security, Diagnostics, Performance, Dependencies, and Cross-Platform topic
owners.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| RUST-A1 | An actual debug-enabled `pumas-rpc` process receives a sentinel HF token and representative failures containing sentinel paths/URLs; captured diagnostics and public responses contain none of those values, while stable typed error codes remain observable. | `system` | `representative` — isolated launcher root and loopback process | `automated` | `satisfied` | [RPC disclosure evidence](reports/rpc-disclosure-evidence.md) |
| RUST-A2 | Every reachable desktop JSON-RPC method and local core IPC operation is admitted through its owning closed DTO contract; wrong protocol versions, unknown operations, missing/extra fields, negative or oversized values, and wrong result types remain distinct typed failures instead of successful defaults. | `contract` | `not-applicable` — deterministic Rust DTO/dispatch fixtures | `automated` | `pending` | Pending Milestone 2 |
| RUST-A3 | The accepted desktop-RPC exposure policy is enforced: either non-loopback binding is unavailable, or an unauthenticated hostile client cannot invoke/read any protected operation or event while an authorized caller can. | `system` | `representative` — isolated real TCP listeners and hostile-client fixtures | `automated` | `satisfied` | [RPC contract and threat model](reports/rpc-contract-and-threat-model.md#accepted-exposure-decision): typed loopback host, real negative CLI process, and real positive listener evidence |
| RUST-A4 | For every event-producing model-index mutation, injected failure cannot commit authoritative state without its durable event; after reopen, event replay and authoritative rows converge before an ephemeral notification is published. | `integration` | `representative` — real temporary SQLite files with controlled fault injection | `automated` | `pending` | Pending Milestone 3 |
| RUST-A5 | Every declared supported prior index schema migrates in deterministic identity/order with integrity checks; interrupted/repeated execution is safe, and unknown or corrupt state returns a typed recovery outcome. | `integration` | `representative` — real SQLite fixtures for every supported prior state | `automated` | `pending` | Pending Milestone 3 |
| RUST-A6 | Rust task and server owners return typed terminal outcomes; shutdown closes admission, signals cancellation, drains under the accepted bound, distinguishes complete/incomplete/failed work, and plugin startup reports the configured root as ready, explicitly disabled/degraded, or failed without path substitution or panic. | `system` | `representative` — real Tokio runtime, loopback servers, active requests, and invalid plugin root | `automated` | `pending` | Pending Milestone 4 and product decision `RUST-I2` |
| RUST-A7 | Each accepted core/RPC feature configuration compiles and runs its applicable tests with the promised public modules and dependency graph present or absent, on every supported Rust target. | `contract` | `required-real` — supported targets supplied by the platform plan | `automated` | `pending` | Pending Milestone 5 and platform evidence |
| RUST-A8 | `pumas-library` contains no UniFFI/Rustler framework dependency, scaffolding, or derives; Rust adapters own their DTO conversions and preserve typed/redacted errors, including distinct paused and cancelled outcomes. | `contract` | `not-applicable` — Cargo graph/source contract and Rust adapter tests | `automated` | `pending` | Pending Milestone 6 |
| RUST-A9 | The affected Rust format, compile, Clippy, test, documentation, feature-matrix, and isolation checks pass without touching a developer launcher root. | `integration` | `representative` — isolated Linux development workspace; target-specific portions use required real targets | `automated` | `pending` | Pending Milestone 7 |

Static search is supporting inventory evidence only. It cannot satisfy secret
non-disclosure, transaction recovery, network trust, lifecycle, or
cross-process claims.

## Scope

### In Scope

- `pumas-rpc` request/outcome DTOs, public error projection, diagnostics,
  transport exposure, admission, and shutdown.
- The separate `pumas-core` primary/client IPC request, response, error, and
  shutdown contract.
- SQLite model-index mutations, durable update events, startup migrations,
  reopen/replay behavior, and their real local test adapter.
- Rust-owned background tasks and fallible blocking-task join outcomes.
- Plugin-loader startup semantics at the Rust RPC composition root.
- `pumas-library` and `pumas-rpc` capability features, dependency ownership,
  public re-exports, and accepted consumer configurations.
- Core-versus-adapter placement for UniFFI/Rustler and Rust-side typed/redacted
  error semantics.
- Rust source/config/tests, focused Rust documentation, and Rust verification
  tooling needed to prove the owned claims.

### Out Of Scope

- Electron/TypeScript generated DTO projections and negative desktop-consumer
  tests; these consume the canonical Rust/server contract and belong to the
  desktop/platform plan.
- Binding host support matrices, generators, generated sources, packaged
  cohorts, host-runtime tests, and release evidence; these belong to the
  desktop/platform plan.
- CI scheduling and the repository-wide permanent-gate inventory, owned by the
  [governance plan](../governance-and-verification/plan.md).
- Frontend model freshness/presentation, accessibility, and UI changes.
- Torch/Python lifecycle or launcher-root durability outside the Rust-owned
  seams.
- General decomposition of the large model-library implementation. Size alone
  does not authorize refactoring.

## Constraints And Assumptions

### Constraints

- The first slice is a security repair and may not wait for the broader RPC
  redesign, but it must establish the error/redaction interface that the next
  slice can deepen rather than add a disposable filter.
- Secret values may not be written to test failure messages, snapshots,
  reports, or logs; use clearly synthetic sentinel values and assert absence.
- Public wire errors are stable, bounded, and redacted. Internal `PumasError`
  may retain operational context inside the owning process but is never a wire
  representation by `Display` conversion.
- SQLite behavior is tested with real temporary SQLite files. Do not add a
  storage trait or in-memory fake that hides transaction, WAL, locking, reopen,
  or migration behavior.
- No dependency is added until the owning milestone records its purpose,
  version authority, feature reachability, security posture, and removal path.
- Tests use temporary launcher roots and isolated ports; they may not discover
  or mutate the user's real library, credentials, processes, or registry.
- Shared Rust owners are changed serially. A milestone may not overlap another
  plan's edits to `rust/Cargo.toml`, `rust/Cargo.lock`, or shared CI config.
- A lower-fidelity unit/compile result does not substitute for a required real
  target, process, network, SQLite recovery, or host-boundary result.

### Assumptions To Validate

- Loopback-only desktop RPC is the accepted product boundary; LAN mode is
  removed and cannot be restored without a new authenticated remote contract.
- The existing `ModelIndex` public surface can remain the storage module
  interface while transaction/event and migration knowledge moves behind it.
- A bounded inventory will identify the accepted `pumas-library` consumer
  configurations; feature names and combinations are not assumed correct merely
  because they already exist.
- The platform plan will provide the binding host disposition and supported
  target matrix before Milestones 5 and 6 close.

## Owned Outcomes

1. One canonical Rust desktop-RPC contract module owns decoded commands,
   typed outcomes, stable public errors, protocol version, and redaction policy.
2. HTTP/SSE/OpenAI transports are protocol adapters and cannot invent defaults
   or expose internal `Display` text. Local core IPC retains its own smaller
   contract because its callers, trust, operations, and lifecycle differ.
3. The model index commits each authoritative mutation with its durable event
   in one recovery unit and publishes ephemeral notification only after commit.
4. Migrations have explicit supported inputs, stable identity/integrity/order,
   and defined interruption/re-entry postconditions without an unneeded generic
   framework.
5. Each Rust task owner has explicit admission, cancellation, join, and typed
   shutdown results; plugin initialization truthfully reports its configured
   authority.
6. Capability names correspond to real dependency, source, public-interface,
   and verification differences for accepted consumers.
7. Binding framework knowledge and wire-safe conversions stay in adapter crates;
   Rust-side errors remain typed and redacted.

## Binding Decisions

| Decision | Owner | Evidence/Reason | Status |
| --- | --- | --- | --- |
| The canonical desktop-RPC DTO and public-error/redaction contract lives in a Rust source module under `rust/crates/pumas-rpc/src/`; `handlers` and HTTP/SSE/OpenAI routing adapt to that interface. | This plan, Milestones 1-2 | R-01/R-07 and RPC/IPC standards; Rust is the producing authority | `accepted` |
| `rust/crates/pumas-core/src/ipc/protocol.rs` remains the canonical local primary/client IPC contract and is not mirrored into the desktop RPC contract. | This plan, Milestone 2 | Different caller, trust, operation, framing, and lifecycle contracts | `accepted` |
| Internal `PumasError` is not serialized by `Display`; each public adapter maps it to a closed error class/code and a bounded safe message. | This plan, Milestones 1-2 and 6 | Security, Diagnostics, IPC, and Language Bindings standards | `accepted` |
| The existing `ModelIndex` interface is the test/caller seam; real local SQLite is the production and test adapter, with test-only fault control kept internal to the module. | This plan, Milestone 3 | R-03/R-04; transaction/recovery behavior must not be faked | `accepted` |
| No universal task supervisor is presumed. Each task-owning module gets the smallest lifecycle interface its actual spawned work requires; consolidation needs a proven shared contract. | This plan, Milestone 4 | R-05 and Rust Async/Concurrency owners | `accepted` |
| UniFFI and Rustler types/conversions/errors belong to their adapter crates; the core stays framework-free. | This plan, Milestone 6 | R-08 and Rust Language Bindings standards | `accepted` |
| Desktop RPC is loopback-only; remove `--allow-lan` and reject every non-loopback `--host`. CORS is not caller authentication. | Product/program owner, consumed by Milestone 2 | R-02 | `accepted` (`RUST-I1`, 2026-09-03) |
| Compiled-out plugin support reports disabled/unavailable; compiled-in configured subsystem root/loader initialization failure fails startup without root substitution or degraded success. | Product/program owner, consumed by Milestone 4 | R-09 | `accepted` (`RUST-I2`, 2026-09-03) |
| Remove Pumas-owned UniFFI, Rustler/Elixir, and false Go surfaces while preserving the public Rust library used by Pantograph's exact-Git dependency. | Desktop/platform plan, consumed by Milestone 6 | R-08 and accepted support matrix | `accepted` (`RUST-I3`, 2026-09-03) |

## Evidence And Oracle Plan

| Claims | Deciding oracle | Independent authority | Unsupported by that evidence | Intended negative failure |
| --- | --- | --- | --- | --- |
| RUST-A1 | Capture an actual debug-enabled RPC binary's diagnostics and responses around synthetic secrets/locators; directly test the public-error mapping. | Public error/redaction interface plus process output | Electron persistence and arbitrary future log sinks | Sentinel appears in stderr/stdout/response, or an internal error becomes public text |
| RUST-A2/A3 | Exhaustive reachable-operation inventory, direct contract fixtures, real HTTP/local-IPC calls, and hostile non-browser clients. | Canonical Rust contract and selected exposure policy | Desktop consumer decoding | Invalid input reaches a domain handler; malformed output becomes success; unauthorized request succeeds |
| RUST-A4/A5 | Real SQLite files, controlled failures at transaction/migration stages, reopen, row/event comparison, and replay. | Model-index interface and migration manifest/identity | Production-disk hardware faults beyond the declared policy | Row without event, event without row, duplicate effect, unknown state accepted, or pre-commit publish |
| RUST-A6 | Active-work lifecycle scenarios observe admission closure, cancellation, joins, deadlines, terminal results, and configured-root startup result. | Owning task/server/plugin interfaces | Electron/frontend/Torch shutdown | Accepted task vanishes, shutdown hangs/claims false completion, root changes, or panic occurs |
| RUST-A7 | Cargo metadata/tree assertions plus compile and applicable behavior tests for every matrix cell on required real targets. | Accepted consumer/configuration matrix | Release artifact composition and binding hosts | Disabled capability remains linked/exported, enabled consumer fails, or target-specific cell is absent |
| RUST-A8 | Core dependency/source scan and adapter conversion/error tests through the adapter interface. | Core and adapter crate manifests/interfaces | Generated host sources, native loading, packaging, and host workflows | Core graph contains binding framework, or adapter collapses/prints internal error state |

## Systemic Finding Audit

- **Invariant family and canonical owners:** desktop RPC disclosure/DTOs are
  owned by the Rust RPC contract; local IPC by `pumas-core::ipc`; durable index
  state by `ModelIndex`; task lifecycle by each spawn owner; feature truth by
  Cargo plus the public module root; host conversion by each binding adapter.
- **Bounded reachable populations:** all registered RPC/HTTP/SSE/OpenAI routes
  and outward error sites; all core IPC methods; all SQL mutation/event and
  startup migration paths; all Rust-owned `spawn`/`spawn_blocking` handles; all
  feature-controlled dependencies/modules/re-exports and workspace consumers;
  all exported UniFFI/Rustler functions and error conversions.
- **Expansion facts:** expand a milestone only when a newly found site shares
  the same invariant and canonical owner. A frontend consumer, host generator,
  release artifact, or Python lifecycle site is handed to its owning plan.
- **Consumer dispositions:** every population entry is migrated, explicitly
  retained with evidence, removed, or assigned to another named plan; counts
  locate entries but do not decide their design.
- **Alternatives considered:** remove LAN instead of adding authentication;
  delete the response wrapper instead of layering validation around it; use one
  SQLite transaction instead of reconciliation after separate commits; keep
  migration machinery store-specific; remove unsupported binding surfaces
  instead of manufacturing support evidence.
- **Evidence-backed stopping condition:** each bounded population has one
  recorded disposition and every owned acceptance claim has its deciding
  scenario evidence in the declared environment.
- **Repaired-composition comparison:** the accepted design must reduce duplicated
  parsing/error/default/migration/lifecycle knowledge. A new registry, adapter,
  or supervisor that merely forwards the old surface fails admission.

## Simplicity And Ownership Review

**Applicability:** `applicable`

- Independent concepts and dimensions: RPC trust, wire decoding, public error disclosure, durable mutation/event consistency, schema evolution, task lifecycle, capability selection, and host conversion can change independently and retain separate module owners.
- State, identity, value, time, policy, and mechanism: SQLite rows/events/migration identities own durable state and order; request IDs/protocol versions own wire identity; task admission/cancellation/deadlines own time; exposure/redaction/capability support are policy; HTTP, local IPC, SQLite, Tokio, and FFI crates are mechanisms behind their owning interfaces.
- Caller and composition-root knowledge: `pumas-rpc::main` chooses exposure, plugin state, and production adapters; handlers know only decoded commands/outcomes; `PumasApi` callers know `ModelIndex` behavior rather than SQL sequencing; binding composition roots choose adapter crates without teaching core about frameworks.
- Representative change paths and forced owners: adding an RPC operation changes the Rust contract plus one handler and downstream generated projection; adding a durable mutation changes the `ModelIndex` implementation and interface test; adding a capability changes its matrix/manifests/module gate; adding a host mapping changes only its adapter unless the domain contract genuinely changes.
- Stable Interfaces versus hidden knowledge: stable surfaces are closed command/outcome/error types, `ModelIndex` operations/recovery promises, typed shutdown results, and accepted feature names; auth internals, SQL statements, tracing text, Tokio handles, serde glue, and host-framework annotations stay hidden.
- Independent evolution, testing, failure, and replacement: contract tests call the same interfaces as production adapters; HTTP and local fixture adapters exercise the RPC seam; real temporary SQLite exercises the index seam; each task owner and binding adapter can fail/test independently without bypassing typed results.
- Necessary complexity and containment: one desktop RPC contract module, the existing distinct local IPC module, the existing model-index module, and owner-local lifecycle interfaces are sufficient; do not add a generic schema registry, storage abstraction, migration framework, or universal supervisor without evidence from two real adapters/consumers.
- Deletion and cumulative machinery result: deleting the RPC contract would redistribute validation/error knowledge across every handler/transport, deleting index transaction ownership would redistribute SQL/event recovery across mutations, and deleting adapter-local conversion would reinfect core; conversely `wrapper.rs`, core UniFFI annotations, false feature markers, temporary plugin fallback, and obsolete pass-through parsing should disappear rather than gain another layer.

## Milestones

### Milestone 0: Reconcile Baseline And Ownership

**Goal:** Turn the audit into one current, routed, executable authority with
cross-plan boundaries and adequate acceptance claims.

**Allowed write set:**

- `docs/plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/plan.md`
- `docs/plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/execution-ledger.md`
- `docs/plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/issues.md`

**Tasks:**

- [x] Recheck audit evidence against the planning baseline.
- [x] Route the current standards and complete the composed-design probe.
- [x] Assign Rust/server DTO and error/redaction ownership here and downstream
  desktop/host evidence to the platform plan.
- [x] Record unresolved decisions without allowing them to block the critical
  first slice.

**Acceptance gate:** The current standards plan checker passes, local links and
referenced source paths resolve, and the ledger records that no implementation
slice has started.

**Status:** `Accepted`

### Milestone 1: Close Critical Diagnostic Disclosure

**Goal:** Establish one usable public error/redaction interface and remove the
known credential disclosure through the real RPC process.

**Allowed write set:**

- `rust/crates/pumas-rpc/src/main.rs`
- `rust/crates/pumas-rpc/src/handlers/**`
- `rust/crates/pumas-rpc/src/contract.rs` (new, if this remains the accepted
  module filename after source reconciliation)
- `rust/crates/pumas-rpc/tests/integration_tests.rs`
- `docs/SECURITY.md`
- this plan, ledger, issues, and `reports/rpc-disclosure-evidence.md`

**Tasks:**

- [x] Inventory request fields and every outward `PumasError`/handler error path
  reachable through the RPC process; classify secret, private locator, safe
  identifier, and bounded public context.
- [x] Define the closed Rust public error class/code and deny-by-default safe
  message projection; keep internal causes/locators in private diagnostics only
  when they are not secrets.
- [x] Remove complete parameter logging. Emit method, request ID, outcome class,
  and correlation data only from an explicit allowlist.
- [x] Route generic JSON-RPC and protocol-specific public errors through the
  redaction interface wherever the inventory proves the same disclosure
  invariant; record non-RPC owners rather than widening silently.
- [x] Modify the real-process test harness to capture bounded stderr, exercise
  `set_hf_token` with a synthetic sentinel and controlled locator failures, and
  assert absence from diagnostics and responses plus presence of stable codes.
- [x] Run focused mapping tests, the real binary scenario, and affected
  `pumas-rpc` format/check/Clippy/default/no-default tests.

**Acceptance gate:** RUST-A1 is satisfied by the process scenario and focused
mapping tests; text search is supporting evidence only.

**Status:** `Accepted`

### Milestone 2: Deepen RPC And IPC Contracts At The Trust Seam

**Goal:** Replace arbitrary JSON/default projection with explicit Rust-owned
contracts and enforce the accepted network exposure policy.

**Allowed write set:**

- `rust/crates/pumas-rpc/Cargo.toml`
- `rust/crates/pumas-rpc/src/main.rs`
- `rust/crates/pumas-rpc/src/server.rs`
- `rust/crates/pumas-rpc/src/contract.rs`
- `rust/crates/pumas-rpc/src/handlers/**`
- `rust/crates/pumas-rpc/src/wrapper.rs`
- `rust/crates/pumas-rpc/tests/integration_tests.rs`
- `rust/crates/pumas-core/src/ipc/**`
- `rust/crates/pumas-core/src/api/hf.rs`
- `rust/crates/pumas-core/src/api/mod.rs` for the exact Milestone 2I
  crate-private required-refresh seam export
- `rust/crates/pumas-core/src/api/models.rs` for the exact Milestone 2I
  refresh/reconciliation caller correction
- `rust/crates/pumas-core/src/api/reconciliation.rs` for the exact Milestone
  2I single-flight admission and success/failure terminal-state correction
- `rust/crates/pumas-core/src/api/state.rs`
- `rust/crates/pumas-core/src/api/state_hf.rs`
- `rust/crates/pumas-core/src/error.rs` for the exact Milestone 2I typed
  operation-in-progress outcome
- `rust/crates/pumas-core/src/index/model_index.rs` for the exact Milestone 2I
  uncapped catalog query
- `rust/crates/pumas-core/src/model_library/download_recovery.rs` for the exact
  PRG-I19 recovery snapshot, stale fingerprint, and managed-target authority
- `rust/crates/pumas-core/src/model_library/download_store.rs` for the exact
  PRG-I19 serialized persistence mutation/revocation owner
- `rust/crates/pumas-core/src/metadata/atomic.rs` and `src/metadata/mod.rs` for
  the exact PRG-I19 Slice A durable-publication Interface and crate-private
  re-export
- `rust/crates/pumas-core/src/model_library/mod.rs` for that bounded public
  recovery-ticket export
- `rust/crates/pumas-core/src/model_library/hf/download.rs` for exact tracked
  destination/repository/file-context matching in the PRG-I19 correction
- `rust/crates/pumas-core/src/model_library/hf/lifecycle.rs` for the exact
  PRG-I19 Slice B download-generation, outer-task, blocking-task, and
  cancellation-finalizer owner
- `rust/crates/pumas-core/src/model_library/hf/types.rs` for the exact private
  state-local recovery capability aggregate and its non-persisted lifecycle
- `rust/crates/pumas-core/src/model_library/library.rs` for the exact Milestone
  2I all-model listing owner and its greater-than-10,000-record oracle
- `rust/crates/pumas-core/tests/api_tests.rs`
- `rust/crates/pumas-uniffi/src/bindings.rs` only as a compile-truthful direct
  exhaustive error projection pending its accepted removal in Milestone 6; no
  new binding behavior is admitted
- `rust/crates/pumas-uniffi/src/bindings/api_hf.rs` only as a compile-truthful
  direct caller pending its accepted removal in Milestone 6; no new binding
  behavior is admitted
- `rust/Cargo.toml`, `rust/crates/pumas-core/Cargo.toml`, and `rust/Cargo.lock`
  for the accepted exact `cap-std = { version = "4.0.3", default-features =
  false }` held-directory dependency and resolved closure only
- `rust/README.md`, `rust/crates/pumas-core/README.md`, `docs/ARCHITECTURE.md`,
  and `docs/SECURITY.md`
- this plan, ledger, issues, and `reports/rpc-contract-and-threat-model.md`

**Tasks:**

- [x] Resolve `RUST-I1` before the first exposure edit: remove LAN support or
  record authenticated capability, operation authorization, credential
  lifecycle, admission/rate, event-stream, and failure contracts.
- [x] Inventory every desktop RPC method and separate HTTP/SSE/OpenAI route,
  plus every local IPC operation and consumer. Record one canonical owner and
  migrate/remove/retain disposition per entry.
- [x] Compare at least two representative contract-module shapes using a
  credential method, a signed pagination/size method, a typed collection
  result, and an event error; select the smaller interface that prevents
  transport/handler knowledge leakage.
- [ ] Decode protocol version and method-specific params before dispatch using
  closed request variants, checked numeric conversion and bounds, declared
  missing/null/extra-field policy, and distinct unknown/unsupported outcomes.
- [ ] Validate typed outcome variants before serialization. Delete the generic
  default-inventing wrapper when its callers are migrated; do not retain it as
  a pass-through compatibility layer.
- [ ] Apply the safe public-error contract to JSON-RPC and the distinct
  protocol adapters. Preserve internal errors for operations without exposing
  `Display` text.
- [ ] Test the contract interface directly and the production HTTP/local-IPC
  adapters with valid, malformed, hostile, unauthorized, and wrong-outcome
  cases.
- [ ] Publish the stable Rust DTO/error handoff to the platform plan. That plan
  owns generated TypeScript projection and negative Electron consumer proof.

**Acceptance gate:** RUST-A2 and RUST-A3 are satisfied, and the platform plan
has an explicit handoff revision for downstream projections.

**Status:** `Active`

### Milestone 3: Make Index Mutation, Events, And Evolution Recoverable

**Goal:** Put authoritative SQLite mutations and durable events in one recovery
unit and make every supported schema transition identifiable and re-entrant.

**Allowed write set:**

- `rust/crates/pumas-core/src/index/model_index.rs`
- `rust/crates/pumas-core/src/index/model_index/**`
- `rust/crates/pumas-core/src/index/mod.rs`
- `rust/crates/pumas-core/tests/api_tests.rs`
- `rust/crates/pumas-core/tests/model_index_persistence.rs` (new)
- `rust/crates/pumas-core/README.md` and `docs/ARCHITECTURE.md`
- this plan, ledger, issues, and `reports/model-index-recovery-evidence.md`

**Tasks:**

- [ ] Inventory all SQL write paths, including upsert/delete, dependency
  profiles, package-facts cache, overlays, repairs, and every durable/ephemeral
  update publication. Give each one a transaction/event disposition.
- [ ] Move each authoritative mutation and its durable event into one SQLite
  transaction behind the existing `ModelIndex` interface; publish broadcast
  updates only after successful commit.
- [ ] Build supported-prior-schema fixtures from repository history and current
  upgrade code. Explicitly declare the oldest supported state and reject
  unknown/corrupt states rather than guessing.
- [ ] Select the smallest store-specific migration identity/integrity/applied-
  state mechanism after probing the real current schemas. Record the decision
  before implementation; do not assume a universal version table or framework.
- [ ] Define transaction and interruption postconditions for every migration,
  including table rebuilds, and make retry/reopen deterministic.
- [ ] Test through the `ModelIndex` interface with real temporary SQLite files
  and internal controlled failure points at mutation, event, commit, migration,
  reopen, and publish stages.

**Acceptance gate:** RUST-A4 and RUST-A5 are satisfied; the recovery report maps
every supported input and mutation family to deciding evidence.

**Status:** `Planned`

### Milestone 4: Make Startup, Task Ownership, And Shutdown Truthful

**Goal:** Ensure accepted work and configured plugin state cannot disappear
behind aborted handles, empty results, false readiness, or an unrelated root.

**Allowed write set:**

- `rust/crates/pumas-core/src/api/runtime_tasks.rs`
- `rust/crates/pumas-core/src/api/builder.rs`
- `rust/crates/pumas-core/src/ipc/server.rs`
- `rust/crates/pumas-core/src/plugins/**`
- `rust/crates/pumas-core/src/error.rs`
- `rust/crates/pumas-core/tests/api_tests.rs`
- `rust/crates/pumas-rpc/src/main.rs`
- `rust/crates/pumas-rpc/src/server.rs`
- `rust/crates/pumas-rpc/src/handlers/plugins.rs`
- `rust/crates/pumas-rpc/tests/integration_tests.rs`
- `rust/README.md`, `rust/crates/pumas-core/README.md`, and
  `docs/ARCHITECTURE.md`
- this plan, ledger, issues, and `reports/rust-lifecycle-evidence.md`

**Tasks:**

- [ ] Inventory each Rust-owned `spawn`/`spawn_blocking`, its admission point,
  handle owner, cancellation signal, natural completion, error/panic path,
  deadline, and shutdown postcondition. Expand only within the same owner.
- [ ] Replace pruning/aborting/discarding with owner-local typed terminal
  outcomes. Preserve blocking-task join failure rather than fabricating an
  empty result.
- [ ] Close server admission, signal connection/request tasks, drain within a
  recorded operational bound, await owned handles, and return
  complete/incomplete/failed shutdown. Keep `Drop` best-effort and separate
  from the explicit async contract.
- [ ] Resolve `RUST-I2`, then make plugin startup use only the configured root
  and return the selected ready/disabled/degraded/failed state without unwrap.
- [ ] Exercise task success/error/panic/cancel, spawn versus shutdown races,
  active RPC/IPC connections, repeated shutdown, timeout/incomplete outcome,
  and invalid/unreadable plugin roots on a real Tokio runtime.

**Acceptance gate:** RUST-A6 is satisfied; every inventoried handle has a
terminal disposition and the plugin state is observable through its owner.

**Status:** `Planned`

### Milestone 5: Make Core And RPC Capabilities Real Configurations

**Goal:** Align capability names with dependency, source, public-interface, and
consumer behavior for every accepted configuration.

**Allowed write set:**

- `rust/Cargo.toml` and `rust/Cargo.lock`
- `rust/crates/pumas-core/Cargo.toml`
- `rust/crates/pumas-core/src/lib.rs`
- `rust/crates/pumas-core/src/api/**`
- `rust/crates/pumas-core/src/model_library/hf/**`
- `rust/crates/pumas-core/src/process/**`
- `rust/crates/pumas-core/src/system/**`
- `rust/crates/pumas-core/src/onnx_runtime/**`
- other core source files only after they are named in the approved
  feature-to-code inventory in the ledger
- `rust/crates/pumas-app-manager/Cargo.toml`
- `rust/crates/pumas-rpc/Cargo.toml`, `rust/crates/pumas-rpc/src/main.rs`, and
  `rust/crates/pumas-rpc/src/server.rs`
- `scripts/rust/check.sh`
- `rust/README.md` and `rust/crates/pumas-core/README.md`
- this plan, ledger, issues, and `reports/rust-feature-matrix.md`

**Tasks:**

- [ ] Inventory real workspace/published consumers and map each accepted
  capability to dependencies, modules, public re-exports, runtime behavior,
  platform restrictions, tests, and incompatible combinations.
- [ ] Decide whether current names (`hf-client`, `process-manager`,
  `gpu-monitor`, `full`, and RPC `inference-plugins`) express those consumers;
  record any public compatibility change before editing.
- [ ] Make optional dependencies and source/public gates agree. Do not call a
  marker optional when heavy HF, archive, ONNX, process, GPU, watcher, or
  platform dependencies remain reachable without an accepted reason.
- [ ] Preserve a working `pumas-rpc --no-default-features` library-only build
  and prove plugin-enabled behavior separately.
- [ ] Add matrix commands that check both dependency exclusion and consumer
  compilation/behavior; coordinate their permanent CI schedule with governance.
- [ ] Obtain required-real results for each supported Rust target from the
  platform target matrix; missing environments leave RUST-A7 pending.

**Acceptance gate:** RUST-A7 is satisfied for every accepted matrix cell, with
Cargo tree/metadata and applicable behavior proof rather than compilation alone.

**Status:** `Planned`

### Milestone 6: Restore The Rust Core/Binding Adapter Seam

**Goal:** Remove host-framework knowledge from core and make Rust adapter error
semantics truthful without claiming unsupported hosts.

**Allowed write set:**

- `rust/Cargo.toml` and `rust/Cargo.lock`
- `rust/crates/pumas-core/Cargo.toml`
- `rust/crates/pumas-core/src/lib.rs`
- `rust/crates/pumas-core/src/models/**`
- `rust/crates/pumas-core/src/model_library/types.rs`
- `rust/crates/pumas-core/src/model_library/dependencies.rs`
- `rust/crates/pumas-uniffi/Cargo.toml` and `rust/crates/pumas-uniffi/src/**`
- `rust/crates/pumas-rustler/Cargo.toml` and
  `rust/crates/pumas-rustler/src/lib.rs`
- `scripts/rust/check.sh`
- `rust/README.md`, `rust/crates/pumas-core/README.md`, and
  `docs/native-bindings.md`
- this plan, ledger, issues, and `reports/rust-binding-boundary.md`

**Tasks:**

- [ ] Consume the platform plan's supported-host/disposition matrix before
  changing public binding claims; record unsupported/deferred hosts explicitly.
- [ ] Remove the core UniFFI dependency/feature/scaffolding/derives and complete
  adapter-local records/conversions through the actual UniFFI interface.
- [ ] Define typed, bounded, redacted Rust adapter errors. Preserve paused,
  cancelled, invalid, unavailable, and internal outcomes distinctly.
- [ ] Resolve `RUST-I3`: remove Rustler's false core dependency/claim if no real
  host consumer is accepted, or implement only the bounded core adapter surface
  accepted by the platform plan. Do not build speculative host breadth.
- [ ] Test Rust conversions and error projection through adapter interfaces and
  assert the core dependency/source graph is framework-free.
- [ ] Hand host generation, native loading, packaged cohort, async host call,
  negative host input, and release proof back to the platform plan.

**Acceptance gate:** RUST-A8 is satisfied. Host support/release claims remain
outside this plan and cannot be inferred from Rust-only tests.

**Status:** `Planned`

### Milestone 7: Rust Objective Acceptance

**Goal:** Re-run all owned deciding evidence, reconcile documentation and
downstream handoffs, and accept only the claims actually proved.

**Allowed write set:**

- `scripts/rust/check.sh`
- `rust/README.md`
- `rust/crates/pumas-core/README.md`
- `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, and `docs/native-bindings.md`
- this plan, ledger, issues, and `reports/**`

**Tasks:**

- [ ] Run each objective's deciding scenario in its declared environment and
  link exact commands/results without copying raw secrets or unbounded logs.
- [ ] Run affected Rust format, compile, Clippy, test, doc, feature-matrix, and
  isolation checks; classify failures instead of weakening gates.
- [ ] Reconcile source module documentation and the concise project guides with
  accepted contracts/configurations.
- [ ] Confirm the platform plan has the canonical RPC revision and binding/target
  handoffs; do not mark its downstream claims satisfied here.
- [ ] Review all issues, reports, re-plan triggers, and objective rows before
  changing lifecycle state.

**Acceptance gate:** RUST-A1 through RUST-A9 are satisfied with linked evidence;
no required-real environment is represented by a lower-fidelity substitute.

**Status:** `Planned`

## Dependencies And Coordination

- The [governance plan](../governance-and-verification/plan.md) owns permanent
  gate inventory and CI scheduling. Its CI edits must land before this plan adds
  new schedules to avoid shared-file conflict.
- The desktop/platform plan (planned sibling
  `../desktop-release-bindings-and-torch/plan.md`) consumes the canonical Rust
  DTO/error revision and owns Electron/TypeScript generation, negative desktop
  consumer tests, host support, packaged cohorts, and release evidence. It
  supplies the target/host matrix and Rustler disposition back to Milestones
  5-6.
- Frontend/UI remediation consumes downstream desktop projections; it does not
  edit the Rust contracts.
- Milestones 1-6 run serially because they overlap Rust composition roots,
  manifests, public errors, or tests. Independent downstream work starts only
  after a versioned handoff, not against a moving schema.

## Risks

- **Credential regression:** another method or protocol-specific error can
  bypass the first projection. Mitigation: systemic outward-site inventory plus
  process-level sentinel evidence.
- **Consumer breakage:** strict DTOs expose latent Electron/TypeScript drift.
  Mitigation: versioned Rust handoff and downstream generated/negative tests;
  no silent compatibility defaults.
- **Remote exposure ambiguity:** retaining LAN mode materially expands auth,
  credential lifecycle, rate, and event-stream work. Mitigation: decide remove
  versus authenticate before Milestone 2 edits.
- **Migration/data loss:** unknown deployed schemas or interrupted rebuilds may
  exceed current fixtures. Mitigation: derive supported states from history and
  block acceptance on unknown/corrupt recovery evidence.
- **Shutdown hangs or loss:** draining without accepted bounds may hang, while
  aborting may lose work. Mitigation: explicit admission/cancellation/deadline
  policy and typed incomplete outcomes.
- **Feature compatibility:** making markers real can change public API and
  transitive dependencies. Mitigation: consumer matrix and compatibility
  decision before source gating.
- **Binding scope expansion:** host claims can pull generator/release work into
  Rust refactoring. Mitigation: platform-owned matrix and strict adapter-only
  ownership here.

## Blockers

- The durable-unlink checkpoint is accepted on Linux. Pending replay still
  requires phase-aware preparation and cross-client/process exclusion through
  cleanup drain; it is not admitted by the unlink fix.
- `RUST-I1` is resolved; Milestone 2 enforces the accepted loopback-only
  boundary.
- `RUST-I2`'s product decision is resolved; Milestone 4 must still remove the
  configured plugin initialization fallback and prove the accepted behavior.
- `RUST-I3` is resolved; Milestone 6 owns Rust source/manifests removal while
  the platform plan owns binding scripts, docs, and release projection.

## Re-Plan Triggers

- The disclosure inventory shows secret propagation outside the current
  `pumas-rpc` owner or requires changing a downstream consumer contract in the
  first slice.
- The LAN decision retains remote access but lacks an accepted credential and
  authorization owner, or introduces browser-session assumptions.
- Representative RPC design shows one closed contract would combine unrelated
  HTTP/SSE/OpenAI/local-IPC promises or merely move the existing dispatcher.
- SQLite capability/history cannot support the proposed atomic event or stable
  migration mechanism without a data-format compatibility decision.
- Lifecycle inventory finds an owner outside this plan or an operation whose
  cancellation/deadline semantics require product policy.
- Feature inventory identifies external consumers or incompatible combinations
  absent from the accepted matrix.
- The platform plan changes supported targets/hosts, rejects the Rust contract
  handoff, or requires core domain changes beyond adapter conversion.
- A proposed module fails the deletion test, introduces a hypothetical seam,
  or increases cumulative parsing/default/supervision machinery.

## Final Acceptance

- Acceptance status: `partial`
- Deferred follow-ups: Electron/TypeScript projections and negative consumer
  tests, real binding-host cohorts, generators, packaged artifacts, and release
  evidence remain with the desktop/platform plan; frontend presentation remains
  with the frontend/UI plan.
- Final status: `Active`
