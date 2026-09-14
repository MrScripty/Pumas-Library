# M3 lifecycle integration proposal

Status: proposed, not accepted. Parent proposes implementing only M3a, the crate-private authoritative SQLite store prerequisite; public ensure/release and background reconciliation remain M3b and are not authorized by completion of that prerequisite. Read-only source inspection on 2026-09-12; no production edits or Cargo execution. This report addresses lifecycle integration; the parent review owns exact index schema, migration/downgrade behavior, and deletion guard inventory.

## Existing owners and constraints

- `src/api/reconciliation.rs:396–496` already schedules single-flight scoped work through `PrimaryState.runtime_tasks`. `start_owned_reconciliation` retains its run token after requester cancellation and awaits `run_scope`. Extend this path rather than adding a declaration scheduler. `notify_filesystem_changes` (approximately line 587) supplies missing-file events; builder marks all scopes dirty at startup.
- `ReconciliationCoordinator::should_run` is event-driven: after success a scope runs again only when dirty. Its cooldown fields do not currently establish periodic retry. Dirty events during an active run survive, but an in-flight trigger does not itself arrange another run. Consequently a missing event concurrent with a pass needs a bounded follow-up in the same existing runner, or an explicitly retained dirty state for the next trigger.
- `src/api/runtime_tasks.rs:26–42` spawns tasks and aborts them on shutdown; it has no closed-admission flag or asynchronous drain. `PumasApi::drop` in `src/lib.rs:289` calls this abort-only shutdown. Do not claim these are durable effect-drain guarantees. Awaited `spawn_blocking` can outlive an aborted wrapper.
- HF `run_download_invocation`, managed pinned admission, task generations, destination reservations, and `shutdown_downloads` already own acquisition. Reuse them. Startup calls real `restore_persisted_downloads` before constructing primary state; ordinary queue inventory is reconciled under its existing owner before restoration. Unresolved admission/recovery custody remains an error, not absence.
- `intent/acquisition.rs::acquire_upstream` currently combines immutable revision/tree selection with managed admission. Its public pending requirement uses a local model reference. That reference alone cannot repair a missing package: acquisition rejects `LocalModel` selectors. M3 needs an immutable upstream acquisition recipe stored alongside declaration identity.

## Declaration and acquisition ordering

Use a durable declaration identified by consumer identity plus the canonical original requirement, with an opaque declaration ID for release. Preserve case-sensitive repository/path/consumer identities; normalize only semantics already normalized by requirement validation. The index owner must define exact canonical encoding and uniqueness. Different consumers retain independently. A duplicate ensure returns the same declaration and must not replace an already pinned recipe because a branch moved.

Persist both the original requirement and, once known, an immutable acquisition recipe containing repository ID, full commit, canonical artifact selector/request, selected artifact ID, and target local identity. Do not store a local handle, availability claim, or download status as authority. A declaration may remain unresolved when upstream is unavailable; ensure can acknowledge that durable desired state with an unavailable observation.

Recommended sequence:

1. Validate requirement and consumer before mutation. Register an owned declaration operation; under the declaration/index mutation gate, insert-or-read the unique declaration in one durable transaction. Acknowledge retention only after observing successful commit. Requester disappearance does not erase the row.
2. Resolve local availability using the existing read-only resolver. For upstream-capable unresolved declarations, prepare revision/tree/artifact through the existing HF invocation owner. Split the current acquisition helper into private preparation and managed admission, without changing public get behavior.
3. Under the same declaration mutation gate, re-read the declaration and atomically install the immutable recipe only if its generation remains live and unbound. A competing ensure uses the committed winner. Release during upstream preparation prevents installation and subsequent admission.
4. After recipe commit, admit the exact pinned request through `start_hf_download_owned_at_revision`. Keep the declaration gate across the admission decision/handoff so release cannot complete and then be followed by a stale new admission. Never hold a SQLite transaction across network or filesystem admission. Download identity/custody remains exclusively in the existing download store.
5. Persisted recipe plus the live declaration is enough to repeat after a crash before admission. A crash after admission but before response must join/recover the existing durable download; never infer absence from an empty live snapshot or a swallowed store error. Existing strict admission performs the final check.
6. Release deletes only that consumer's declaration transactionally and idempotently. It does not delete files or cancel an admitted shared download. Other consumers retain their declarations. Once release returns, a stale declaration pass must not admit fresh work for the deleted generation.

A locally selected artifact with no verified upstream provenance can be retained against deletion, but external disappearance has no invented repair source. Return Missing/Incomplete as appropriate. `LocalOnly` declarations never resolve upstream, including on startup and watcher events.

## Reconciliation and bounded retry

Add a desired-state phase to the existing scoped run after catalog reconciliation. For `Model(id)`, select declarations bound to that identity; unresolved declarations are considered on all-scope startup or explicit ensure. Deduplicate recipes within one pass and invoke the same acquisition service as get. Do not call broad operational reconciliation from intent query/status; these stay read-only.

Startup should load declarations after database migration and ordinary download restoration, then schedule one all-scope desired pass through the existing coordinator. An unavailable upstream must leave declaration rows intact and permit startup. An unresolved download-authority failure still prevents unsafe admission; preserve the existing restore behavior rather than treating it as successful empty recovery.

Each declaration gets at most one resolution/admission attempt per pass. A pass must not automatically resume a paused, failed-integrity, or custody-blocked download: these states are observed and retained. The existing downloader owns transfer retries (`hf/download.rs::run_download` uses `NetworkConfig::hf_download_max_retries` and `hf_download_max_retry_elapsed`). Do not add an independent retry loop or reset its budget on every watcher event.

Use persisted `next_attempt_at` and consecutive transient-failure count for unresolved upstream preparation if repeated events would otherwise bypass throttling. Proposed bounded backoff is 30 seconds, 2 minutes, then 10 minutes, capped at 10 minutes; eligibility is checked only on existing triggers (startup, explicit ensure, relevant watcher event). This is event-driven retry, not a promise of timed autonomous recovery. Successful preparation clears preparation backoff; invalid requirements/integrity/custody failures do not automatically retry. Parent must settle whether explicit ensure bypasses backoff; recommend no bypass for duplicate ensures. If automatic wall-clock retry is required, explicitly amend the existing coordinator rather than silently adding detached sleeps/tasks.

## Durable deletion claims (required correction to the gate)

`ModelLibrary::delete_model` (`src/model_library/library.rs:2092`) deletes the index row before awaited symlink and directory removal. An in-memory mutex released when the requester disappears cannot exclude late filesystem effects. A declaration commit between index deletion and directory removal would otherwise acknowledge retained bytes that are about to disappear.

M3a must expose a transactional deletion-claim API: acquire a unique claim for the model/target only if there are no retaining declarations; reject ensure/bind while the claim exists; settle only the matching claim token after all destructive effects are observed. Persist the claim before any removal. A crash or uncertain cancellation leaves it blocking, never implicitly clear it because the model row vanished. The claim cannot cascade away when the catalog row is removed. Startup reconciliation of such claims belongs to M3b's owned deletion protocol, not a schema-open repair that guesses effects completed. Guard all destructive paths using the same authority, including cleanup/reclassification destinations found by the parent audit.

The claim is durable authority, while any in-memory gate merely serializes live handoffs. Recipe installation must check both declaration generation and destination deletion claim in the transaction. Claim release requires an exact token; a stale cleanup task cannot release a newer claim. Store-only M3a tests establish these transaction contracts, not safe filesystem deletion.

## Shutdown checkpoint and minimal additional seam

Do not make durable declaration work depend on a cancellable RuntimeTasks wrapper. Preferred prerequisite: extend the existing runtime owner with closed admission and an awaitable drain for newly tracked declaration/reconciliation work, retaining any blocking database effect until its result is observed. This changes an existing owner, not the scheduler or library ownership model. Orderly intent shutdown closes declaration admission/triggers, drains in-flight commit/pin/admission handoffs, then drains HF downloads. Drop remains best effort and must not be described as orderly shutdown.

This requires explicit write-set admission for `api/runtime_tasks.rs` and the selected public shutdown integration (`lib.rs` or `api/hf.rs`), which M3's current list omits. Alternative: use the existing HF invocation owner for every declaration effect, but it must also work when HF is not configured; introducing a hidden HF client solely for local database ownership is not recommended. This is a concrete ownership prerequisite, not evidence that current acknowledged SQLite rows are lost.

## Evidence gates and crash points

Run real child processes with fresh SQLite and download-store owners. Terminate at (a) before declaration commit, (b) after commit before acknowledgement, (c) after acknowledgement before preparation, (d) after immutable recipe commit before admission, (e) after durable download admission before publication, and (f) during release commit. Reopen and assert acknowledged declarations survive, released declarations do not reappear, a recipe never follows a moved branch, unresolved custody blocks duplicate admission, and availability is recomputed from current package facts/files.

Include requester drop during commit and pin preparation, two consumers releasing independently, release racing admission, missing-file watcher while reconciliation is already running, startup with unavailable upstream, and shutdown while database/pin/admission effects are held. Acknowledgement-before-exit needs parent/child synchronization, not sleeps. Test malformed/incompatible stores separately from valid empty stores. Existing M2 fresh-owner resume evidence is useful but does not replace these inter-store crash gates.

Linux process-exit tests prove only the tested SQLite/filesystem configuration. Record journal/synchronous settings and platform behavior from the index review. They do not prove power-loss durability or all-platform recovery. No migration, broader recovery contract, or M3 acceptance is asserted by this proposal.

## Suggested implementation split

- Index owner: declaration schema, canonical key, transactional insert/bind/release, generation check, retention queries, migration/downgrade fixtures.
- Intent owner: public ensure/release result types, validation, prepare/persist/admit composition, immutable recipe conversion, no-write status projection.
- Lifecycle owner: `api/{state,builder,reconciliation}.rs`, the admitted runtime-owner drain seam, startup/watcher triggers, bounded preparation eligibility, shutdown ordering.
- Parent/library owner: all destructive deletion and duplicate cleanup guards sharing the retention transaction/gate; release never delegates to deletion.
- Process-test owner: `tests/intent_desired_state_tests.rs` and necessary test-only crash hooks at the exact commit/admission/publication points.

Before implementation, settle the exact mutation gate shared with deletion, runtime shutdown write-set amendment, retry contract, and immutable recipe serialization. These are decisions required by the plan; no second downloader, library owner, remote identity, or independent scheduler is proposed.

## M3a boundary and follow-up

Repository owner inventory finds no general core task owner beyond `api/runtime_tasks.rs`; HF lifecycle is download-specific and `runtime_profiles/process_owner.rs` owns runtime processes. Do not instantiate another HF client or borrow runtime-process ownership for local declaration effects. Extend the existing runtime task owner in M3b with closed admission, retained completion/result observation, and an awaitable drain. The existing reconciliation coordinator continues to decide scheduling; this owner extension only controls effect lifetime. It must retain blocking transaction/deletion joins through requester cancellation and shutdown, with errors observable by the owner even when the caller is gone.

M3a implements only crate-private schema/store APIs for declarations, immutable recipe CAS, retention queries, and durable deletion claims. It adds no public ensure, background loop, or claim auto-clear. A later M3b must wire the effect owner, claim-aware deletion, startup reconciliation, and public operations before claiming retained-artifact behavior. Forced-process tests for M3a can prove committed store rows/claims survive reopening, but do not satisfy M3's cross-store admission/publication or repair gates.

M3b follow-up: see [the owner contract](m3b-owner-contract.md) and [deletion inventory](m3-deletion-inventory.md). The latter requires an explicit `model_library/merge.rs` write-set amendment before public ensure can rely on all destructive-path guards.
