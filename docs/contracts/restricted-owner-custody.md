# Restricted catalog/query owner and remaining full-owner custody

## Supported operating slice

`PumasApi::builder(root).with_instance_profile(InstanceProfile::CatalogQuery)`
opens an **existing** `shared-resources/models/models.db`. `Full` remains the
unchanged default. This is an existing API/builder/IPC profile, not a second
ownership service or a new public owning facade. It reuses the existing instance
row, native `StoreLifetime`, checkpoint receipt slot and finite-task coordinator.

CatalogQuery permits `list_models`, `get_model`, `search_models`, typed
`catalog_query`, `catalog_checkpoint`, authenticated instance description,
passive local-owner retention, and ordered instance shutdown. Search is literal
case-insensitive substring matching over indexed ID, names, model type and tags;
it does not use forgiving FTS, refresh files, rebuild an index or reconcile.
An empty index is distinct from an unsupported/corrupt index. Existing model
paths and metadata are **indexed observations**, not proof that bytes are present,
ready, fresh, or suitable for inference. No model download/execution is involved.

The explicit profile branch runs before full constructors. It opens SQLite
read-only, admits no schema creation, model-file mutation, watchers, orphan scan,
HF/connectivity probe, acquisition, intent callbacks, process manager,
conversion/setup, runtime-profile or launcher-update effects. `auto_create_dirs`
is refused. Optional builder flags cannot widen this profile. It exposes no raw
writable model library, index, acquisition service or arbitrary custody callback.
Unsupported fallible Rust methods return an error before their effect. Legacy
infallible full-service methods/accessors assert before service access; callers
must check `instance_profile`/capabilities before using them. Their signatures and
Full behavior remain unchanged. HTTP service/initializer registration is refused
before custody/listener handoff; no HTTP capability is advertised. An arbitrary
external HTTP host or writer is outside this profile and cannot acquire recovery
qualification by publishing a receipt.

The same existing IPC server installs the restricted dispatcher before binding.
Only `describe_instance`, `catalog_query` and `catalog_checkpoint` are admitted,
with the exact retained generation's connection token. The existing typed IPC
protocol and `PumasLocalClient` carry the two new catalog methods; a language or
CLI consumer may use the same bounded JSON/framing contract. RPC/HTTP routes and
CLI commands were not extended in this slice. Full-owner routes continue through
their existing dispatcher. Capabilities are exactly `catalog.indexed-query@1`,
`catalog.literal-search@1` and `catalog.same-boot-recovery@1`; ordinary
`model.query@1`/selector/artifact requirements do not silently negotiate here.

## Durable acknowledgment and explicit cold reopen

Linux, the same kernel boot, a stable cooperating root/registry/index namespace,
regular unaliased index/WAL/SHM files, and a filesystem honoring native flock and
SQLite durability are required. Symlinked index subtrees and hardlinked index
files refuse. Namespace/PID identity, free locks and timeouts never authorize
ordinary full/legacy/unknown owners. Hostile raw SQL/file writers, registry
rollback, copied receipts, cross-boot and power-loss guarantees are outside this
qualification.

Before opening the index or exposing IPC, the existing registry commits a bounded
versioned query-only scope under WAL/FULL. A claiming scope is **not recoverable**.
The strict reader validates the supported nine TEXT columns and ID primary key,
required stored JSON types, bounds (100,000 rows, 4 KiB IDs, 1 MiB per other
field, 64 MiB raw total, serialized rows below the existing 16 MiB IPC frame
limit with 32 KiB reserved for query/envelope overhead),
and existing WAL index integrity. It hashes exact stored strings in primary-key
order with unambiguous length framing in one SQLite read transaction. The digest
covers the `models` table, including committed WAL rows, not other index tables
or model files. It never silently defaults JSON, omits bad rows or rebuilds data.

Ready promotion and its complete acknowledgment commit in a single FULL immediate
registry transaction after the existing invalidation trigger. The private scope
binds policy/version, boot, registry identity, physical root and database identity,
exact instance/generation/token and library identity. Public
`CatalogOwnerCheckpoint` is an observation, not ownership authority. Reads check
physical identities and exact durable qualification again; changed/corrupt state
fails closed. Constructors/read workers retain the existing native lease through
actual SQLite work/connection lifetime. Cancellation of a read waiter does not
abandon its finite effect. Shutdown closes admission and drains finite work,
passive holds and IPC before releasing the exact row. The API and actual retained
connections may continue holding physical exclusion after row release.

`discovery::recover_catalog_owner(registry, root, expected).await` is explicit;
normal builder/discovery/bootstrap never invokes it. It acquires the same native
lease, validates the strict WAL-inclusive snapshot and complete durable receipt,
then consumes the exact ready generation and mints a fresh claim/token in a FULL
immediate transaction. It returns only another CatalogQuery `PumasApi`; no generic
start authority escapes that could initialize a full owner. A second recoverer
or stale observation refuses. Failure/cancellation before complete successor
promotion leaves a claiming/unqualified row; it cannot replay the predecessor.
Only an acknowledged qualified operating history is recoverable.

## Remaining full-owner gap

| Components | Unqualified effects/custody |
| --- | --- |
| `api/builder.rs`, `runtime_tasks.rs`, `reconciliation.rs`, `model_library/watcher.rs` | Broad constructor, watcher, orphan/download/intent reconciliation writes and their interrupted storage states. |
| `model_library/library.rs`, `index/model_index.rs`, link/migration/mapper stores | Writable escapes, projection/migration/report/link/copy/removal and read-side regeneration; filesystem/index consistency beyond the restricted models observation. |
| `acquisition/store.rs`, `service.rs`, `task_custody.rs` | Persistent acquisition and arbitrary work/cleanup/transfer callbacks that can create children or independent supervisors. |
| `api/state.rs`, external HTTP/RPC hosts | Full launch/conversion/setup/status routes and external transport owners require a separately sealed allowed scope before admission. |
| `platform/managed_child.rs`, `linux_group.rs`, `runtime_profiles/process_owner.rs`, conversion managers/workers/setup/readiness/process owners | Parent-memory custody is lost on SIGKILL; exec'd or escaped descendants can survive the native parent descriptor. |
| `process/manager.rs`, `api/instance_shutdown.rs` | Legacy children/reapers and explicit drainage; PID/pattern fallback is not exact historical ownership proof. |
| `discovery/start.rs::LocalStartupCustody`, `retention.rs`, external HTTP owners | Transfer acknowledgments/passive disposal do not certify external cessation or cross-crash lease transfer. |
| `system/utils.rs`, `launcher/updater.rs` | Ambient opening, probes, Git/build/install/restart commands and path writes. |

Child-capable recovery would require a separately qualified cooperating custodian
retaining the **same** native lease, exclusively admitting and reaping those exact
children, surviving owner loss, closing admission on EOF, and recording complete
terminal drainage before releasing the lease. Unknown/failed/escaped custody
continues refusing. Process groups alone do not prevent `setsid` escape. This
slice does not authorize killing unrelated processes or recover an inference/
download/full owner merely because its PID died or its lease became free.

## Acceptance evidence limits

Native owned-process tests commit fixture catalog state into WAL, acknowledge it
through a real CatalogQuery owner and IPC, SIGKILL/reap that owner, cold reopen,
and verify the exact digest/payload and fresh generation. Simultaneous owned
recoverers admit one winner; killing that winner cannot replay the old observation.
A killed real Full owner remains refused. Controlled corruption/schema/alias,
queued actual SQLite read cancellation, passive-guard cancellation and abandoned
claim/redemption cases are explicitly fixtures, not power-loss or child-custodian
proof. Test logs/review reports and hashes are retained outside Git.
