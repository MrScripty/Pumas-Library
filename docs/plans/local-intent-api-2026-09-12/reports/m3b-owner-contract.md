# M3b existing runtime owner extension

Status: admitted design record. Historical source observations below preceded implementation; current behavior and verification are recorded in [the M3b implementation report](m3b-local-desired-state.md).

## Source basis

`api/runtime_tasks.rs` currently stores a vector of `JoinHandle<()>`. `spawn` launches before taking the registry lock; `shutdown` aborts handles and has no closed flag or asynchronous observation. `PumasApi::drop` (`lib.rs:289`) calls it. This is suitable for best-effort background cancellation, not acknowledged declaration or destructive filesystem effect settlement. `api/reconciliation.rs::start_owned_reconciliation` keeps the single-flight token in its outer task, but that task is currently abortable. `api/hf.rs::shutdown_downloads` already closes and drains its independent existing download owner; it does not cover local-only declarations and should retain its documented download-specific behavior.

The [M3 deletion inventory](m3-deletion-inventory.md) identifies additional destructive paths. M3b must admit `model_library/merge.rs` to the write set before public ensure is exposed. The durable deletion claim from M3a remains the crash authority; runtime ownership complements it and cannot replace it.

## Proposed Rust interface

All types below are crate-private; the public intent facade is handled separately.

```rust
impl RuntimeTasks {
    // Existing interface for best-effort, potentially long-lived background work.
    pub(crate) fn spawn<F>(&self, task: F)
    where F: Future<Output = ()> + Send + 'static;

    // Register one finite operation independently of the requesting future.
    pub(crate) async fn run_owned<T, F, Fut>(
        &self, operation: &'static str, function: F,
    ) -> crate::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(RuntimeTaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<T>> + Send + 'static;

    pub(crate) fn close(&self);
    pub(crate) async fn shutdown_owned(&self) -> crate::Result<()>;

    // Existing synchronous best-effort path: close and abort background tasks.
    // Do not abort already admitted finite owned operations.
    pub(crate) fn shutdown(&self);
}

#[derive(Clone)]
pub(crate) struct RuntimeTaskContext { /* opaque operation identity and owner */ }

impl RuntimeTaskContext {
    pub(crate) async fn run_blocking<T, F>(
        &self, operation: &'static str, function: F,
    ) -> crate::Result<T>
    where T: Send + 'static, F: FnOnce() -> T + Send + 'static;
}
```

`run_blocking` deliberately transports `T`; callers can return `Result<T>` as observed domain data and explicitly handle expected conflicts. A panic or failed join is always an owner failure. For database or filesystem mutation errors, the operation must propagate failure and retain its durable claim as appropriate; do not accidentally classify expected retention conflicts as uncertain write outcomes. No typed error must be flattened into a fake successful domain outcome.

If reconciliation needs synchronous registration before dropping its requester receiver, factor `run_owned` internally through a `start_owned` returning a receiver; no second registry is needed. An immediately dropped, never-polled `run_owned` future admits nothing, as with existing async API calls. Once admitted, dropping its receiver never aborts work.

## Required implementation semantics

Use one shared owner state with a closed flag, abortable background handles, admitted operation entries, and one cached drain completion. Each operation entry has a unique identity, its outer handle, and retained nested blocking joins/results. This is an extension of `RuntimeTasks`, not a new task scheduler. Keep `ReconciliationCoordinator` responsible for single-flight/dirty policy.

- Check closed and register under the same mutex. A start barrier prevents any side effect before registration; close racing registration has exactly two outcomes: rejected with no effects, or admitted and included in drain. Also fix legacy `spawn` ordering so it cannot launch after close; after close it drops the unpolled background future.
- Run the caller closure on an owner-retained task and catch unwind. Send the result through a oneshot, but do not make completion depend on receiver survival. Preserve operation identity until its nested joins are observed. Log unclaimed domain errors; archive panic/join failure in the drain outcome.
- `RuntimeTaskContext::run_blocking` registers each blocking operation before dispatch. Its join remains owned if the requesting outer future panics while awaiting it. Closing root admission does not reject nested work belonging to an already admitted operation: that work may be necessary to commit or settle its transaction/claim. Reject stale context identities once their operation settles.
- Never prune a finished handle without observing its join. A finished outer operation can still have a live blocking effect. Catching the outer panic does not prove effects stopped.
- `shutdown_owned` starts or joins a shared drain task independent of its waiter. It closes root admission, aborts the legacy background population, and waits for every admitted operation and nested effect. Repeated/cancelled shutdown waiters observe the same terminal result. Cache a cloneable internal drain result (for example a failure summary), not the non-cloneable public error itself.
- Synchronous `shutdown`/Drop closes admission and aborts background tasks but leaves finite owned operations registered and running. Runtime/process exit can still interrupt them: durable declarations/deletion claims are the recovery evidence, not a Drop guarantee. Avoid a registry/reference cycle that survives after all entries settle; retained owner references should end with their operation/drain tasks.

No extra queue, retry loop, periodic timer, library instance, HF client, or executor is needed. Captured Tokio `Handle` preserves existing registration from non-runtime threads where the interface supports it.

## Integration and shutdown ordering

Public ensure/release and mutating desired-state reconciliation run within finite owned operations. Pass `RuntimeTaskContext` through their private helpers so SQLite transactions and destructive filesystem work use owned blocking calls. Do not wrap an existing detachable `spawn_blocking` helper and assume the context sees its inner join. Where bounded filesystem work already completes inside a synchronous library helper, run that helper through the context; otherwise pass context through the actual effect boundary.

Migrate `start_owned_reconciliation` to this finite operation population, retaining its `ReconciliationRunToken` until the scope's effects settle. Its `ReconciliationInputs` can carry the context into affected helpers without holding a borrowed API. Keep the watcher dispatcher itself abortable; it schedules finite owned work. New trigger registration after close is rejected. Nested scopes should execute within the admitted context, not attempt a new root registration during shutdown.

For the eventual public local-intent shutdown operation, close declaration/reconciliation admission first, drain those admitted handoffs, then call the existing HF `shutdown_downloads`. This prevents a late declaration admission from creating a download after HF shutdown has been reported. Do not invoke this drain from inside one of the operations it must drain. Existing download-only shutdown remains available and keeps its narrow meaning. Parent chooses the public method name/write set; its docs must not claim unrelated inference/runtime processes are shut down.

Deletion claims are committed before destructive effects and settled only after their owned joins complete. If a failure or process exit prevents observing completion, leave the claim durable and fail closed on ensure/bind. Concurrent release removes the declaration only; it does not clear another operation's claim or cancel a shared acquisition.

## Minimum owner tests

1. Registration racing close never starts an unregistered effect; post-close roots are rejected.
2. Drop an ensure-style requester after a held blocking transaction starts; the effect remains owned, commits once, and shutdown waits for it.
3. Panic the outer operation while its blocking effect is held; drain waits for the effect and reports panic/join failure.
4. Cancel one shutdown waiter, then release the effect; another waiter gets the same settled result, with no second drain.
5. Admitted work can complete necessary nested effects after root close; a stale context cannot create new effects after settlement.
6. Legacy background tasks still abort; finite operations do not. Completed panics are observed rather than silently pruned.
7. Existing reconciliation single-flight stays held until a nested cleanup settles, including requester disappearance and shutdown.

The owner tests prove process-lifetime settlement. M3's real on-disk forced-exit tests must separately prove declaration/recipe/claim persistence and recovery at the SQLite/download admission boundaries. This proposal does not substitute for those gates.
