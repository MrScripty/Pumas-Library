# Future operating-owner recovery: restricted cooperating custody

This is a design prerequisite, not implemented recovery. The implemented first
slice recovers only an explicitly checkpointed **unstarted** reservation.
The source basis is immutable `a67859e912f2456cac3ad004665efa6ba1ba7625`, tree
`86907ec2055e2c9770bee03cce791d149a166284`. Ready/legacy/unknown rows still refuse.

## Actual effects and current custody

| Cooperating component | Actual lifetime and remaining gap |
| --- | --- |
| `api/builder.rs`, `api/runtime_tasks.rs` | Constructor directory/index initialization, orphan/download/intent reconciliation and watcher effects retain `StoreLifetime` in their actual in-process closures. Aborting a requester does not end blocking work. Historical crash consistency of every admitted storage leaf still needs qualification. |
| `model_library/library.rs`, model index/link stores, `acquisition/store.rs` | Escaped writable library/index/link/acquisition handles retain the native lifetime after API shutdown. A recoverable profile must account for them or withhold those writable escapes; shutdown success alone does not revoke them. |
| `acquisition/service.rs`, `task_custody` | Generic consumer/work/cleanup closures can create children or independent supervisors. Their return or receipt cannot certify those descendants. A restricted profile must deny these callbacks, or explicitly transfer every effect into qualified custody before admission. |
| `api/state.rs` | The builder exposes full primary IPC dispatch. `launch_runtime_profile`, `start_conversion`, `is_conversion_environment_ready` and `ensure_conversion_environment` admit managed runtimes/Python/setup effects. A Rust facade and disabled HF/process/probe flags alone do not close these transport routes. |
| `platform/managed_child.rs`, runtime-profile sessions and conversion manager | Unix `ManagedChild` starts a private process group, retains cleanup leases and observes/drains owned groups. Custody slots and cleanup leases are parent memory. SIGKILL bypasses Drop, so exec'd descendants can outlive the owner and its native descriptor. Failed drain retains unresolved custody. |
| `process/manager.rs`, `api/instance_shutdown.rs` | Legacy process management owns child/reaper records but is not explicitly in the current instance shutdown drain list. Pattern/PID-based orphan discovery is not exact historical ownership proof and must not authorize recovery or unrelated termination. |
| `discovery/start.rs::LocalStartupCustody`, external HTTP owners | Initializer effects can transfer to an independent supervisor; success records settlement/transfer, not that supervisor's cessation. HTTP/initializer process lifetimes need the same enforced scope as core IPC. |

## Smallest next operating profile

A restricted **in-process-only catalog owner** is smaller than general runtime
recovery. Its public capability surface would allow audited local catalog/query
operations and their required crash-consistent in-process storage effects. It
would expose no raw `PumasApi`, writable escape, arbitrary callback, runtime launch,
conversion/setup/probe, launcher update, acquisition or external-supervisor path.
Every admitted finite writer must retain the existing native lifetime through its
actual completion, including SQLite close/checkpoint and queued blocking work.

This is an enforced profile, not a caller boolean. Install the restriction before
constructor effects or the first IPC listener; apply the identical allowlist to
IPC, HTTP, CLI and Rust entry points. Persist versioned scope, physical identity,
exact generation and compatibility in the existing registry before the first
qualified effect. Any unsupported admission must be denied or must durably remove
recoverability **before** that effect. Unknown predecessors remain refused.
Audit all transitive constructor and read-side projection/migration helpers; the
current full builder/dispatch cannot be declared safe by wrapping its return value.

Only after this closed cooperating profile is proved can native lease acquisition
evidence cessation of its participating in-process holders after owner loss.
Require SQLite transaction recovery and atomic/fsynced publication contracts for
each allowed file leaf; the lock does not supply file-level crash consistency.
`metadata/atomic.rs` already offers a held-target, parent-directory-synchronized
publication result, including explicit ambiguous durability. Its legacy
`atomic_write_json` convenience path is a separate file-sync/rename contract;
do not treat both paths as equivalent or infer power-loss proof from process
termination tests. The profile must select and qualify the needed semantics.
Then exact-generation FULL redemption may return the existing owner authority.
Do not infer cessation for ordinary broad owners from PID death or free locks.

## If supported child effects are later admitted

A restricted custodian could retain the **same** native lifetime and be the
exclusive launcher/reaper for those exact admitted children. It must survive
application-owner loss, close admission on owner-channel EOF, drain its actual
owned groups through existing `ManagedChild` semantics, and persist a complete
terminal exact-generation receipt before releasing the lifetime. Recovery still
requires that lease and consumes the receipt atomically. Custodian death, failed
observation, escaped/untracked descendants or unknown transfer leaves refusal.
This extends existing custody; it does not introduce a second owner registry,
TTL, PID-based takeover or authority to kill unrelated processes.

Process groups alone are insufficient containment: group signals target a group,
and a permitted descendant may create a new session/group. A supported child
contract must prevent such escape or keep it explicitly unqualified. See the
Linux [kill](https://man7.org/linux/man-pages/man2/kill.2.html) and
[setsid](https://man7.org/linux/man-pages/man2/setsid.2.html) manuals.

## Available guarantees and required native proof

Linux `flock` follows the open file description and ends when its last descriptor
closes; independent opens contend, including in one process. It is advisory
exclusion for participating holders, not a detector of child or arbitrary writer
cessation. Forked descriptors can extend exclusion; exec handling must be audited.
See [flock](https://man7.org/linux/man-pages/man2/flock.2.html).

SQLite permits one write transaction, and BEGIN IMMEDIATE acquires write admission
before observation/replacement. WAL/FULL syncs committed WAL transactions; it
does not identify filesystem/process ownership or repair arbitrary nontransactional
payload writes. Storage must honor those syncs. See SQLite's
[transactions](https://www.sqlite.org/lang_transaction.html),
[WAL](https://www.sqlite.org/wal.html) and
[synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous) documentation.

Before enabling any operating profile, native acceptance must cover committed
allowed payload hashes across owner termination/cold reopen, simultaneous
recoverers, cancellation at every admitted writer boundary, escaped handles,
unsupported IPC/HTTP admission before effects, uncommitted/corrupt state,
root/registry/boot mismatch and stale generations. Child-capable scope additionally
needs actual owner/custodian loss, still-live and failed-drain descendants, and
exact owned termination evidence. No guarantee about arbitrary unrelated
processes is required; they receive no supported custody or recovery authority.
