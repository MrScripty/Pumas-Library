# M3 authoritative declaration store: proposed design

Status: **implementation admitted for M3a; acceptance pending**, 2026-09-12. Read-only source inspection; no schema or implementation changes and no test execution. This document proposes an M3a store prerequisite followed by M3b public ensure/reconciliation/deletion integration. Neither slice is accepted by this report.

## Existing authority and hazards

Use the existing `ModelIndex` connection in `shared-resources/models/models.db`; do not create a second authoritative database or derive declarations from files/download rows.

- `index/model_index.rs:257–283`: opening configures the connection, runs schema helpers, then initializes FTS. There is no global schema-version gate today.
- `:312–331`: writers use WAL and `synchronous=NORMAL`; readers use query-only connections. `Arc<Mutex<Connection>>` serializes clones of one index but does not serialize a different connection/process.
- `:334–376` and `model_index/governance.rs`: initialization is multiple DDL/seeding operations, including unrelated schema migrations. It is not one authoritative intent migration transaction.
- `model_library/library.rs:224–235`: an index-open failure already propagates; do not replace this with delete/recreate recovery.
- `library.rs:1006,1126`: startup rebuild removes missing projected model rows. `:1295–1318` deep rebuild calls `ModelIndex::clear`; `model_index.rs:1071` clears models/FTS. Declarations must survive both.
- `library.rs:2092–2126`: administrative deletion removes the index row before asynchronous link/file deletion. A transaction that merely checks retention before deleting the row is insufficient: a new ensure could commit during the later filesystem deletion.

## M3a write scope and boundary

Add a private `index/model_index/intent_declarations.rs` module with only necessary crate-private visibility from `model_index.rs`. Integrate schema guards and durability settings in `model_index.rs`. Tests belong in the private module and exercise real on-disk connections. Existing index/catalog behavior must remain unchanged in M3a except refusing incompatible/corrupt intent schemas and strengthening writer commit durability.

M3a exposes no public `ensure_model`, does not start reconciliation, and does not wire a partial retention guard into existing deletion paths. Its methods establish the atomic store operations required by M3b. Keep M3a records crate-private until domain input/output semantics are integrated.

## Schema and compatibility version

Use a component schema version, initially `1`, separate from model/package-facts contract versions:

```sql
CREATE TABLE intent_schema_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0)
);
CREATE TABLE intent_declarations (
    declaration_id TEXT PRIMARY KEY,
    consumer_key TEXT NOT NULL,
    original_requirement_json TEXT NOT NULL CHECK (json_valid(original_requirement_json)),
    generation TEXT NOT NULL,
    model_id TEXT,
    bound_target_json TEXT CHECK (bound_target_json IS NULL OR json_valid(bound_target_json)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_intent_declarations_model ON intent_declarations(model_id);
CREATE INDEX idx_intent_declarations_consumer ON intent_declarations(consumer_key);
CREATE TABLE intent_deletion_claims (
    model_id TEXT PRIMARY KEY,
    claim_token TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);
```

There is deliberately **no foreign key to `models`**, and no cascade from model records, package facts, or download state. A declaration is authoritative even while its artifact/index row is absent. `model_id` identifies the managed target for retention; it is not proof that the artifact exists. Validate row bounds and canonical values in Rust on every read/write, in addition to SQL constraints. Do not persist transient download progress or a cached Available assertion here.

A marker absent with all three tables absent means legacy version 0. Any partial combination, missing/wrong singleton, unsupported version, wrong required columns/keys/indexes, malformed requirement, or noncanonical derived key is a startup error. Do not silently repair authoritative schemas with independent `CREATE TABLE IF NOT EXISTS` statements. Existing declarations with unsupported contract data must remain on disk and cause a bounded failure, not be skipped during enumeration.

Read-only opens check the same supported-version/data boundary without creating tables. Legacy version 0 is readable as having no declarations. New writer opens reject future intent versions before unrelated schema initialization; perform this inspection immediately after opening the connection and before write-oriented PRAGMAs/DDL where feasible.

## Upgrade and durability protocol

1. Open the connection; inspect the component marker/table combination and refuse malformed or future state.
2. Set WAL, busy timeout, foreign keys, and **`synchronous=FULL` before any acknowledged declaration/claim transaction**. Use FULL on the writer connection rather than temporarily toggling it around individual commits.
3. Perform the additive 0-to-1 intent schema installation, all indexes, and marker insertion in one `BEGIN IMMEDIATE` transaction. On a legacy database initialize existing ordinary schema first if necessary, but do not intermingle its unrelated migration batches inside the new intent migration. Validate the new intent schema before committing.
4. On any statement/validation failure roll back the transaction. Preserve the pre-existing logical schema/rows; never remove or reconstruct `models.db`, WAL, SHM, or declaration tables.
5. On ambiguous commit/I/O error return failure and reopen/read authoritative state before retry. Do not treat an error as proof that commit was absent and do not roll back by overwriting files. The idempotency keys below make readback/retry safe.

SQLite transaction rollback is the primary preservation mechanism; M3a performs no destructive migration. No automatic side backup/replacement algorithm is proposed. If a separate pre-upgrade backup is added, it must be a consistent SQLite snapshot including WAL state, not a copy of the main `.db` file. Future destructive migrations require a separately reviewed backup/recovery protocol.

FULL strengthens the intended acknowledged-commit durability boundary on supported SQLite/filesystem combinations. Process-crash tests prove process restart behavior; they do not prove arbitrary device/controller power-loss behavior.

## Exact M3a key and payload contract

`consumer_key` is a caller-supplied stable, case-sensitive identifier, 1–128 ASCII bytes, restricted to letters, digits, `.`, `_`, `:`, and `-`, starting with a letter or digit. Reject surrounding whitespace, empty keys, controls, and path separators rather than silently aliasing callers.

`declaration_id` is lowercase SHA-256 of the bytes `pumas-intent-declaration-v1\0`, followed by the big-endian u64 length and UTF-8 bytes of consumer_key, followed by the big-endian u64 length and UTF-8 bytes of the canonical original requirement. It is deterministic, not a random task ID. This makes equal requirements from the same consumer idempotent and distinguishes consumers and meaningfully different requirements. On a key hit compare the canonical stored payload/consumer too; a conflicting collision is an error, never an overwrite.

Normalize a validated `ModelRequirement` in one private helper before fixed-field-order serialization. Preserve the original upstream selector (including a moving branch/default) in that original value; its immutable binding is stored separately. Reuse the existing M1 quantization normalization through a crate-private helper re-export; M1 matching semantics remain unchanged. Canonicalize known equivalences consistently with the intent domain: absent optional fields use one representation, commit hex is lowercase, and domain-normalized quantization labels are consistent. Reject unsupported/invalid semantics; preserve selected-artifact-path constraints exactly under existing M1 validation, including absolute observed paths. Such a constraint is never a managed replay destination; model_id remains the relative managed identity. Do not accept unchecked caller-supplied declaration IDs or arbitrary JSON.

`generation` is a freshly generated UUID on insertion/re-ensure only and remains unchanged on first binding. Binding is immutable-once, so retaining the creation generation permits idempotent bind retries while its compare-and-swap still rejects stale work. An exact duplicate operation returns the existing generation unchanged. Release followed by re-ensure receives a new UUID even though declaration_id is identical; stale work from the removed declaration cannot bind or release the new row. Numeric generation counters resetting to 1 would create an ABA race.

Private Rust values, agreed with the parent for Sol implementation:

- `IntentDeclarationRecord`: declaration ID, consumer key, normalized original requirement, UUID generation, optional typed bound target, timestamps. Its model_id projection must equal the bound target's model reference ID; unbound targets have null model_id.
- `BoundTarget::Local { model_ref: PumasModelRef }`.
- `BoundTarget::Upstream { model_ref: PumasModelRef, repository_id: String, commit: String, filename: String, format: PackageArtifactKind }`.

`bound_target_json` is only serde encoding of that private typed enum, with explicit discriminants and unknown-field rejection. No arbitrary JSON recipe and no duplicated mutable `DownloadRequest` structure is stored. Upstream replay is limited to the current single-file formats supported by M2 (GGUF, bare Safetensors, ONNX); later package/bundle support needs its own schema/contract decision.

Validate a bound target against the original requirement before writes and again on reads. For Upstream require a validated repository, lowercase full 40-character commit, one portable relative filename, supported matching format, and an existing valid model reference containing that same revision and a selected artifact ID. Reconstruct the minimal existing `DownloadRequest` exactly as current M2 does: repository owner/name for family/name, model_type `llm` for GGUF, exact filename, no request quant or file group. Recompute `SelectedArtifactIdentity::from_download_request_at_revision` with that filename and require exact selected-artifact equality. Preserve original format/quantization constraints in original_requirement_json; binding is not a claim that uninspected bytes have a verified quantization. Explicit original repository/immutable revision/artifact constraints cannot be changed by binding. Symbolic/default original selectors may bind to a validated resolved immutable commit once.

The local model ID cannot currently be purely recomputed without upstream metadata-derived destination family/model type. M3b must supply a validated managed target reference from its planning owner before binding; M3a validates its identity shape/coherence but cannot claim it proves the exact future destination. This is an explicit M3b integration gate, not permission to accept arbitrary filesystem destinations.

A local-only declaration receives `BoundTarget::Local` directly from its original local reference at commit and immediately retains that model ID, even if the artifact is absent or the optional selected-artifact ID is unspecified. Its purpose is model retention; it does not imply availability. Do not silently upgrade a local-only target into upstream acquisition. M3b separately proves which original policies/selectors support public ensures.

M3a accepts only `LocalModel` with `LocalOnly`, and `UpstreamRepository` with
`AllowUpstream`. Other combinations fail validation before mutation. In
particular, repository-based local-only retention needs a verified local
provenance binding that this private codec does not yet express; M3b must settle
that contract before exposing public ensure coverage for it. A binding never
implicitly grants upstream permission.

An unresolved upstream declaration has null target/model_id and no physical retention claim yet. It is durable intent to resolve, not an admitted acquisition. A deletion claim on an unknown eventual target therefore need not reject the unresolved commit; the atomic later bind must reject it before any acquisition. Only bound targets count toward retention. M3b must determine whether public ensure acknowledges unbound resolution or only pinned acceptance; M3a exposes no public acknowledgement contract.

Strictly decode the typed target and validate all null combinations, identity consistency, canonical keys, and UUIDs during inventory. Unsupported replay versions or malformed durable rows fail closed; never reconstruct a missing commit as main.

## Exact crate-private store operations

Every store operation revalidates the component marker and authoritative row contract; only `ModelIndex::new` may install the schema. Every mutation locks the existing connection and uses `BEGIN IMMEDIATE`; never hold a SQLite transaction or the connection mutex across `.await`, HTTP, callbacks, or filesystem deletion. SQL transactions serialize retention decisions across cooperating connections.

- `commit_intent_declaration(consumer_key, original_requirement) -> IntentDeclarationRecord`: validate/canonicalize/derive ID before SQL. Return an exact existing row unchanged; never resolve its moving selector again. Otherwise insert a fresh UUID generation and original payload. For an original local selector derive BoundTarget::Local and model_id, atomically rejecting an existing deletion claim for it. The upstream-unbound insert performs no download or target mutation.
- `bind_intent_declaration(declaration_id, consumer_key, expected_generation, bound_target) -> IntentDeclarationRecord`: validate input before SQL; inside the transaction require a live row belonging to the consumer with the exact UUID generation. Reject a deletion claim on the bound model ID. If already bound to identical data return unchanged; reject an attempted repin rather than silently replacing it. Otherwise bind the typed target and model_id atomically, retaining the declaration creation generation. Missing/deleted/stale rows are conflicts, never recreated by a stale worker.
- `release_intent_declaration(declaration_id, consumer_key, expected_generation) -> ReleaseResult`: exact consumer/declaration scope. Missing is idempotently AlreadyAbsent; a different current generation conflicts. Delete and commit before acknowledging. No file deletion, cancellation, runtime operation, or cascading release.
- `get_intent_declaration(...)`, `list_intent_declarations(...)`, `list_intent_declarations_for_model(model_id)`: strict decoding and stable ordering; no per-row error suppression. Bounded public pagination may be added with M3b projection.
- `claim_intent_model_deletion(model_id, claim_token) -> ClaimResult`: validate opaque UUID token. In one transaction reject if any declaration retains model_id, otherwise insert the claim. Repeating the same model/token is idempotent; a different existing token conflicts. A claim for a model absent from `models` is still valid and blocks future commit/bind.
- `release_intent_model_deletion(model_id, claim_token) -> bool`: delete only the exact model/token. A mismatched token cannot clear another task's claim. Missing is idempotent. M3b may invoke successful removal only after owned filesystem settlement.
- `list_intent_model_deletion_claims()`: strict restart inventory. No TTL, automatic expiry, PID liveness heuristic, or unconditional startup clearing.

M3a may use narrowly scoped, documented `allow(dead_code)` for these staged private methods pending M3b composition. Do not create public runtime operations merely to suppress warnings. A release removes future desired maintenance; M3b rechecks declaration existence/generation before admission, while admitted shared work remains owned to settlement.

## M3b deletion and downgrade integration obligations

Before exposing ensures, every destructive model path must acquire a durable deletion claim, and every declaration commit must reject that claim in the same database authority. Parent owns the exact deletion/pruning inventory. A process-local mutex alone is insufficient because cancellation/crash can occur after index removal and before file deletion.

Retain a claim through the entire owned deletion task. Cancellation of a caller must not clear it while effects continue. A crash leaves a visible blocked claim requiring validated owner settlement/recovery; never silently expire it. Release the claim only after deletion effects and associated index/link updates are observed. Ensure-versus-delete races must end either in a committed declaration and rejected deletion, or in a claimed deletion and rejected/blocked ensure.

An additive SQL `BEFORE DELETE ON models` retention trigger can provide defense for older code paths because current administrative deletion removes the index row first. It is **not a complete downgrade fence** and is not proposed as M3a integration. Before adding it in M3b, decide how projection rebuild/clear handles retained rows: preserve their projected rows conservatively or introduce an explicit transaction-scoped projection path. Do not let a trigger unexpectedly turn a missing-artifact scan into loss of declaration inventory or a partially cleared FTS table. Never rely on a model-row trigger alone when the row may already be absent.

Existing M2 binaries do not read a future component marker or `PRAGMA user_version`. Therefore M3a's version check protects newer schema-aware binaries, **not old binaries**. Downgrading to those binaries with live declarations/claims is unsupported; do not claim startup refusal. They may ignore retention, although non-cascading declaration tables survive ordinary old index clears. A complete old-binary refusal fence would require a separate compatibility mechanism/change, not a marker that old code never reads. M3 acceptance must state this limitation explicitly and prove whatever partial SQL protection is actually installed.

## M3a acceptance evidence required

1. Fresh and populated legacy on-disk databases: transactional install, strict reopen, existing model/governance/facts rows preserved.
2. Inject failure between table/index creation and marker commit; reopen proves no partial authoritative schema or lost old rows. Reject future/partial/corrupt versions without schema replacement.
3. FULL configuration verified on actual writer connection; committed declarations survive forced process exit and reopen. Kill before commit yields either no declaration or a complete committed one, never partial acknowledgement.
4. Exact duplicate commit, first-bind CAS, conflicting repin, release/re-ensure UUID ABA rejection, two-consumer retention, scoped/idempotent release, and malformed key/requirement/recipe rejection without writes.
5. Separate connections race claim versus declaration commit: exactly one wins; wrong token cannot release a claim; reopened claims block ensures.
6. `clear`, startup rebuild/stale model removal, and FTS rebuild leave declaration/claim rows intact. No foreign-key cascade to models.
7. Read-only legacy/current/future/corrupt opens perform no migration. Old-reader downgrade behavior described from actual code/tests, not inferred from the presence of a version field.

M3b then supplies acquisition replay, serialized deletion integration, owned reconciliation, startup resumption, and public ensure/release semantics. Passing M3a does not satisfy full M3 acceptance.
