# Proposal: Fast Model Library Snapshot And Batch Resolution For Pantograph

## Status

Draft updated after architecture review.

This proposal must not be implemented by expanding transparent RPC-backed
`PumasApi` behavior. The intended direction is an explicit split between:

- Pumas Library instances that own a library root and publish local instance
  services;
- external clients such as Pantograph that consume typed model facts and update
  streams;
- transport adapters such as RPC, SSE, Electron, loopback TCP, Unix sockets, or
  named pipes.

The Rust API should not secretly route through RPC. Cross-process attachment is
valid, but it must be an explicit local-client choice.

## Context

Pantograph uses Pumas Library as the canonical model source. The integration
depends on Pumas for model identity, model metadata, package facts, update
cursors, and runtime-relevant model facts while Pantograph remains responsible
for runtime selection, scheduling, workflow behavior, diagnostics, and node
execution.

Current Pantograph UI paths expose a performance problem at the Pumas/Pantograph
boundary:

- Pumas `list_models()` can return the current local library quickly.
- Pantograph's `puma-lib` selector and library page need more than raw model
  records, so they currently enrich each row through separate Pumas calls.
- That enrichment path is too slow for interactive UI and graph authoring.

Measured against the current local Pumas Library repository:

```text
list_models_ms=19 records=53
summary_snapshot_ms=43 items=53
resolve_summaries_ms=4783 ok=53
resolve_descriptors_ms=4438 ok=53
get_settings_ms=4380 ok=53
query_port_options_ms=9152 options=53
```

Pantograph can avoid eager enrichment on the UI path, but Pumas should also
provide a fast canonical API surface so consumers do not need to issue one or
more expensive calls per model.

## Problem

The current public API encourages consumers to compose a model selector from:

- `list_models()`
- `model_package_facts_summary_snapshot()`
- `resolve_model_package_facts_summary(model_id)` for many models
- `resolve_model_execution_descriptor(model_id)` for many models
- `get_inference_settings(model_id)` for many models

That shape creates repeated metadata loads, primary-file scans, package-facts
fingerprinting, dependency checks, and possible IPC round trips. The individual
APIs are useful, but they are not shaped for a fast model-library list, graph
model selector, or startup cache.

The current low-level Rust facade also mixes deployment concerns:

- a caller may create a direct primary-owned API handle;
- a caller may silently receive an IPC-backed client handle;
- the same type hides whether calls are in-process typed Rust calls or
  serialized transport calls.

That ambiguity is not appropriate for Pantograph or other external consumers.
They need to know whether they are embedding a Pumas instance, reading a local
library snapshot, or connecting to a running local Pumas service.

## Goal

Add a Pumas-owned fast snapshot and batch-resolution surface that allows
Pantograph and other consumers to:

- list model-selector rows from materialized indexed state;
- render library and graph selector UIs without deep per-model resolution;
- receive explicit missing, stale, invalid, or unresolved states;
- hydrate full model details lazily after a user selects a model;
- subscribe to model-library updates and recover missed updates from a stable
  cursor;
- keep Pumas as the canonical source of model facts without making consumers
  duplicate Pumas semantics.

## Non-Goals

Pumas should not own Pantograph runtime scheduling, final backend selection,
workflow behavior, node execution, or diagnostics policy.

Pumas should not inspect Pantograph runtime registries, queues, sessions, warm
processes, or scheduler state.

Pumas should not make a UI list call perform deep package-facts regeneration,
dependency installation checks, filesystem scans, or one API call per model.

Pumas should not make the direct Rust API a transparent RPC facade. RPC and
other transports are adapters for cross-process clients, not the internal Rust
execution model.

Pumas should not make Pantograph a Pumas Library instance unless Pantograph
explicitly opts into owning a library root and its background lifecycle.

## Scope

### In Scope

- Public Rust API role split for owning instances, explicit local clients, and
  read-only local snapshot readers.
- Core-owned typed model-library update subscription and durable cursor
  recovery.
- Fast selector snapshot DTOs backed by indexed SQLite/cache state.
- Batch hydration APIs for expensive descriptor, package-facts summary, and
  inference-settings paths.
- Local instance discovery for same-device clients.
- Transport adapter alignment so RPC/SSE/Electron forward core contracts
  instead of defining separate semantics.
- Pantograph-facing integration guidance based on typed Rust contracts and
  explicit local-client attachment.

### Out Of Scope

- Distributed or multi-host instance discovery.
- Pantograph runtime scheduling, queueing, session, or workflow policy.
- Replacing SQLite as the canonical local persistence layer.
- Preserving transparent `PumasApi` auto-client behavior as a compatibility
  promise.
- Making the GUI RPC surface the preferred API for non-GUI consumers.

## Inputs And Current Architecture Evidence

- `pumas-core` currently documents itself as usable without HTTP/RPC, but
  `PumasApi` can contain either primary state or an IPC client.
- `PumasApi::new()`/builder behavior can claim primary ownership or attach to
  an existing owner through the local instance registry and IPC.
- `pumas-core` already has a local registry for library paths and running
  instance rows.
- `pumas-core` already has local IPC concepts for primary/client convergence.
- The GUI already has a pushed model-library update path through backend SSE,
  Electron forwarding, preload validation, and frontend subscriptions.
- Existing selector and package-facts calls demonstrate that deep per-model
  enrichment is too slow for Pantograph UI startup and graph-authoring paths.

## API Boundary And Instance Model

Pumas needs three explicit roles.

### Pumas Library Instance

A Pumas Library instance is a process that owns one launcher/library root. It is
responsible for:

- claiming the library root in the local instance registry;
- writing SQLite and metadata state;
- running migrations, reconciliation, import, download, and recovery work;
- maintaining materialized selector/cache rows;
- producing canonical model-library and runtime-profile update events;
- optionally exposing a local service endpoint for same-device clients.

Opening an instance should be explicit:

```rust
let instance = PumasLibraryInstance::open_primary(root, options).await?;
```

If another live process already owns the root, the constructor should fail with
an ownership error. It should not silently return a transport client.

### Pumas External Client

An external client consumes Pumas facts but does not own the Pumas lifecycle.
Pantograph is in this category by default.

Client access should be explicit:

```rust
let instances = PumasLocalInstanceRegistry::list_instances()?;
let client = PumasLocalClient::connect(instances[0].endpoint()).await?;
let snapshot = client.model_library_selector_snapshot(request).await?;
```

For local read-only use cases that do not need active downloads, reconciliation,
or pushed events, a separate read-only SQLite-backed API can exist:

```rust
let reader = PumasReadOnlyLibrary::open(root)?;
let snapshot = reader.model_library_selector_snapshot(request).await?;
```

The read-only API must not start watchers, recover downloads, mutate metadata,
or claim the instance registry.

### Transport Adapters

RPC, SSE, Electron IPC, loopback TCP, Unix domain sockets, and Windows named
pipes are transport adapters over the instance/client contract. They must not
define the core Rust semantics.

Local instance discovery can still use a local registry row containing the
library root, process id, status, endpoint, and connection token. Same-device
transport should prefer platform IPC where practical:

- Unix domain sockets on Linux/macOS;
- named pipes on Windows;
- loopback TCP only when platform IPC is not available or as a compatibility
  fallback.

Any loopback TCP endpoint must bind to localhost only and use an instance token
or equivalent local authentication from the registry.

## Affected Contracts And Artifacts

### Structured Contracts

- Rust public API entry points currently represented by `PumasApi`.
- Model-library selector snapshot request/response DTOs.
- Batch descriptor, package-facts summary, and inference-settings DTOs.
- Model-library update event, cursor, stale-cursor, and subscription contracts.
- Local instance registry endpoint records.
- RPC/SSE/Electron forwarding payloads derived from core event contracts.

### Persisted Artifacts

- Local registry SQLite database containing library paths and running instance
  rows.
- Model index SQLite database containing materialized selector rows or
  projections.
- Durable model-library update feed rows.
- Existing metadata and package-facts cache rows used to populate selector
  state.

### Compatibility Position

Legacy transparent client-mode behavior does not need to be preserved. If
`PumasApi` remains temporarily, it should be deprecated or converted into an
explicit alias that does not hide transport attachment.

## Proposed API: Fast Selector Snapshot

Add a snapshot API for model-library selectors and library pages:

```rust
impl PumasLibraryInstance {
    pub async fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot>;
}

impl PumasLocalClient {
    pub async fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot>;
}

impl PumasReadOnlyLibrary {
    pub fn model_library_selector_snapshot(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot>;
}
```

The same DTOs can be shared, but the caller must choose whether it is using an
owning instance, an explicit local client, or a read-only local snapshot reader.

Example request and response shape:

```rust
pub struct ModelLibrarySelectorSnapshotRequest {
    pub limit: usize,
    pub offset: usize,
    pub search: Option<String>,
    pub model_type: Option<String>,
    pub task_type_primary: Option<String>,
}

pub struct ModelLibrarySelectorSnapshot {
    pub cursor: String,
    pub total_count: usize,
    pub rows: Vec<ModelLibrarySelectorSnapshotRow>,
}

pub struct ModelLibrarySelectorSnapshotRow {
    pub model_ref: PumasModelRef,
    pub model_id: String,
    pub repo_id: Option<String>,
    pub selected_artifact_id: Option<String>,
    pub selected_artifact_path: Option<String>,
    pub entry_path: Option<String>,
    pub entry_path_state: ModelEntryPathState,
    pub artifact_state: ModelArtifactState,
    pub display_name: String,
    pub model_type: String,
    pub tags: Vec<String>,
    pub indexed_path: String,
    pub task_type_primary: Option<String>,
    pub pipeline_tag: Option<String>,
    pub recommended_backend: Option<String>,
    pub runtime_engine_hints: Vec<String>,
    pub storage_kind: Option<String>,
    pub validation_state: Option<String>,
    pub package_facts_summary_status: ModelPackageFactsSummaryStatus,
    pub package_facts_summary: Option<ResolvedModelPackageFactsSummary>,
    pub detail_state: ModelLibrarySelectorDetailState,
    pub updated_at: String,
}

pub enum ModelLibrarySelectorDetailState {
    Ready,
    MissingSummary,
    StaleSummary,
    InvalidSummary,
    NeedsDetailResolution,
}

pub struct PumasModelRef {
    pub model_id: String,
    pub selected_artifact_id: Option<String>,
    pub model_ref_contract_version: u32,
}

pub enum ModelEntryPathState {
    Ready,
    Missing,
    Ambiguous,
    NeedsDetailResolution,
}

pub enum ModelArtifactState {
    Ready,
    Missing,
    Partial,
    Ambiguous,
    NeedsDetailResolution,
}
```

Exact names can change, but the contract should be a fast materialized read
from indexed/cache state.

`indexed_path` is a display/debug field, not the executable model contract.
Pantograph and other API consumers should use `model_ref` plus the selected
artifact and entry-path state to create graph-facing references. If
`entry_path_state` or `artifact_state` is not `Ready`, the consumer can render
the row but must hydrate details before treating it as executable.

`model_ref_contract_version` is the version of the Pumas model-reference
contract, not a model revision or publisher version. If Pumas later needs to
surface source model revisions, those should be separate explicit fields with a
defined source such as Hugging Face commit SHA or local metadata revision.

## Snapshot Performance Contract

For a normally indexed local library:

- direct in-process `PumasLibraryInstance` and `PumasReadOnlyLibrary`
  snapshots should return in `<= 5ms` for common pages around 50-100 rows on a
  warm local SQLite-backed library;
- explicit `PumasLocalClient` snapshots should be measured separately as one
  local transport request plus the core query, with no per-row transport calls;
- the initial local-client target is `<= 25ms` for common warm same-device
  pages around 50-100 rows, excluding first connection setup, and must be
  re-measured against the selected transport implementation once Unix sockets,
  named pipes, or loopback TCP are chosen;
- The snapshot must be served from indexed SQLite/cache rows only.
- The snapshot must not touch model directories.
- The snapshot must not scan files.
- The snapshot must not load per-model metadata JSON.
- The snapshot must not compute package-facts fingerprints.
- The snapshot must not resolve dependency requirements.
- The snapshot must not regenerate package facts.
- The snapshot must not perform one IPC call per model.
- The direct Rust snapshot path must not serialize or deserialize JSON.
- JSON is acceptable only at transport boundaries.
- Access-mode benchmarks must report direct instance, read-only, and local
  client timings separately.
- Missing, stale, invalid, or unresolved details must be represented as state
  in the returned row instead of being resolved inline.

Cold database open, first process startup, import, migration, and background
reconcile can be measured separately from snapshot query latency.

## Proposed API: Batch Resolution

Add batch equivalents for expensive single-model APIs:

```rust
pub async fn resolve_model_execution_descriptors(
    &self,
    model_ids: Vec<String>,
    options: ModelExecutionDescriptorBatchOptions,
) -> Result<Vec<ModelExecutionDescriptorResult>>;

pub async fn get_inference_settings_batch(
    &self,
    model_ids: Vec<String>,
) -> Result<Vec<ModelInferenceSettingsResult>>;

pub async fn resolve_model_package_facts_summaries(
    &self,
    model_ids: Vec<String>,
    options: ModelPackageFactsSummaryBatchOptions,
) -> Result<Vec<ModelPackageFactsSummaryResult>>;
```

The batch methods should reuse metadata, index rows, cache rows, and filesystem
observations internally. They should not behave as a loop over public
single-model APIs unless that loop still meets the performance target.

## Split Cheap Descriptor From Dependency Resolution

`resolve_model_execution_descriptor(model_id)` currently includes dependency
resolution. That makes the descriptor too heavy for selectors and library rows.

Pumas should split the concerns:

```rust
pub async fn resolve_model_execution_descriptor(
    &self,
    model_id: &str,
    options: ModelExecutionDescriptorOptions,
) -> Result<ModelExecutionDescriptor>;

pub struct ModelExecutionDescriptorOptions {
    pub include_dependency_resolution: bool,
}
```

or provide separate APIs:

```rust
pub async fn resolve_model_execution_descriptor(
    &self,
    model_id: &str,
) -> Result<ModelExecutionDescriptor>;

pub async fn resolve_model_dependency_requirements(
    &self,
    model_id: &str,
    platform_context: PlatformContext,
    backend_key: Option<&str>,
) -> Result<ModelDependencyRequirements>;
```

The cheap descriptor should include model identity, entry path, model type,
task, storage kind, validation state, backend hints, and contract version.
Dependency resolution should be opt-in.

## Precompute And Materialize Selector Facts

Pumas should populate selector and summary cache rows during import, migration,
metadata refresh, and reconciliation. A normal UI selector call should not be
responsible for making cache rows exist.

Acceptance criteria:

- Existing imported models appear in the selector snapshot without consumers
  resolving each model.
- Newly imported, deleted, moved, or metadata-modified models produce update
  events that allow consumers to refresh their cached selector rows.
- Missing, stale, and invalid summaries are explicit row states.
- Summary regeneration can happen in background maintenance or through explicit
  detail-resolution APIs.
- The snapshot includes a cursor suitable for
  `subscribe_model_library_updates_since(cursor)` and
  `list_model_library_updates_since(cursor, limit)` recovery calls.

## Canonical Subscriber Model

Model-library updates should be a core contract, not a GUI-specific feature.

The Pumas Library instance owns a typed update bus:

```rust
impl PumasLibraryInstance {
    pub async fn subscribe_model_library_updates_since(
        &self,
        cursor: Option<ModelLibraryUpdateCursor>,
    ) -> Result<ModelLibraryUpdateSubscription>;

    pub async fn list_model_library_updates_since(
        &self,
        cursor: Option<ModelLibraryUpdateCursor>,
        limit: usize,
    ) -> Result<ModelLibraryUpdateFeed>;
}

pub struct ModelLibraryUpdateSubscription {
    pub recovered_events: Vec<ModelLibraryUpdateEvent>,
    pub cursor_after_recovery: ModelLibraryUpdateCursor,
    pub live_events: ModelLibraryUpdateStream,
}
```

Required behavior:

- update events are appended to the durable SQLite feed before or atomically
  with publishing to live subscribers;
- subscription accepts a cursor from the selector snapshot;
- subscription handshake returns all recoverable events after that cursor and
  then transitions to live events from the same ordered stream;
- the initial handshake response includes the latest delivered cursor so the
  client can persist it before processing live events;
- subscribers receive typed in-process events without JSON;
- cross-process clients subscribe through an explicit local transport client;
- frontend SSE/Electron forwarding is an adapter over the same core bus;
- cursor recovery is the required reconnect path after client disconnect,
  process restart, or stale subscription state;
- if a cursor is no longer recoverable, the feed returns a stale-cursor signal
  that forces a fresh selector snapshot.

This preserves push-based updates while retaining durable recovery. Consumers
should not poll blindly. They should subscribe once, debounce refresh if needed,
and use `list_model_library_updates_since` only for recovery or missed ranges
after a disconnect.

The snapshot-to-subscription handoff must be atomic by contract. A consumer that
loads a snapshot at cursor `C` and calls
`subscribe_model_library_updates_since(C)` must not miss an update committed
after the snapshot cursor was produced, even if that update occurs before the
live subscriber is fully active.

## Reuse Internal Work

Pumas should introduce an internal loaded model snapshot used by descriptor,
summary, and inference-settings resolution.

The current hot paths repeatedly do work that can be shared:

- load effective metadata;
- detect primary file or entry path;
- read dependency binding rows;
- inspect cached summary/detail facts;
- compute or compare source fingerprints;
- derive default inference settings.

A shared internal shape could look like:

```rust
struct IndexedModelResolutionSnapshot {
    model_record: ModelRecord,
    metadata_projection: ModelSelectorMetadataProjection,
    cached_summary: Option<ResolvedModelPackageFactsSummary>,
    cached_summary_status: ModelPackageFactsSummaryStatus,
    dependency_bindings: Vec<ModelDependencyBindingRecord>,
    primary_entry_path: Option<String>,
}
```

This does not need to be public, but public batch APIs should use a shared
internal read path rather than repeatedly loading the same facts.

## Update Feed Semantics

Pumas already has model-library update feed concepts. The selector snapshot and
canonical subscriber model should align with them.

Required behavior:

- The snapshot returns a producer cursor.
- Consumers subscribe with the snapshot cursor using
  `subscribe_model_library_updates_since(cursor)`.
- The subscription handshake returns recovered events after the cursor before
  emitting live events.
- Consumers can also call `list_model_library_updates_since(cursor, limit)` for
  reconnect recovery without missing changes.
- Update events identify the model id, fact family, change kind, refresh scope,
  selected artifact id when relevant, and whether summary/detail rows should be
  refreshed.
- If a cursor is stale, the feed must indicate that a full snapshot is required.

The update feed is owned by core. RPC/SSE/Electron endpoints are forwarding
surfaces, not separate event systems.

## Pantograph Integration After This Change

Pantograph would use the new Pumas API as follows:

- Pantograph decides whether it is embedding a Pumas instance, connecting to a
  running local Pumas instance, or opening a read-only local snapshot.
- Library page startup calls `model_library_selector_snapshot`.
- `puma-lib` graph node options call `model_library_selector_snapshot`.
- Pantograph stores the returned cursor.
- Pantograph subscribes to model-library updates through the explicit client
  surface using `subscribe_model_library_updates_since(cursor)`.
- On reconnect, Pantograph calls `list_model_library_updates_since(cursor)`.
- Selecting a model calls Pantograph hydration.
- Hydration calls Pumas detail APIs for exactly the selected model.
- Runtime scheduling and final backend selection remain in Pantograph.

This keeps initial UI list rendering fast while preserving access to full Pumas
facts where they are actually needed.

Pantograph should not consume the GUI RPC surface unless it intentionally wants
to be an out-of-process local client. Its preferred integration should be the
typed Rust API or an explicit local service client generated from the same core
contracts.

## Success Criteria

- Selector snapshot returns 50-100 indexed rows in `<= 5ms` on a warm local
  SQLite-backed library for direct instance and read-only access modes.
- Local-client selector snapshots use one transport request and meet a separate
  measured same-device budget without per-row calls.
- Selector snapshot does not do filesystem work or deep resolution.
- Selector rows include canonical model-reference and selected-artifact state
  fields so consumers do not treat raw indexed paths as executable contracts.
- Direct Rust snapshot and subscription paths do not use RPC or JSON
  internally.
- Local cross-process clients use explicit instance discovery and an explicit
  transport client.
- Pantograph no longer needs one descriptor, summary, or inference-settings call
  per listed model.
- Single-model hydration still has access to full descriptors, package facts,
  dependency facts, and inference settings.
- Core-owned subscriptions and update cursors let Pantograph keep a startup
  cache current without race-prone reload behavior.
- The snapshot/subscription handoff is atomic: subscribing from a snapshot
  cursor returns recovered events before live events.
- Pumas remains consumer-agnostic and does not add Pantograph-specific runtime
  policy.

## Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Splitting `PumasApi` exposes more call sites than expected | High | Inventory all constructors and classify each caller before implementation |
| Read-only SQLite snapshot access observes incomplete owner writes | High | Use SQLite transaction boundaries and documented read-only consistency semantics; prefer explicit local client when live consistency is required |
| Subscriber events diverge between direct Rust and GUI paths | High | Make core the only event producer and make transports forward core events |
| Snapshot/subscription startup handoff misses events | High | Require cursor-based subscription handshakes that replay recovered events before live events |
| Selector rows become stale without a reliable invalidation source | High | Tie materialization to import, migration, reconciliation, download completion, metadata refresh, and update-feed publication |
| Selector rows lack safe executable identity | High | Include `PumasModelRef`, selected artifact id/path, entry path state, and artifact state in the materialized row |
| Consumers accidentally use an entry path when its state is not ready | High | Add contract and tests that `entry_path` is executable only when `entry_path_state == Ready` and `artifact_state == Ready` |
| Local endpoint discovery creates an accidental network surface | Medium | Prefer platform IPC; if loopback TCP is used, bind localhost only and require a registry token |
| `<= 5ms` target is treated as a cold-start, local-client, or deep-resolution target | Medium | Measure direct/read-only warm SQLite latency separately from selected local-client transport, startup, migration, and maintenance work |
| Batch APIs become loops over slow public single-model calls | Medium | Add tests or tracing assertions that batch paths share internal loaded facts |

## Implementation Plan

This proposal is design context only. The active milestone checklist,
definition of done, verification plan, worker coordination rules, and execution
notes live in `plan.md`.

## Re-Plan Triggers

- The current `PumasApi` surface cannot be split without a larger crate-level
  API break.
- Read-only direct SQLite access cannot provide coherent enough snapshot
  semantics for external consumers.
- Platform IPC support expands the scope beyond a thin local-client adapter.
- Selector materialization requires schema changes that conflict with active
  migration work.
- Existing GUI update forwarding cannot be cleanly backed by the core event bus.
