# Plan: Current Standards Remediation Program

**Plan status:** `Active`

**Post-release deferral:** Further unified inference gateway development is
deferred until after the next release (user decision, 2026-09-12). The
[architecture brief](../../breif/unified-inference-gateway.md) records the proposed direction;
it is not implementation authority or a next-release requirement.

**Current phase:** The coordinated reconciliation, selected desktop contract,
and artifact-collision checkpoints are committed and accepted within their
recorded Linux evidence boundaries (`f77f4bed`, `2b081fba`, `2b9553a0`). The
selected catalog/download/ticket producer, generated validators, bundled
preload, renderer consumers, and root-scoped display-only startup cache now
integrate together. They are not completion of the all-route contract, full
frontend M4/M5, or general shutdown work.

PRG-I19 C1/C2 and the incremental C3 admission, pause, cancellation, recovery,
and current-only cutover checkpoints remain verified. Old download formats and
partial relocation are retired, not compatibility obligations. C3 production
restore/lifecycle remains open; C4 awaited managed importer ownership is accepted
within the Linux checkpoint below. Hard-process crash recovery is not proved.
Slices A/B, governance, frontend milestones 0–3,
and recorded incremental launcher/Torch work retain their evidence limits.

The bounded Verified Ambient restore settlement is accepted after focused
review, dual core suites, and strict lint. Remaining C3 requires separate
exact admission. The
[execution ledger](execution-ledger.md) retains dated findings and verification;
the contracts below remain binding. Current blockers are listed in
[Blockers](#blockers).

**Accepted Slice B boundary:** PRG-I19 Slice B is internally accepted at exact
source hashes `abfe0382…`, `c69953b1…`, `2e75638f…`, and `fdd00a3c…` after both
independent review axes and root reproduction passed. The detailed contract
below remains the regression boundary for later slices. It is not a standalone
commit, full PRG-I19/M2I acceptance, consumer-compatible result, or M4 shutdown
claim.

Slice B corrected only its admitted four source
files. Revalidate the current actual-ID `RecoveryTransition` while holding the
download-state lock immediately before generic resume or relocation mutates;
combine that with a state-local revocation disposition/epoch or equivalent so
a completed durable revoke remains authoritative after transition removal,
without awaiting persistence under the state lock. The earlier pre-await check
is not authoritative, and relocation must reject any current owner, including
a `CancelFinalizer` that replaced the transition. Deterministically pause each
ambient operation after that first check, install the transition, and prove the
operation returns false without state, destination, task, or persistence
mutation while recovery completes. Also force durable revoke followed by
transition failure/removal before each ambient mutation, plus transition-to-
finalizer replacement before relocation, and prove refusal.
Reconciliation must also revalidate the
generation before both its in-memory and durable projections: a worker
installed after the candidate snapshot remains owned and is never persisted as
`Paused`; a matching successor remains authoritative even when finished until
its observation is consumed. Finished-task observations must also remain
generation-matched through state projection, including concurrent observers,
so an old failed Worker or finalizer cannot corrupt a successor. Observation
custody must survive caller cancellation and remain authoritative to
reconciliation until exact-once projection completes. Replace synchronous
handle polling with a start-gated actual-ID `TerminalProjection` that is
installed only for a proved-finished predecessor, retains its outcome, performs
async state/persistence projection after all guards are released, and settles
only its matching generation through a typed, idempotent outcome that
distinguishes pending, already settled, stale, and missing. Duplicate waiters
must terminate without spinning, and a stale ticket cannot remove a successor.
Panicked projection fallback is also owner work: one owner/winner performs its
state and publication exactly once, independent of ticket waiter cancellation.
Its panic boundary encloses both projection future construction and polling;
call-time or poll-time panic cannot strand the outcome as pending. Its sticky
failure sink belongs directly to the projection cell/context and does not
depend on the entry still occupying the task map when concurrent cancel wins.
If the fallback itself panics during construction or polling, settlement must
return a typed unprojected failure without spinning while retaining enough
owner/state provenance for a later cancel to remain fail closed; cover waiter
cancellation as well as both fallback panic phases.
If cancel supersedes a failed projector before fallback acknowledgement,
failure custody transfers explicitly to the finalizer: the old cell must reach
a typed terminal settlement rather than leave its waiter looping on `Pending`,
and the finalizer must project `Error` before acknowledging that transfer.
The old fallback returning through `catch_unwind` is not acknowledgement by
itself: it may have produced a domain `RolledBack` because cancel won. Only a
verified failure projection, or the replacement finalizer after its `Error`
projection, may acknowledge the obligation.
Prove waiter completion, finalizer settlement, and no residual owner.
After projection installation, start/drain custody must already belong to the
lifecycle owner rather than a caller-held gate token, so concurrent cancel or
installer drop/panic before `start()` cannot detach predecessor work.
That cannot be implemented as `start_on_drop`: destructor execution during
unwind may occur while an outer state guard is held, and must not start, abort,
or close gates. Drop stays side-effect free; explicit post-lock lifecycle
actions own start/rollback/drain. Its sole permitted mutation is a bounded
lock-free lease mark from `Gated` to `Abandoned`; it cannot lock or mutate the
owner map, wake, abort, allocate, call back, or perform I/O. The lifecycle owner
retains the strong atomic lease; weak/refcount disappearance is not the state
transition. Every rejection/collision invokes explicit rescue after releasing
guards, and that rescue retains/observes aborted handles through an owner-held
bounded reaper rather than dropping them. A retirement flag alone is not
terminal observation: the reaper retains each observer handle until that task
is terminal-ready, consumes and records its join result, and proves no
outstanding observer remains. A nested completion flag set before its observer
callback/result delivery and task return is not sufficient; a held
post-flag/pre-return cancellation case must prove terminal cannot overtake that
observer. The same rule covers abandoned prepared Workers
and `RecoveryTransition`s; aborting and dropping their wrapper handles is not
accepted lifecycle ownership. Successful recovery also currently projects
`Completed` and clears capability before its persisted-row cleanup is observed;
cleanup failure is discarded instead of failing closed. Verified recovery does
not use ordinary status persistence, so it needs a separate terminal-cleanup
authority that makes the resumable row unable to return while preserving Slice
A's durable revoked disposition; it must not re-enable ambient recovery-state
serialization or delete the tombstone.
Cancellation of either Ambient or Recovery work needs its destination and bound
file set so the finalizer—not a Worker attempt—owns removal of every part and
marker, then persisted row and final drain, before `Cancelled` or capability
clear.
Owner-held gated entries therefore need explicit started/abandoned state: an
unopened gate is not live attach evidence, and reconcile or repeated cancel
must start or roll back/drain it after locks are released.
Inherited panic/nested failure must survive
projector cancellation or supersession and remain visible to a replacement
cancel finalizer. Projection of an outer-completed `CancelFinalizer` on
nonterminal state also captures an owner-visible unfinished-finalizer
obligation before release. The projector's own `Failed`/`Panicked` outcome must likewise
fold into later task observation/finalization rather than live only in a
private cell. If the failed projection settles and removes before a later
cancel, typed state-local lifecycle-failure provenance must still distinguish
it from an ordinary user error and prevent false `Cancelled`/capability clear.
Every failed or missing lifecycle-role observation—Worker,
`RecoveryTransition`, `TerminalProjection`, or `CancelFinalizer`—sets that
unresolved provenance when state is not explicitly verified terminal, even if
worker/pause logic already cleared `task_registered`.
The state-locked install also captures a status-based unfinished obligation for
a Join-Ok Worker left active or a finalizer left `Cancelling`, regardless of
`task_registered`; the former projects `Error`, never generic `Paused`.
Public cancel must capture that same obligation when it directly replaces a
finished Worker while state remains active; it cannot rely on a prior
`TerminalProjection` having observed the Worker or on a separate pre-replacement
task snapshot. Capture occurs atomically in the lifecycle replacement result;
it uses the predecessor outer handle's terminal readiness rather than aggregate
entry readiness, which may wait on held nested work. A deterministic
Worker-finish-between-snapshot-and-replacement case, including an outer-
finished/nested-held variant, must prove nested drain followed by `Error`,
sticky provenance, and retained recovery capability.
Repeated public cancel must itself observe or replace a finished/panicked
`CancelFinalizer`; it cannot return `AlreadyRunning` and leave `Cancelling`
until an unrelated list/status call repairs it. Even an outer-completed
finalizer that left state `Cancelling` is unfinished/fail-closed, not clean
success.
Recovery-transition semantic failure cannot bypass that projection by directly
removing and discarding its task observation after delivering a non-resumed
result; a revoke error is sticky failure, while clean context mismatch remains a
separate outcome.
Reconciliation reserves that same role before
persisting `Paused`, blocks ambient operations, then generation/status-rechecks
before memory/snapshot projection. When persistence is configured, only an
actual updated row (`true`) proves that durable projection; an absent-row
`false` is fail-closed/no-durable-row and cannot be coerced to success or invent
a row. Add deterministic
two-observer, reconcile-during-unprojected, and cancelled-observer awaits plus a deterministic public cancellation case with real state and
capability but an absent predecessor owner, and a ready-path replacement race
proving an old Worker cannot preempt the finalizer's terminal projection or
strand capability. Bind every Worker state write after an await/cancel point to
the current generation, role, and expected status; deterministically race both
retry reset and pause projection against cancel replacement. Preserve the already-corrected
owner-visible write/flush and shared nested custody, but add generation-safe
reaping/archive or a bounded actor so retained handles do not grow with chunk
count and owner-side semantic failures survive waiter cancellation and still
fail drain/finalization closed. Install finalizer and blocking-operation
reservations under the task-map mutex, explicitly release it, then signal their
gated external work. Every installed task token must likewise leave the
download-state lock before its start gate opens. This exhaustively includes the
ordinary shared seam, but changes only signal timing; its preparation,
publication, and admission outcomes remain Slice C. One explicit terminal
failure-policy exception is admitted here: configured Ambient persistence-row
cleanup and final drain precede `Completed` and its success callback; failure
projects `Error` with no success callback. Synchronous callbacks run through
TaskContext-owned blocking isolation after releasing the destination lease.
Auxiliary callback completion is awaited before reacquiring the lease and
revalidating generation, role, state, and exact destination; completion
callback runs only after terminal settlement. Panic is observed as notification
failure and cannot resume stale work. Auxiliary panic is a typed owner-recorded
failure before result delivery and survives waiter cancellation; completion
panic after verified `Completed` is observed but does not reopen terminal. Before
`Completed` or recovery capability release, owned persistence
cleanup through the separate recovery terminal-cleanup authority and final
drain must complete; cleanup failure projects `Error` and
retains capability. Add deterministic held and failing removal oracles.
Cancellation finalization for Ambient and Recovery captures destination plus
all bound files; it removes every part and marker, then the persisted row, and
drains before terminal. Ambient removes its live row; Recovery prevents any
resumable row from returning while retaining the revocation tombstone. Exercise held and failing multi-file, scanner-visible
artifacts for both destinations rather than an empty fixture.
This authority also requires Ambient Worker file mutations to remain in
`TaskContext` custody. Tokio-internal blocking create/open/write/flush/rename
cannot outlive the outer task unseen and then mutate after finalizer cleanup or
`Cancelled`; add a held production Ambient mutation followed by public cancel
and prove drain plus no post-terminal artifact.
Add reentrant callback cases that cancel and acquire the destination lease,
then prove no stale write resumes. Snapshot broadcasts that still occur under
the destination guard and the direct ordinary-start auxiliary callback that
still runs synchronously in the async request are named Slice C publication and
callback defects; Slice B claims no global callback or full guard/signal
compliance.
Preserve the closed predecessor/failure algebra, finalizer settlement, and
other public-cancellation compositions, including finished/panicked finalizer
repair by cancel alone, then
rerun their gates and explicitly re-freeze for both independent reviews. Do not
reopen Slice A or otherwise migrate ordinary start/resume
admission reserved for Slice C, change recovery-domain policy/RPC, claim full
client Drop shutdown, or run full gates. Commit eligibility follows the
current incremental-commit decision below. `abort_all`/Drop only
requests or aborts the currently owned async handles; it has no Slice B drain
or completion claim for running blocking work or finalizer-captured
predecessors. Its existing test remains narrow unstarted-worker evidence.

Verified Recovery terminal settlement is accepted after review and core gates,
preserving revocation, hidden history, and the follower's exact queue position.

The capability cleanup parent-sync prerequisite is accepted after review, dual
core suites, strict lint, and formatting on Linux.

Phase-aware cancellation preparation is accepted after independent review,
dual core suites, strict lint, and formatting. Terminal retries no longer infer
deletion authority from a stale Pending projection or missing admitted snapshot.

Terminal intent restart confirmation is accepted after review and dual core
gates: existing restore can settle completed cleanup without repeating deletion.

Cancellation predecessor custody is accepted after independent review and dual
core gates. Interrupting a started finalizer cannot hide unfinished predecessor
effects or their failures. This does not establish client Drop drainage.

Explicit download shutdown is accepted under the Rust plan's
[bounded admission](rust-library-and-rpc/plan.md#explicit-download-shutdown-admission).
Core and IPC preparation share atomic invocation/task custody; real effects and
final interruption projection drain before retained HF/RPC results. Independent
reviews, SD-1 through SD-4, dual core/RPC suites, and strict affected lint pass on
Linux. This supersedes earlier abort-only descriptions for that population, not
for unrelated importers or the application's whole runtime.

C4 awaited ordinary/restored managed-download import is accepted under the Rust
plan's [importer admission](rust-library-and-rpc/plan.md#awaited-download-importer-admission).
Metadata/index success precedes settlement; retained failures remain retryable;
notifications follow release. C4-1 through C4-4, dual core/RPC suites, and strict
affected lint pass on Linux.

Physical-root execution exclusion is accepted under the Rust plan's
[bounded grant admission](rust-library-and-rpc/plan.md#physical-root-execution-grant-admission).
Phase custody, idle handoff, canonical admission validation, and busy consumer
behavior pass Linux gates. The existing desktop diagnostic enum and generated
validators migrated together; no persistence schema or live data changed.

**Next slice:** Validate `get_app_status` through its generated response contract,
actual preload and `usePlugins` consumer, preserving current identifier and
unknown-app semantics. Cached runtime-liveness RPC/main validation is accepted;
managed router synchronization retains its documented restart policy and evidence
limits. M4 and overall remediation remain incomplete.
The Linux CPU dedicated-profile hosting checkpoint remains accepted without
closing remaining M4 or overall remediation, following the accepted runtime-stop
response and the [standalone backend/link-health
slice](frontend-and-ui/plan.md#standalone-backend-and-link-health-contract-admission).
Import-picker, conversion progress reads and remaining conversion operation
contracts (FE-I24) are also accepted. The basic GUI format-conversion workflow
(FE-I23) is accepted within its documented evidence limits. Backend setup custody
and aggregate shutdown are accepted with Linux controlled-process evidence.
Setup identity/state and explicit retry admission are now accepted through core,
RPC and the desktop bridge. The optional dialog migration across uncertain
responses and reopen is accepted with built Linux GUI fixture evidence (FE-I25).
Quantization review found backend lifecycle and preflight prerequisites (FE-I26).
Retained conversion worker observation, atomic manager-local admission and
standalone/RPC shutdown composition are accepted. Unique conversion staging,
non-replacing publication and actual destination identity are accepted within
the documented stable-parent boundary. Foreground cancellation, dual-pipe
draining and direct-child reaping are accepted with Linux controlled-process
evidence. Cooperating Linux group cleanup now retains unreaped leader identity,
checks worker threads, preserves realtime exit signals and explicitly releases
setup exclusion after cleanup. Escaped descendants remain outside this bounded
contract. Worker receipts now own terminal progress after cleanup, publication
and indexing; script complete/error records cannot declare early completion.
NVFP4/Sherry nonterminal phases, completed-epoch progress and deferred script
failure now share the base conversion protocol, closing FE-I27 within controlled
Linux process evidence; public/wire types are unchanged. Built-in quantization
installers now share retained setup custody and root exclusion; manager setup
shutdown closes all owners before draining. Setup now verifies dependency imports
before skipping installation and before success, repairing incomplete existing
environments without recreating them. Controlled Linux fixtures establish this
setup contract, not real package/GPU compatibility or public readiness checks.
llama.cpp now checks required artifacts by selected route before staging/spawn:
GGUF needs no Python converter, safetensors requires Python and converter, and
IQ/forced calibration requires imatrix. Aggregate readiness remains advisory;
artifact metadata alone does not prove imports, effective access or loader compatibility.
Public quantization readiness now also verifies imports through bounded retained
async probes. Later reads are fresh, dropped waiters retain cleanup, and aggregate
setup shutdown closes/drains all matching probe owners. Sync reads remain
caller-owned; no real package/hardware compatibility claim is added. Native setup
now checks converter and both tools, rebuilds unusable generated outputs, and
verifies outputs before Python setup/success. Occupied output directories and
symlinks refuse cleaning. This is controlled recipe evidence, not real build or
source-revision coherence. [Backend-specific Rust setup observation/retry](frontend-and-ui/plan.md#backend-specific-rust-setup-observation)
is accepted, including a verification-discovered procfs disappearance correction.
The [RPC/optional desktop projection](frontend-and-ui/plan.md#backend-setup-rpc-and-desktop-projection)
is also accepted with generated request/response validation and controlled
RPC/preload/typed-consumer evidence.
[Managed setup-versus-conversion exclusion](frontend-and-ui/plan.md#managed-setup-and-conversion-exclusion)
is accepted through retained cleanup/publication/indexing within a stable root.
[Retained base-format readiness](frontend-and-ui/plan.md#retained-base-format-readiness)
is accepted with explicit probe failures and aggregate shutdown.
[Native setup source/build coherence](frontend-and-ui/plan.md#native-setup-source-and-build-coherence)
is accepted for the successful-setup path.
[Native setup interruption invalidation](frontend-and-ui/plan.md#native-setup-interruption-invalidation)
is accepted for process-exit/reopen and explicit repair. Later external mutation
provenance remains open. Test-only [admission wait diagnostics](frontend-and-ui/plan.md#download-admission-wait-diagnostics)
are accepted; FE-I28 stays open on recurrence without a diagnosed cause.
[Calibration-file preflight](frontend-and-ui/plan.md#calibration-file-preflight)
is accepted for shared classification and a bounded open/read probe, not content
quality or immutable custody. [Direct target validation](frontend-and-ui/plan.md#direct-quantization-target-validation)
is accepted across built-in backends, as is [direct importance-matrix option validation](frontend-and-ui/plan.md#direct-importance-matrix-option-validation).
[Source-file discovery](frontend-and-ui/plan.md#quantization-source-file-discovery)
is accepted for shared regular-file classification and llama.cpp error propagation.
The user [deferred further conversion work](frontend-and-ui/plan.md#conversion-scope-and-priority):
prefer existing runtime/tool capabilities when revisited; Sherry is not required.
[Hugging Face download-details responses](frontend-and-ui/plan.md#hugging-face-download-details-contract)
are accepted through generated decoding and the hydration consumer. Its
[request admission](frontend-and-ui/plan.md#hugging-face-download-details-request-admission)
is also accepted, as are [inference-settings reads and modal isolation](frontend-and-ui/plan.md#inference-settings-read-contract-and-modal-isolation).
[Library-model metadata response decoding](frontend-and-ui/plan.md#library-model-metadata-read-contract),
[inference-settings mutation admission](frontend-and-ui/plan.md#inference-settings-mutation-admission),
model-notes admission and the selected runtime-version read contracts are accepted.
Latest is the exact
[runtime installation-cancellation response](frontend-and-ui/plan.md#runtime-installation-cancellation-response-contract),
which distinguishes requested cooperative cancellation from terminal completion.
The raw nullable [runtime installation-progress response](frontend-and-ui/plan.md#runtime-installation-progress-response-contract)
remains accepted with generated camelCase validation and one explicit UI-domain projection.
The exact [runtime-version info response](frontend-and-ui/plan.md#runtime-version-info-response-contract)
and raw [runtime installation-validation response](frontend-and-ui/plan.md#runtime-installation-validation-response-contract)
remain accepted. Next, bound runtime-version removal responses without removing
a runtime.
This does not admit
Pending cleanup replay. FE-I29 retains a separate
intermittent preflight-test failure despite fixture hardening.
Independent probes, direct backend calls
and external tools still need caller coordination. FE-I28 records an unrelated
download-fixture timeout discovered during verification, not a diagnosed fix.
Stronger containment and remaining preflight remain prerequisites before GUI configuration
(FE-I23).
Managed quantization starts now reject foreign/invalid target names, missing
required IQ/forced calibration, unsupported force flags and supplied paths
that are missing, empty or not regular files before worker admission. This
does not establish execution readiness, calibration content or file custody;
direct backend execution retains its existing lower-level preconditions.
The GUI is independently optional; embedded Rust and standalone RPC remain
supported. Pending cleanup replay is deferred, not accepted: it still needs an
exact contract against durable authority and retained root custody. No old-format
support. Cargo/commits stay serial; full C3/M4 and program acceptance remain open.

The following checkpoints refine Slice C's integration order; they preserve its
existing end-to-end criteria and do not authorize independent release or commit.
C3 remains active, but its Pending replay cannot precede the newly identified
M4/C4 ownership prerequisites. The focused owner must admit each exact source
contract before implementation; historical checkpoint order is not permission
to bypass those dependencies.

| Checkpoint | State | Owned result and deciding evidence |
| --- | --- | --- |
| C1 — Store contract | Accepted (internal checkpoint) | Independent source review accepted; 38 store tests, atomic-publication regressions, and 155 HF regressions passed the C1 checkpoint. Subsequent core integration passes full package tests and strict lint in both feature configurations. |
| C2 — Destination authority | Accepted (internal checkpoint) | Independent source review accepted; root reproduced 14 recovery, 22 atomic, 38 store, and 155 HF tests in both feature configurations. One atomic and two store helper tests remain ignored. Both library checks pass with 23 unused-integration warning groups. |
| C3 — Lifecycle integration | Active | Core ticket recovery, legacy migration/relocation, and cancellation quarantine are independently reviewed and verified for incremental integration. Network-wait pause also passes independent review and dual full-package tests. Remaining: queued-pause ownership, hard process-crash recovery, owned comprehensive unresolved-state restore, admitted relocation, and guard-free effects/publication. See the focused Rust ledger for exact verification and lint gates. |
| C4 — Importer integration | Planned | The builder's actual async importer is observed and drained. Held Aux+cancel and Completion+queued-successor tests prove real mutation ownership; failed import retains bytes and resumes without downloading them again. |

Store repair owns `model_library/download_store.rs` and its colocated tests;
destination authority owns `model_library/download_recovery.rs` and affected
private HF reservation/marker paths; lifecycle integration owns the private
HF/store and relocation wiring; importer integration owns the admitted
builder/HF hook wiring.
These are subsets of the exact cumulative write set below, not permission to
alter other contracts. Each checkpoint uses focused evidence for its changed
invariants. The composed Slice C gate retains all existing A/B regressions and
production-path evidence; repeat review only for materially changed meaning.

The persistence comparison is complete: retain the JSON owner for C1's
immediate corrections. A SQLite replacement is a separate follow-up only after
real-file evidence proves database and journal path authority, durability, and
migration/handover, together with a net reduction in owned machinery. SQLite
transactions alone do not settle filesystem recovery. No further investigation
blocks C1; the [focused Rust plan](rust-library-and-rpc/plan.md) owns the
implementation decision.

After C1–C4, Slices D/E and M2J still precede the canonical producer handoff.
Frontend M4 then migrates generated contracts, root-scoped display-only caches,
and fresh recovery actions before representative GUI verification. This keeps
the user's model-display and startup outcomes in the acceptance path.

Slice C's cumulative exact source write set is
`rust/crates/pumas-core/src/model_library/hf/download.rs`, `hf/lifecycle.rs`,
`hf/types.rs`, `hf/mod.rs`, `model_library/download_store.rs`, and the existing
untracked `model_library/download_recovery.rs`, plus
`rust/crates/pumas-core/src/api/builder.rs`,
`rust/crates/pumas-core/src/metadata/atomic.rs`, and
`rust/crates/pumas-core/src/metadata/mod.rs`, plus the focused Rust plan,
ledger, issues, and `reports/rpc-contract-and-threat-model.md`.
`download_store.rs` is admitted only for the private v3 quarantine schema,
migration, durable queue inventory, strict atomic APIs, and tests described
below; `download_recovery.rs` is admitted only for the held physical
destination identity/authority shared by the reservation and effects.
`api/builder.rs` may only establish `ModelLibrary` first, then open its
already-selected accepted root once and inject that crate-private authority
into the client. It must cover `auto_create_dirs=false` with an initially
absent models directory and has no fallback to a path reopen.
`metadata/atomic.rs` and
`metadata/mod.rs` may only expose the accepted durable-publication mechanism to
a capability-relative marker target, preserving Slice A's typed pre-effect,
visibility-unknown, and durability-unknown algebra. This cumulative overlap
must rerun every accepted Slice A focused test and receive exact-hunk review;
it does not reopen Slice A's store protocol or admit another atomic writer. No
other Module, manifest, public API/RPC/IPC/UniFFI, frontend, platform, package,
generated, or shared-doc source is admitted.

Slice C migrates ordinary start and resume into the one Slice B lifecycle
owner without changing recovery-domain policy or wire outcomes. Complete all
awaitable authentication, remote/preflight, and destination dependencies before
admission where their failure must leave no task/state. Prepare a start-gated
Worker, then under the fixed download-state-to-task-owner lock order commit its
state, exact generation, destination authority, and real owner atomically;
release every guard and synchronously open the gate before any further await or
caller-cancellation point. `Queued`, `Downloading`, or `Pausing` may never be
published without that exact registered owner, and task installation failure
cannot be ignored. Directory/marker/persistence setup and admitted callbacks
that belong to execution run inside owned work, with setup failure projecting a
typed/sticky terminal result rather than a false active success. Ordinary and
restored work retain no recovery capability.

The commit also revalidates the exact existing dedupe/overlap predicate and
current recovery reservation under the same state-to-task critical section; an
earlier read cannot authorize duplicate same-file/destination owners or cross
capability-backed recovery. Do not prepare/register a gated entry and then
await another dependency: cancellation there strands owner state until
incidental rescue. Ambient resume preserves its exact prior Paused/Error state
until preparation plus gated install can commit together, and persistence must
precede any state claim it proves rather than late-writing Queued over a faster
Paused/Error projection.

The same atomic commit installs a private state-lifetime destination
reservation for the exact physical destination identity, download ID, domain,
and task generation. A raw or lexically canonicalized `PathBuf` does not prove
physical identity across aliases or replacement; the reservation and later
effects must derive from the same crate-private held capability/root plus
validated relative target owned by `download_recovery.rs`. This is
serialized ownership, not a synchronization guard held during publication:
short queue locks mutate custody, then release before start, wake, callback, or
broadcast. The configured model-library root is opened once after builder
directory setup and remains the runtime authority. Destination identity is
that held root identity plus a validated portable relative target, not an
ambient/canonical path or the identity of the nearest existing ancestor. Alias
spelling that resolves to the same live configured root and relative target
compares equal, including when the final target is initially missing and later
created. Root replacement, relative symlink substitution, escape, or held
identity mismatch fails closed before any effect. Persistence stores only a
non-authorizing root fingerprint and relative identity for restore comparison;
runtime effects stay handle-relative and revalidate the configured root.

Public admission is durable, not merely an in-memory queue insertion. The
atomic state/task/reservation commit installs a caller-independent
`AdmissionTransition`; before returning the ID, publishing active state, or
performing directory/marker/part/network/callback effects, that owner durably
persists the complete immutable request, destination identity, domain, exact
admission ordinal, and predecessor/release relation, then generation-matches
its promotion to Worker. Admission is a strict attempt-identified two-phase
`Intent`/`Unknown` to `Durable` protocol, not ordinary row save: phase one
durably parks the complete request and queue position, and only a confirmed
phase-two durability result authorizes promotion/public success. A typed
definitely-not-published phase-one result permits safe rollback. Visibility or
durability ambiguity in either phase returns a caller error but keeps hidden
custody parked; it cannot return an ID, publish active state, perform effects,
or release the queue. A later same-attempt owner or restart strictly rereads
and re-publishes the matching phase to a confirmed barrier before resolving;
plain presence or absence never guesses durability. Failure before durable
phase-one admission rolls back the unpublished state/reservation and returns
the typed failure; caller drop after
the internal commit cannot detach the transition. Concurrent persistence
completion order cannot change FIFO: restore sorts by persisted ordinal and
requires durable predecessor release evidence. A missing predecessor with no
durable release proof blocks conservatively instead of promoting the follower.
Legacy/v2 rows have no ordering proof; a uniquely matching marker/artifact
incumbent may be placed first, while ambiguity remains blocked for Slice D
recovery policy rather than guessed from UUID, vector order, or time.
The persisted queue admission itself carries a closed `Unknown` or
`Durable { attempt, predecessor_proof }` disposition. Phase two may write
Durable only after the same attempt's phase-one Unknown was confirmed durable.
If that phase-two publication is ambiguous, the initiating actor still fails
and parks; a later fresh owner may accept a strictly valid visible Durable
product under that confirmed-predecessor invariant, while visible Unknown
remains hidden and must be re-published. Promotion authority is the typed
confirmed call result or fresh predecessor proof, never an untracked runtime
cache or an ordinary-row presence test.

Exact duplicate reuse is permitted only when one current started
same-domain owner and the reservation both prove the exact current task
generation and whole bound file set. Partial overlap retains the full immutable
request, receives its own truthful ID, and waits in admission order. Paused or
recoverable Error parks the reservation as dormant because its
parts and marker remain resumable authority; resume or recovery promotes that
same queue position, cancel transfers it to the finalizer, and relocation moves
old/new destination custody without an unowned gap. Relocation is one
caller-independent state, durable-persistence, and reservation transaction:
failure preserves the old state/path/claim, while success makes the new durable
path, state, and dormant claim agree before waking the old-path successor. A
recovery owner can never
be reused as an Ambient result. Cleanup failure or panic parks sticky Error and
blocks successors until repeated cancellation verifies cleanup; only verified
terminal cleanup and publication release the reservation. A successful retry
does not erase accepted Slice B failure provenance: state remains Error,
`lifecycle_failure_unverified` remains set, and recovery capability/blocked
authority remains held even though verified cleanup permits the destination
queue to advance. A durable Pending/Verified cleanup disposition and the sticky
failure flag must reconstruct that distinction on restart before any admission;
Pending reconstructs and parks destination custody, while Verified reconstructs
the quarantined Error/capability without a reservation so a successor can
advance. Restore/start scanning may not re-reserve Verified quarantine, and
ordinary resume, relocation, duplicate reuse, and Worker promotion reject both
dispositions. Recovery Pending has one cleanup-only exception: a fresh verified
ticket matching the quarantined snapshot and current root may atomically
reattach held capability and install a CancelFinalizer, never a Worker, then
return the existing `Attached { status: Cancelling }` outcome. Stale, mismatched,
or Verified tickets are denied. In-memory evidence cannot authorize release or
resume. Token destruction is
signal-free and lifecycle rescue owns abandoned post-lock wake actions. Stop
and re-plan if this requires another source file, marker schema, or public
outcome. The public `HuggingFaceClient::new` signature remains unchanged and
does not infer a download root from its cache path. Product construction uses
the private builder-injected root; an unconfigured client may still search but
destination mutation fails with a typed unavailable result. Tests configure an
explicit temporary root through crate-private seams.

`download_store.rs` v3 owns the durable lifecycle quarantine. Its private map is
keyed by download ID and stores the complete reconstructable snapshot, Ambient
or Recovery domain, an independently validated sticky-lifecycle-failure fact,
and cleanup disposition Pending or Verified. Pending alone never implies
sticky Error: it also represents a clean cancellation interrupted before
removal, so the persisted product must distinguish clean retry from
fail-closed verification. `VerifiedIntent` or `Verified` with a false sticky
fact is an impossible/corrupt v3 product and strict load rejects it. Quarantine
exclusively owns that snapshot: its ordinary
row is removed in the same Pending publication, and ordinary save, status, or
relocation rejects the quarantined ID. Ambient quarantine excludes a recovery
revocation, Recovery quarantine requires and preserves a `Durable` revocation;
`DurabilityUnknown` is invalid quarantine authority, and neither may
coexist with an ordinary row. Clean cancellation without sticky provenance
uses a strict idempotent atomic Pending removal before publishing Cancelled,
preserving a Recovery tombstone and recording an exact cleanup-attempt/queue
release proof; sticky failure moves
Pending-to-Verified and retains its Error snapshot. Legacy/v2 Error migrates as
recoverable because those formats contain no quarantine evidence. Strict load
and every v3 mutation use Slice A's in-instance plus OS lock and durable
publication algebra. A failed or visibility/durability-unknown publication
never promotes Pending to Verified, releases destination custody, or
fabricates an empty restore.

The same v3 document owns a private destination-queue inventory. It stores only
the non-authorizing destination identity, ordered download references,
admission ordinal/predecessor truth, domain, and durable release/disposition;
an admission attempt, ordinary row, or exclusive quarantine entry is the one
full-snapshot owner and those forms never coexist for an ID. Phase one creates
the attempt-owned `Intent`/`Unknown` plus queue position; phase two atomically
moves the exact attempt snapshot to the ordinary row and marks its queue
admission Durable. Row admission plus queue insertion, relocation plus old/new
queue transfer, Pending quarantine plus ordinary-row removal, Verified
promotion plus release, clean Pending removal plus terminal release, and clean
terminal row removal plus release are each a single strict durable store
mutation. No ordinary mutation may bypass quarantine or create an orphan queue
reference. Restore never treats file order, UUID, `created_at`, or store-write
completion order as FIFO authority.
Begin/idempotent adoption compares the entire immutable admission identity,
including payload and execution file sets, destination/domain, ordinal, and
predecessor relation; a stale owner cannot adopt a newer quarantine. Clean
removal returns a typed distinction among removed by this exact attempt,
already removed by this exact attempt, stale/mismatched, and never present.
After post-effect publication ambiguity, the same attempt may safely retry from
the durable queue-release proof, while an unrelated fresh caller cannot treat
absence as permission to publish Cancelled or release custody.

V3 decoding is version-specific and strict; legacy compatibility defaults may
not supply a missing v3 identity, ordering, owner, attempt, or disposition
field. Ordinal allocation is checked against overflow and `(destination
identity, ordinal)` is unique across Ambient and Recovery together. A
predecessor shares the physical destination and has a lower ordinal, but may
have the other domain; domain governs provenance/reuse rather than partitioning
filesystem custody. The graph is acyclic. Every unreleased entry has
exactly one full-snapshot owner in admission, ordinary, or quarantine state;
every such owner has a queue reference except an explicitly migrated blocked
legacy ambiguity. A released predecessor may be snapshotless, and a missing
unreleased predecessor is valid-but-blocked rather than corruption or a reason
to auto-promote. Release proof binds the exact entry, attempt, and generation,
remains while any follower references it, and is garbage-collected by a strict
atomic mutation only after the last reference disappears.

Provisional admission state is not inserted into the ordinary `downloads` map
or any public projection before the confirmed Durable barrier. A separate
private admission owner participates in dedupe: a concurrent exact request
waits on or attaches to that same attempt instead of returning the provisional
ID or admitting a duplicate. Unrelated list/snapshot publication cannot expose
the hidden request.

Atomic-unknown handling also covers relocation and every terminal row-plus-queue
release. Each is an attempt-identified intent/confirmation transition, not a
collapsed `bool` or generic error. Relocation intent owns and parks both old and
new destination claims across visibility/durability ambiguity; neither “old
preserved” nor “new committed” may be published until strict same-attempt
reread and a confirmed barrier resolves the durable product. Terminal-release
ambiguity retains the runtime claim, suppresses terminal publication, and does
not wake a successor until strict resolution proves the row/queue release.
Caller drop or process restart transfers resolution to the lifecycle/restore
owner without converting unknown into success or safe rollback.

Pause may transition only a matching started, nonfinished Worker; the same
generation owns the later `Paused` projection after destination/blocking work
drains, and a required durable update succeeds before Paused is published.
Failure or nested drain error is observed and fails closed. Cover both
install-before-start and after-pause-check/before-Downloading-projection races
so neither can strand `Pausing`. Once exact final-file completion is committed,
pause loses before marker or persistence cleanup can remove resumable truth.
An exact-generation terminal-intent projection linearizes that winner before
the final rename; rename or cleanup failure then settles Error rather than
reopening pause. Logical destination release occurs only after every owned
blocking effect and observer completes the final drain.
Pause uses an owner-visible wake signal at header, stream, and retry suspension
points, so a stalled network future cannot indefinitely retain a Pausing state.
Cancel continues through Slice B's finalizer, and reconciliation
observes every finished owner. Move every snapshot broadcast out from under the
destination lease. One private publication owner must linearize current-state
capture, revision allocation, and dispatch so an older delayed publisher cannot
deliver after a newer revision/state, while the external send occurs with no
destination/download/task guard. Remove the earlier direct ordinary-start auxiliary callback
from the async request path or route it through the already admitted
`TaskContext` isolation after lease release. No external callback or publication
signal runs under any of those guards, and no inline/unowned blocking work or
filesystem/network I/O runs under task/download-state guards. Do not invoke or
await isolated blocking setup, persistence, marker, or file work while holding
the physical destination mutex either. The state-lifetime logical reservation
replaces that mutex as the Worker effect-serialization owner and spans async
filesystem/network work plus guard-free publication without itself protecting
shared memory. This is
the exhaustive C correction for the two defects Slice B explicitly held, while
its strict completion/cancellation policy and all B custody invariants remain
unchanged.

Production importer mutations are not synchronous callback notifications.
`api/builder.rs` installs crate-private asynchronous Aux and Completion mutation
hooks that return their real typed result; it must not wrap them in a
`RuntimeTasks.spawn` callback that returns before the mutation. `TaskContext`
registers each hook as generation-owned asynchronous nested work before it can
run, catches invocation and polling panic, observes its result, and transfers
its handle plus semantic failure to a replacing finalizer. The existing public
synchronous callback types/setters remain notification-only Interfaces and run
through TaskContext-owned blocking isolation; no new public type or outcome is
admitted.

The Aux mutation runs and drains under the logical destination reservation
after auxiliary bytes and before weight work. Cancellation may replace the
Worker, but the finalizer drains the real mutation before cleanup and terminal
publication; after the waiter returns, the Worker revalidates its exact
generation, role, state, destination identity, and terminal intent before any
further effect. Expected mutation error projects recoverable Error with dormant
custody, while invocation/poll panic, join error, or owner failure preserves
sticky quarantine; concurrent cancel cannot permit a post-cleanup metadata
write. The public Aux notification, if configured, follows successful mutation
through owned blocking isolation and the same post-callback revalidation.

After the exact final-file terminal intent wins, pause and cancel may not
replace completion finalization. The owned Completion mutation runs while the
logical destination claim is still held and before marker/persistence cleanup
or `Completed`. Expected importer failure preserves byte completion, marker,
durable row, and dormant recoverable Error; a later exact resume retries only
import/final cleanup, never network bytes. Panic, join, or owner failure enters
sticky quarantine. On success, strict marker/row/queue cleanup and preterminal
drain precede `Completed` publication, then the exact destination claim is
released. Only afterward may the public synchronous completion notification
run as TaskContext-owned blocking work, followed by a notification-only drain
before the outer owner exits. Notification failure is observed and cannot roll
back `Completed` or reclaim the destination.

Marker creation uses the same held destination authority and the canonical
metadata durable publisher: collision-resistant exclusive staging, write plus
flush and file sync, atomic capability-relative rename, parent sync, and exact
configured-root/parent identity validation. It never truncates the visible
marker or cleans a foreign staging collision. Pre-effect failure, staging
failure, post-rename visibility ambiguity, and parent-sync durability
ambiguity are typed and fail closed before part/network/callback effects; the
marker schema is unchanged. This remains Linux local-filesystem incremental
evidence under Slice A's target limits: macOS is pending, non-Unix is typed
unavailable, and network/distributed filesystems are unsupported.
The marker target is constructed from the held root directory, validated
relative parent, and filename; it may not call the existing ambient
`AtomicJsonTarget::open(display_path)` or fall back to any path reopen. Staging,
write, rename, cleanup, sync, and the post-effect identity comparison all remain
capability-relative under root/parent replacement.
The existing pretty-serialized marker string must not be passed as a JSON
string to `publish_json`, which would double-encode it. Carry a structured
private marker value or validate and publish pre-serialized object bytes, and
assert that the resulting marker decodes to the unchanged object schema.

Destination settlement is exact-generation and typed (`Released`,
`AlreadySettled`, `Stale`, or `Missing`) with a reverse generation-to-claim
index. Worker, CancelFinalizer, or successor TerminalProjection may release
only after checking state and the strict durable disposition under the
download-state lock, then waking a successor after all locks. Eligible states
are durably cleaned and published `Completed` or clean `Cancelled`, or durably
`Verified` and published sticky quarantined `Error`. `Paused`, recoverable
`Error`, and quarantine `Pending` never release. If a Worker panics after
durable cleanup plus `Completed` publication but before release, or a finalizer
panics after durable `Verified` plus sticky `Error` publication but before
release, TerminalProjection owns the idempotent rescue; stale generations
cannot release a successor.

Restoration starts from a strict store load and installs caller-independent
lifecycle plus destination custody for every candidate before inspection,
finalization, removal, publication, callback, or artifact effect. Every durable
result is observed before publication or reservation release; caller drop may
not detach restoration, and load/removal failure is not converted to empty or
success. Slice D
continues to own fresh-filesystem-versus-stale-status reconciliation policy and
Slice E owns its aggregate closed failure evidence. Successful completion
follows the owned mutation/import, strict terminal cleanup, `Completed`,
release, and notification-only order above; restoration uses the same mutation
hook rather than a second fire-and-forget importer path.

All ordinary start/resume/pause persistence save or update work is registered
with `TaskContext` before its first await/effect and its result is consumed. A
fresh start's initial v3 admission failure rolls back before public success;
failure of a mutation for an already durable row settles sticky `Error`.
Neither a late best-effort save nor an ignored `false`/error may regress durable
Error/Paused state to Queued, falsely publish Paused, or outlive cancellation;
no persistence work detaches.

Red-first deterministic evidence must cover caller cancellation immediately
before atomic commit and immediately after commit/start; auth, remote,
destination, directory/marker, persistence, callback, and task-installation
failure; same-context contention; ordinary and recovery resume cancellation
boundaries; pause before destination-lock Worker execution; retry/pause/cancel
interleavings; callback and Worker panic; finished-owner observation; repeat
cancel; and start-token rejection/rescue. Prove no false
Queued/Downloading/Pausing, active-but-unregistered state, detached work,
duplicate owner/callback, stale destination mutation, or lost terminal outcome.
Add reentrant destination-lease/snapshot/callback oracles proving all external
signals occur after guard release. Hold an older snapshot after candidate
capture, publish a newer mutation, then release them in reverse and prove
monotonic revisions/nonregressing payload. Hold/fail initial save and prove no
ID/state/effect before its durable success or after typed rollback; hold/fail late resume
Queued save, pause update `false`/error, and post-state/pre-registration cancel
to prove persistence never detaches or overwrites authoritative state. Exercise
two real same-destination Workers, exact and partial overlap, reverse task
scheduling, active/waiting/dormant cancellation, pause/Error retention,
recovery-domain refusal, relocation, marker serialization, cleanup panic and
retry, terminal-only FIFO wake, and abandoned wake rescue. Relocation evidence
holds old/new queue successors and persistence around both commit outcomes;
terminal evidence holds the final rename and the post-terminal drain; exact
reuse proves matching task and reservation generations. Add crash/restore
evidence for a queued peer with reverse persistence completion, a missing
unreleased predecessor, a durably released predecessor, and conservative
legacy ambiguity. Prove same-root aliases and missing-to-created targets share
identity while missing-ancestor creation is stable and root/target or relative
symlink replacement fails closed. Inject marker failure before staging, after
staging write, after rename, and during parent sync, proving no false success
or foreign-temp cleanup. Inject both post-Completed/pre-release Worker panic and
post-Verified-Error/pre-release finalizer panic with a queued successor, proving
one idempotent rescue, typed stale/duplicate settlement, and no wake before
durable eligibility. Exercise quarantine begin, verify, and clean removal at
pre-effect, post-effect, durability-unknown, and killed-between-phase seams;
fresh reconstruction must preserve sticky-versus-clean intent and the Recovery
tombstone. Keep requested model payload files distinct from execution-only
auxiliaries: artifact identity and primary hash derive only from the requested
payload, while durable expected/execution files may include validated
auxiliaries; cover a larger non-weight LFS auxiliary during partial overlap.
Exercise both admission phases at pre-effect, post-rename,
parent-sync-unknown, and killed-between-phase seams: only confirmed Durable may
return/promote for the initiating call, ambiguity parks that exact actor,
fresh valid Durable uses its predecessor proof, and visible Unknown must
re-barrier rather than infer from row presence. Cover builder construction with
an initially absent root under `auto_create_dirs=false` and root replacement.
Exercise relocation and terminal-release intents at the same pre/post-effect,
parent-sync, caller-drop, and crash seams, proving relocation parks both paths
while unknown and terminal ambiguity retains its unpublished claim/no wake.
Malformed-v3 fixtures cover missing required fields, overflow/duplicate
ordinals, wrong-destination or later predecessors, cycles, duplicate/missing
snapshot owners, mismatched release attempt/generation, both cross-domain queue
orders, and release-proof garbage collection. Hold phase-two admission while an
unrelated mutation publishes: the provisional ID remains absent, and an exact
concurrent start waits on the same attempt rather than duplicating it.
Production-topology callback tests use the builder-style real async hook rather
than an inline synchronous fake. Hold Aux mutation, cancel, and prove finalizer
drain precedes cleanup with no later metadata write or Worker effect. Hold the
Completion importer with a same-destination successor and prove no
`Completed`/release/successor effect until import and strict cleanup succeed;
then hold the notification after release and prove the successor may advance
while the callback remains owned/observed. Importer error preserves completed
bytes plus marker/row/dormant Error and exact resume performs no network work;
panic/join failure quarantines and no success notification fires.
Rerun every accepted Slice A and Slice B regression,
focused lifecycle/download tests in default and no-default modes, both affected
all-target Clippy modes with warnings denied, format, scoped diff/write-set,
and current Standards plan gates, then explicitly freeze for both reviews.
Stop rather than widening if a closed public outcome or another owner is
required.

Slice D, Slice E, M2J, full aggregate acceptance, and general Drop drain
remain held. The approved bounded downstream integration is defined below.
Verified incremental commits follow the
current commit decision; incompatible producer-only integration remains held.
Re-review M2I's recovery projection before starting
M2J. Accept Rust Milestone 2 only
after every reachable
operation is closed and the temporary `Legacy` contract is absent, and hand
that canonical producer
Interface to platform Milestone 1. The user's 2026-09-05 `continue` decision
accepts PRG-I21's correctness-safe roughly one-second marker delay and
authorizes completing the held draft integration before committing it,
including the required staged UniFFI adapter. Both the terminal-bootstrap and
hidden-capture alternatives failed; no immediate but causally unproved reveal
may be admitted. Neither launcher-root tranche may integrate
until the no-bridge negative and immediate/delayed/never-terminal composed
oracles pass.

**Approved bounded downstream integration:** Read-only cross-owner review confirms
the current renderer cannot consume the candidate closed Rust catalog or
`{modelId,recoveryToken}` recovery action and cannot treat its unscoped v1
snapshot as evidence for the selected launcher root. Before source admission,
M4-S0 inventories every consumer and supersedes the old cached-unknown-age and
universal causal-first-frame claims. Platform then owns a synchronously decoded
terminal bootstrap with an opaque stable `libraryScopeId` and one generator
that emits both Electron-decoder and renderer-consumable contract projections.
Frontend owns a deep closed Catalog Projection Module and a strict v2,
display-only, root-scoped snapshot; v1, corrupt, mismatched, or cold state is
ignored/evicted and renders honest `Loading Library`. Cached/degraded rows are
read-only and never retain model paths, recovery capability, activity, or
action state. Fresh partial recovery alone may invoke exact
`{modelId,recoveryToken}` through model-ID single-flight, while unassociated
downloads remain explicit orphan rows rather than repo/name/quantization
guesses.

For this coordinated replacement, the same 2026-09-05 decision supersedes the
sequence hold requiring all Rust M2I Slices B-E and M2J before consumer edits.
It does not accept those unfinished milestones or general application Drop
drain. Reconciliation retains an admitted run through caller cancellation;
RPC catalog and FTS share one projection with owned blocking work; process
fixtures use current owner admission, not legacy store serialization. M4-S1
adopts generated catalog types and builds the projection core; M4-S2 owns the
root-scoped snapshot/lifecycle and PRG-I21 consumer; M4-S3 migrates row,
activity, action, and runtime consumers; link health, picker, and generated
conversion-type deletion are separate overlapping M4-S4 dispositions. Exact
write sets and evidence are recorded in the program ledger. This bounded
integration admits the required frontend/platform source, canonical generated
contract, and root-scoped display-only bootstrap; it does not admit unrelated
release or Torch work. PRG-I21 retains the representative startup workload and
approved marker barrier, with no universal compositor-causality or arbitrary
one-sample threshold claim. This coordinated checkpoint and subsequent RUST-I12
Linux collision remediation are complete; their ledger entries own verification.

Platform may also continue
independent Torch request-contract narrowing, but PRG-I17 requires a serialized
managed-deployment disposition after the current Rust producer boundary and
before DRBT-A5 required-real work. Windows/macOS launcher evidence remains
unavailable until required-real target runners execute it.

**Acceptance status:** `blocked`

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Reports:** Program evidence is indexed in the [ledger](execution-ledger.md#reports).

**Audit source:** [Current standards audit](../../audits/current-standards-2026-09-03/README.md)

## Objective

Remediate every finding in the 2026-09-03 standards audit through four focused
owners, so Pumas preserves authoritative model state, rejects invalid or
unauthorized cross-process input, completes asynchronous work truthfully,
presents state accessibly, and ships only configurations and artifacts backed
by the evidence their support claims require.

This plan owns program sequence, cross-plan handoffs, and objective-level
acceptance. It does not duplicate the implementation decisions or write sets
owned by the focused plans.

## Baseline

- Audit code baseline: `a33c8c0efa7cd8783c7deeac9e608db205290d43`.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Execution-start code baseline: `453105780b1e5181d27dd1f20b234591bb6ead86`.
- Audit standards baseline: `52b096ded9c53afd439a3cf0efc4cc85252da570`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Execution-start standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Current validation standards baseline: `ef400727e2d81a467af64b95f64e7c631096faee`.
- Documentation cleanup `d84e2b35` is an accepted prerequisite and is not
  repeated by this program.
- No source implementation or objective acceptance evidence is created by
  authoring these plans.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| PRG-A1 | A hostile or malformed caller cannot disclose credentials/internal locators or invoke protected remote RPC operations, while authorized supported operations retain typed results. | `system` | `representative` (real debug RPC process and isolated network clients) | `automated` | `blocked` | [Rust RUST-A1 satisfied](rust-library-and-rpc/reports/rpc-disclosure-evidence.md) and [RUST-A3 loopback-only exposure satisfied](rust-library-and-rpc/reports/rpc-contract-and-threat-model.md#accepted-exposure-decision); platform DRBT-A2 remains |
| PRG-A2 | Requests, responses, errors, and events traverse Rust, Electron, preload, and renderer through one producer-owned contract; invalid, unsupported, unavailable, and failed outcomes never become valid-looking defaults. | `system` | `required-real` (built RPC and Electron process path) | `automated` | `blocked` | Loopback-only exposure accepted; Rust RUST-A2, platform DRBT-A1/DRBT-A2, and frontend FE-A1 remain |
| PRG-A3 | Interrupted model mutations/events, supported schema migrations, and launcher-root updates recover without missing durable history or silently selecting another library authority. | `system` | `required-real` (real SQLite and every accepted desktop filesystem/OS target) | `either` | `blocked` | Conservative desktop target matrix plus [incremental persisted-authority and Linux-local atomic-publication evidence](desktop-release-bindings-and-torch/reports/desktop-lifecycle-evidence.md) at `1964760d` and `767e71f0` accepted; Rust RUST-A4/RUST-A5 and renderer-recovery/required-real platform DRBT-A3 remain |
| PRG-A4 | Rust, Electron, frontend, launcher, and Torch work owners observe admission, supersession, cancellation, failure, deadlines, and bounded shutdown without detached work, false completion, or starvation of control traffic. | `system` | `required-real` (real runtimes, accepted OS targets, and resolved Torch stack) | `automated` | `blocked` | [Frontend FE-A3 restored](frontend-and-ui/execution-ledger.md) after PRG-I12; plugin failure/disabled and non-shipped Torch dispositions accepted; Rust RUST-A6 and platform DRBT-A4/DRBT-A5/DRBT-A6 remain |
| PRG-A5 | Cached model state, recovery, dialogs, popups, progress, motion preference, and both renderer modes behave truthfully and accessibly through representative built-renderer workflows. | `user-workflow` | `representative` (built renderer in supported Electron/Chromium runtime) | `automated` | `pending` | [Frontend FE-A4 satisfied](frontend-and-ui/execution-ledger.md#2026-09-03--m2-s4-representative-chromium-evidence-accepted) and [FE-A5 satisfied](frontend-and-ui/execution-ledger.md#2026-09-03--m3-s3b-popover-motion-and-terminal-semantics-accepted); FE-A2/FE-A6 remain |
| PRG-A6 | Every accepted feature configuration, host binding tuple, desktop target, and release artifact is supported by an explicit consumer matrix and matching real-target/cohort/final-byte evidence. | `release-artifact` | `required-real` (every accepted target, host/runtime, and assembly environment) | `either` | `blocked` | [Conservative support contract accepted](desktop-release-bindings-and-torch/reports/release-and-host-contract-decision.md); Rust RUST-A7/RUST-A8 and platform DRBT-A7/DRBT-A8/DRBT-A9 evidence remain |
| PRG-A7 | Contributor guidance and every retained permanent gate follow the current standards route and state their exact claim, oracle, schedule, overlap, and blocking authority. | `contract` | `representative` (repository and Linux pinned toolchain) | `either` | `satisfied` | [Governance GOV-A1 through GOV-A5](governance-and-verification/reports/final-governance-evidence.md) |

## Scope

### In Scope

- All CS-01 through CS-15 findings and their focused-audit expansions.
- The producer/consumer contracts, persistence, lifecycle, user workflows,
  configuration matrices, bindings, release evidence, launchers, documentation,
  and governance needed to close those findings.
- Bounded design investigations admitted by the focused plans where missing
  product, compatibility, environment, or support facts can change a
  high-consequence implementation decision.
- Cross-plan sequencing and objective-level integration/release evidence.

### Out Of Scope

- New inference providers or unrelated product features.
- Decomposition justified only by file size, complexity counts, or repository
  shape.
- Expanding LAN, platform, host-language, OpenAI-compatibility, or release
  promises without an accepted consumer and adequate evidence.
- Signing, notarization, registry publication, or GitHub release publication
  unless an accepted release contract later brings it into scope.
- General accessibility certification beyond the audited workflows.
- Rewriting historical audit evidence to describe post-audit code.

## Focused Plan Ownership

| Plan | Canonical ownership | Principal audit findings |
| --- | --- | --- |
| [Governance and verification](governance-and-verification/plan.md) | Standards routing, count/error gate disposition, permanent-gate claims and schedules | G-01–G-06; CS-10, CS-13, CS-14 and G-06 evidence routing |
| [Rust library and RPC](rust-library-and-rpc/plan.md) | Rust/server RPC and local IPC contracts, public errors/redaction, SQLite state/events/migrations, Rust lifecycle/features, Rust binding placement, plugin startup | R-01–R-09; CS-01–CS-03, Rust portions of CS-04/CS-07/CS-08, CS-11 |
| [Desktop, release, bindings, and Torch](desktop-release-bindings-and-torch/plan.md) | Generated Electron projections/decoding, desktop authority/lifecycle, Torch, launcher, host cohorts, release artifact/dependency evidence | P-01–P-10; desktop portions of CS-02/CS-04/CS-06/CS-07, CS-12, CS-15 |
| [Frontend and UI](frontend-and-ui/plan.md) | Renderer consumption, cached model provenance, installation lifecycle, interaction Modules, status/motion, renderer variants | F-01–F-08; CS-05, CS-09, renderer portions of CS-02/CS-04/CS-08 |

Shared findings close only when every named producer and consumer claim passes;
one focused plan cannot accept another owner's behavior by agreement alone.

## Constraints And Assumptions

### Constraints

- Security containment precedes broader refactoring. The Critical credential
  disclosure slice may not wait for the complete RPC redesign.
- Each implementation invocation names one canonical focused `plan.md` and an
  explicit `start`, `continue`, or `verify` operation.
- The focused plan owns its source write set and lifecycle. This program plan
  records only handoffs and aggregate state.
- Shared manifests, CI, package scripts, generated artifacts, schemas, and
  current documentation are serial integration-owner writes.
- Required-real evidence cannot be replaced by compilation, fakes, generated
  freshness, startup smoke, or another plan's local tests.
- Unsupported capabilities are removed or made explicitly unavailable rather
  than retained behind empty/default behavior.

### Assumptions

- Platform Milestone 0 can establish the actual release consumers, channels,
  target matrix, binding host matrix, and evidence/legal owners without first
  changing release automation.
- The focused plans' bounded populations cover the audited systemic families;
  each expands only when a newly discovered site shares the same authority or
  consumer promise.
- Existing useful strict typing, Electron isolation, Rust unsafe defaults,
  launcher structure, and focused tests are preserved unless direct evidence
  invalidates them.

## Binding Decisions

| Decision | Owner | Evidence | Supersedes |
| --- | --- | --- | --- |
| Critical RPC diagnostic disclosure is the first implementation slice. | Program plan | CS-01/R-01 severity and independent reversibility | Broad governance-first recommendation in the audit |
| Rust/server owns desktop RPC semantics and public error/redaction; Electron generated code is an Adapter/projection, and the renderer consumes only decoded outcomes. | Rust, platform, and frontend plans in that order | CS-01/CS-02 and cross-plan review | Hand-maintained, asserted, and fallback contract copies |
| SQLite index state/event/migration recovery and launcher-root authority remain separate Modules because their stores, consumers, and lifecycles differ. | Rust and platform plans | CS-03/CS-12 | One generic persistence framework |
| Feature, host, target, channel, and artifact support is selected from real consumers before build/release machinery is admitted. | Rust feature milestone and platform Milestone 0 | CS-06–CS-08 and Release/Binding standards | Incumbent CI/generator output as support authority |
| Desktop RPC is loopback-only: remove `--allow-lan` and reject non-loopback `--host` values. | Product/program owner and Rust plan | RUST-I1 consumer inventory found no production LAN consumer | An unauthenticated remote exposure promise |
| Plugin support compiled out reports disabled/unavailable; configured compiled-in subsystem initialization failure fails startup without temporary-root substitution. Optional post-start plugin failures may be typed per-plugin unavailable only where the existing contract proves that distinction. | Product/program owner and Rust plan | RUST-I2 and the requested compile-gated plugin product direction | Treating configured subsystem failure as degraded success |
| The preview channel supports only Linux x64 AppImage/deb, Windows x64 NSIS/portable, and macOS arm64 DMG, each gated by real evidence. Remove the unconsumed `.crate` and UniFFI/Rustler/Go surfaces; Torch is non-shipped until a real tuple is proved; the repository maintainer owns manual promotion and notice acceptance. | Product/program owner and platform plan | [Milestone 0 decision report](desktop-release-bindings-and-torch/reports/release-and-host-contract-decision.md) | Incumbent release assets and source scaffolding as support claims |
| Installation status must be reachable through the real install-dialog owner chain; deterministic active/terminal auto-presentation is repaired with red regressions before M3-S3 runtime evidence. | Program and frontend plans | FE-I13/PRG-I10 runtime-admission contradiction | Fixture-only evidence for unreachable UI |
| Governance removes weak proxy gates before later plans add claim-directed permanent evidence to shared schedules. | Governance plan | CS-10/CS-13 | Repairing legacy gate machinery by convention |
| Link-health refresh and model-import picker rejection failures discovered during governance classification remain in this program. Before frontend Milestone 4, the frontend owner must classify them into that existing authority or admit one exact focused slice; active Milestone 0 absorbs only installation-cancellation feedback. | Program and frontend plans | [Governance error-contract disposition](governance-and-verification/reports/error-contract-gate-disposition.md) and PRG-I5 | Deferring both findings outside this program |
| Historical audit text remains fixed to its baseline; plans and current guides own forward implementation authority. | Documentation/program owner | Audit baseline and documentation workflow | Editing evidence into current instructions |
| PRG-I19 remains one Critical program gate but is implemented as five serialized review slices: durable publication/revocation; deep task/blocking ownership; ordinary lifecycle migration; recovery-domain correctness; then closed/composed evidence. Verified incremental commits may preserve completed coherent outcomes; M2J/downstream consumer implementation and incompatible producer-only integration remain held. | Program and Rust owners | Independent review found unrelated persistence, lifecycle, domain, and evidence failures that evolve and fail independently | Repeated broad patches to `hf/download.rs` with one aggregate test boundary |
| The user authorizes coherent verified incremental commits and, on 2026-09-05, completion of the required coordinated consumer integration, staged UniFFI adapter, and correctness-safe roughly one-second PRG-I21 marker barrier. Verify actual candidate contents and preserve compatible reachable contracts. Release, full C3 acceptance, and incompatible producer-only integration remain excluded. | Program integration owner; focused owners supply candidate evidence | User direction to complete integration fixes before commits and collision remediation | Treating a commit as whole-plan acceptance or relying on checks from a different candidate |

## Evidence And Oracle Plan

| Claim | Domain | Deciding oracle | Independent authority | Unsupported domain | Intended negative failure |
| --- | --- | --- | --- | --- | --- |
| PRG-A1 | Security/diagnostics | Captured real-process responses and diagnostics plus hostile-client outcomes | Safe public-error and accepted exposure contracts | Arbitrary future log sinks | Sentinel secret/path appears, or unauthorized operation succeeds |
| PRG-A2 | Cross-process semantics | Real Rust-to-Electron-to-renderer scenario plus closed negative contract corpus | Producer contract and independently observed consumer result | Domain correctness unrelated to transport | Malformed value reaches presentation or becomes empty/default success |
| PRG-A3 | Durable authority | Controlled interruption, cold reopen, and authoritative row/event/root comparison | Accepted store/root formats and recovery policy | Hardware failures outside declared filesystem contract | Missing event, duplicate effect, guessed migration, or silent root switch |
| PRG-A4 | Lifecycle | Owners expose and tests observe every applicable terminal state at real runtime seams | Accepted state machines, deadlines, and external responsiveness | Unclaimed hardware/provider behavior | Detached work, stale completion, starvation, hang, or false shutdown success |
| PRG-A5 | User workflows | Built-renderer keyboard/accessibility/state/mode observations | Browser accessibility tree, controlled backend outcomes, and build-mode configuration | General certification or packaged contents | Cached state appears fresh, focus/status/motion contract fails, or wrong mode UI appears |
| PRG-A6 | Shipped support | Real target/host execution and exact extracted final artifact inspection | Accepted consumer/support matrices and final resolved bytes | Unadvertised tuple/channel | Mismatch, missing/extra file, incomplete provenance/notices, or absent target evidence |
| PRG-A7 | Governance | Executable config cross-review and affected retained command results | Current standards and named gate claims | Product behavior owned by focused plans | Count/regex proxy or unmapped scheduled gate remains |

## Systemic Finding Audit

- Invariant family and canonical owner: diagnostic safety/RPC semantics,
  durable state, lifecycle, renderer truth/accessibility, support/release
  evidence, and governance each have the focused owner named above.
- Bounded authority, representation, and reachable consumer population: the
  four focused plans enumerate their Rust, Electron, frontend, Torch, launcher,
  binding, release, documentation, and tooling populations. The program owns
  only cross-plan handoffs and combined acceptance paths.
- Expansion facts: add a population only for a new semantic owner, reachable
  consumer, persisted/public promise, supported tuple, or material risk in the
  same invariant family.
- Consumer dispositions: migrate, already-safe, delete, explicit unsupported/
  unavailable, or named follow-up owner; no unclassified consumer can close a
  systemic claim.
- Deletion, consolidation, smaller-Interface, stronger-proof, and evidence-
  replacement alternatives: each focused plan prefers deletion of unsupported
  paths/proxy tests, one producer contract, existing deep Modules, and direct
  scenario evidence before new registries, Adapters, or frameworks.
- Evidence-backed stopping condition: all CS-01 through CS-15 mappings have
  accepted focused claims and every program acceptance path has its stated
  objective-level evidence.
- Repaired-composition comparison: runtime Modules stay independently owned;
  contract and release knowledge propagates through small Interfaces and
  generated Adapters rather than synchronized hand copies.

## Simplicity And Ownership Review

**Applicability:** `applicable`

- Independent concepts and dimensions: security, wire semantics, durable
  authority, async lifecycle, renderer interaction, support configuration,
  release evidence, and governance change independently.
- State, identity, value, time, policy, and mechanism: focused plans keep these
  roles separate within their Modules; this plan owns only sequence, handoff
  identity, finding disposition, and aggregate acceptance.
- Caller and composition-root knowledge: implementers select one focused plan
  and learn only its next slice; the program integration owner learns the
  dependency graph and acceptance claims, not every implementation mechanism.
- Representative change paths and forced owners: an RPC field flows producer
  to generated Adapter to renderer consumer; a release-target change flows
  support matrix to build/host evidence to final assembly; local changes do not
  force unrelated plans.
- Stable Interfaces versus hidden knowledge: accepted DTO/error revisions,
  decoded outcomes, persisted-state results, lifecycle results, support
  matrices, and focused-plan evidence are stable Interfaces; SQL, generator,
  transport, focus, queue, and packaging mechanics remain hidden.
- Independent evolution, testing, failure, and replacement: each focused plan
  can verify and fail its Module locally, while program claims add only the
  cross-process, user-workflow, or release path that local evidence cannot
  prove.
- Necessary complexity and containment: four plans match four independently
  owned audit domains; no program-level runtime Module, schema, test runner, or
  release registry is added.
- Deletion and cumulative machinery result: deleting this coordination plan
  would scatter ordering, shared-write ownership, and objective acceptance
  across four plans; deleting a focused plan would redistribute its detailed
  contract and evidence knowledge into this plan, so both levels earn their
  distinct Interfaces without duplicating implementation.

## Milestones

Each milestone delegates source changes to the exact write sets and gates in
the linked focused plans. This plan and its ledger/issues are the only
additional program-level write set.

### Milestone 1: Contain Critical RPC Disclosure

**Goal:** Prevent credentials and internal locators from reaching backend or
public diagnostics before broad contract refactoring.

**Allowed write set:** Rust plan Milestone 1's exact write set, plus this plan,
ledger, and issues.

**Tasks:**

- [x] Explicitly start the Rust focused plan and execute only its Milestone 1.
- [x] Require real debug-process sentinel evidence and safe typed public errors.
- [x] Update program acceptance state without starting later Rust work
  implicitly.

**Acceptance gate:** Rust RUST-A1; PRG-A1 remains pending until exposure
evidence also closes.

**Status:** `Accepted`

### Milestone 2: Establish Governance and Support Authority

**Goal:** Remove invalid gate authority and decide actual release/host support
before adding permanent evidence or packaging machinery.

**Allowed write set:** Governance Milestones 1–3 and platform Milestone 0 exact
write sets, plus this plan, ledger, and issues.

**Tasks:**

- [x] Complete governance count/error gate dispositions.
- [x] Complete governance's retained-gate evidence inventory.
- [x] Complete the bounded release/host contract investigation.
- [x] Obtain product decisions for LAN support and plugin startup before their
  dependent Rust milestones.
- [x] Serialize shared package/CI/documentation and canonical matrix changes
  and record handoffs.

**Acceptance gate:** Governance GOV-A1 through GOV-A4 and accepted platform
Milestone 0 matrices; unresolved tuples are typed and block/remove only their
own claims.

**Status:** `Accepted`

### Milestone 3: Deepen the RPC Contract Path

**Goal:** Carry one producer-owned request/response/error/event contract through
Rust, Electron, preload, and renderer without default invention.

**Allowed write set:** Rust Milestone 2, then platform Milestone 1, then
frontend Milestone 4 exact write sets, plus this plan, ledger, and issues.

**Tasks:**

- [ ] Accept the Rust DTO/error/exposure contract before generating consumers.
- [ ] Generate and enforce Electron projections and observable invalid outcomes.
- [ ] Migrate renderer consumers and cached model projection to decoded results.
- [ ] Before frontend Milestone 4 source edits, disposition the link-health and
  model-import picker consumers into its shared authority or an exact focused
  frontend slice with behavior evidence.
- [ ] Run the real cross-process and immediate/degraded model-list paths.

**Acceptance gate:** PRG-A1, PRG-A2, and the cache portion of PRG-A5.

**Status:** `Active`

### Milestone 4: Restore Durable and Async Authority

**Goal:** Make storage/root recovery and each runtime's accepted work lifecycle
atomic, current, bounded, and observable.

**Allowed write set:** Rust Milestones 3–4, platform Milestones 2–4, and
frontend Milestone 0 exact write sets, plus this plan, ledger, and issues.

**Tasks:**

- [ ] Accept SQLite mutation/event and migration recovery.
- [ ] Accept launcher-root atomicity and explicit corrupt/unavailable outcomes.
- [x] Re-accept frontend installation-progress admission, supersession,
  cancellation, and terminal outcomes after PRG-I12.
- [ ] Accept Rust, Electron, Torch, and launcher
  admission/cancellation/shutdown outcomes in their required environments.
- [ ] Run combined cold-reopen and runtime lifecycle paths where ownership
  crosses focused plans.

**Acceptance gate:** PRG-A3 and PRG-A4.

**Status:** `Active`

### Milestone 5: Accept Renderer Interaction and Variants

**Goal:** Prove truthful cached state and the audited keyboard, focus, status,
motion, and default/library-only behavior in a representative renderer.

**Allowed write set:** Frontend Milestones 1–3 and 5–6 exact write sets, plus
this plan, ledger, and issues.

**Tasks:**

- [x] Admit a representative renderer harness only if it supplies deciding
  value beyond current tests/smoke.
- [x] Migrate audited modal/popup consumers through deep interaction Modules.
- [x] Implement and prove progress/status and reduced-motion semantics.
- [ ] Exercise both renderer modes through real entry points.

**Acceptance gate:** PRG-A5 and frontend FE-A1 through FE-A7.

**Status:** `Active`

### Milestone 6: Accept Configurations, Bindings, and Release Artifacts

**Goal:** Make supported Rust/build variants, binding cohorts, and final release
bytes agree with accepted consumer and evidence matrices.

**Allowed write set:** Rust Milestones 5–7 and platform Milestones 5–6 exact
write sets, plus this plan, ledger, and issues.

**Tasks:**

- [ ] Make Rust feature/dependency/public-interface configurations real.
- [ ] Remove binding framework leakage and unsupported host claims.
- [ ] Provision pinned generators and test exact host/native cohorts.
- [ ] Assemble exact target artifacts with final-byte version, dependency,
  SBOM/provenance, checksum, license/notice, and extracted-content proof.

**Acceptance gate:** PRG-A6 plus all required-real target/host results.

**Status:** `Planned`

### Milestone 7: Program Acceptance

**Goal:** Reconcile all focused outcomes and run only the objective-level paths
not already proved by adequate focused evidence.

**Allowed write set:** This plan, ledger, issues, focused plan lifecycle/evidence
links, and their already-declared final documentation/report write sets.

**Tasks:**

- [ ] Verify all non-deferred focused milestones are Accepted or Superseded.
- [ ] Re-run cross-plan path claims in their declared environments.
- [ ] Record blocked required-real evidence without substituting weaker checks.
- [ ] Reconcile current architecture, development, security, release, binding,
  frontend, Rust, Electron, Torch, launcher, and plugin documentation.
- [ ] Close or explicitly disposition every issue and acceptance row.

**Acceptance gate:** PRG-A1 through PRG-A7 are satisfied with linked evidence.

**Status:** `Planned`

## Blockers

- PRG-I18/PRG-I19: C1 and C2 are internally verified, while C3–C4 must still connect
  destination authority, relocation, lifecycle persistence, and actual importer
  mutations through production paths. Historical A/B acceptance remains a
  regression obligation; it does not accept the expanded C implementation.
  The focused Rust owner retains the exact write set and detailed evidence.
  Complete M2H/M2I/M2J acceptance remains pending; selected downstream adoption
  is accepted at `2b081fba`, not held by the former repo/path consumer mismatch.
  Core/UniFFI ambient recovery remains a separate transitional removal surface.
- PRG-I21: current evidence has not satisfied both immediate model-list reveal
  and a truthful first framebuffer. The causal marker path adds roughly one
  second; terminal-bootstrap and hidden-capture alternatives did not prove the
  required result. The downstream re-plan above owns the narrower
  representative startup claim and workload. The user accepted the existing
  roughly one-second barrier on 2026-09-05; no unproved reveal replaces it.
  The no-bridge and immediate/delayed/never-terminal composed checks remain
  required for complete startup acceptance, not a hold on the accepted checkpoint.
- PRG-I17 blocks managed Torch deployment, DRBT-A5 required-real runtime work,
  and any installed/ready product claim. Fake-backed request-decoder narrowing
  may continue independently, but no Rust/plugin/manifest/frontend deployment
  write set is admitted until the current producer boundary is stable and the
  program serializes the selected removal/disabled or repaired disposition.
- Platform Milestone 1 generation and frontend Milestone 4 consumption have an
  accepted selected-contract checkpoint; complete milestones remain pending.
  FE-I10/FE-I11/FE-I14 still require their own remaining contract/consumer proof.
- Platform DRBT-A6 remains unavailable for Windows x64 and macOS arm64 until
  accepted required-real runners execute the launcher suite. No local hosts are
  available; independent Linux implementation continues. GitHub Actions results
  must be observed, not presumed to fail or treated as runtime acceptance merely
  because a target compiles.
- Later target, binding, release, persistence, and lifecycle claims remain
  gated by their focused milestone evidence; compile/static substitutes cannot
  close required-real rows.
- Any required legal interpretation remains blocked until its designated owner
  accepts the evidence.

## Re-Plan Triggers

- A focused plan changes its canonical owner, objective, shared write set, or
  acceptance environment.
- The first disclosure repair cannot establish the error/redaction Interface
  needed by the broader RPC contract.
- A product decision retains a capability without an authorization, lifecycle,
  dependency, support, or evidence owner.
- Another consumer expands a systemic population or contradicts the current
  producer/consumer handoff.
- Required-real infrastructure is unavailable for an accepted support claim.
- A focused replacement introduces pass-through Modules, hypothetical Seams,
  duplicate semantic registries, or cumulative machinery beyond its admitted
  composed design.
- A lower-fidelity result was being used to close a higher-fidelity program
  claim.

## Final Acceptance

- Acceptance status: `blocked`
- Deferred follow-ups: `none`; any explicit deferment added during execution
  requires an owner, reason, consequence, and revisit trigger and cannot satisfy
  an affected acceptance claim.
- Final status: `Active`
