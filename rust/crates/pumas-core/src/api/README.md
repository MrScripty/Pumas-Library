# pumas-core api

## Purpose
Defines the primary API facade (`PumasApi`) methods that orchestrate core subsystems without embedding transport-specific behavior.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `builder.rs` | API builder and initialization wiring. |
| `links.rs` | Link registry, health, cleanup, cascade delete, and link-exclusion API methods. |
| `migration.rs` | Migration report generation/execution API methods and partial-download relocation helpers. |
| `mapping.rs` | App-facing model-mapping, sync, and cross-filesystem warning API methods. |
| `models.rs` | Model-library query, metadata, import, review, and reclassification API methods. |
| `network.rs` | Connectivity and network-status API methods. |
| `process.rs` | Process lifecycle/status API methods. |
| `reconciliation.rs` | Reconcile scheduling, watcher routing, and startup freshness rules. |
| `state.rs` | Primary-state dispatch and IPC method execution. |
| `state_hf.rs` | HuggingFace search, download, metadata-refetch, and auth helpers used by primary-state IPC dispatch. |
| `state_process.rs` | Process lifecycle, launch, and status helpers used by primary-state IPC dispatch. |
| `state_runtime.rs` | Disk, status, system-resource, and network-status helpers used by primary-state IPC dispatch. |

## Problem
Expose a stable host-facing API while keeping runtime ownership, reconciliation, and transport wiring in one backend-owned composition root.

## Constraints
- Primary-owned background work must not leak into client instances.
- API methods must preserve facade compatibility for embedders and bindings.
- Reconciliation must remain event-driven and idempotent for unchanged state.

## Decision
- Group API methods by domain so transport layers call one facade instead of reaching into subsystems directly.
- Keep startup, watcher, and reconciliation lifecycle ownership in this directory so primary/client behavior stays explicit.
- Split larger API surfaces into focused submodules such as `migration.rs` when
  one feature area starts adding its own lifecycle, reporting, and recovery
  helpers.
- Keep the primary-state HuggingFace workflow helpers in a dedicated sibling
  module so `state.rs` stays focused on dispatch and non-HF runtime ownership.
- Keep process lifecycle helpers in a dedicated sibling module so launch/stop
  logic can evolve without further inflating the core dispatch file.
- Keep runtime status helpers in a dedicated sibling module so system and
  connectivity reporting can evolve without coupling to core dispatch wiring.
- Keep link-registry health/cleanup flows and app mapping flows in separate
  modules so `models.rs` stays centered on model-library metadata and import
  behavior.

## Alternatives Rejected
- Let transport layers orchestrate model-library and process subsystems directly: rejected because lifecycle ownership would fragment across process boundaries.
- Split startup ownership across multiple modules: rejected because it weakens single-owner guarantees for watcher and reconcile flows.

## Invariants
- Only the primary instance starts watcher, reconcile, and other primary-owned background work.
- Primary-owned watcher and reconciliation tasks stay under `RuntimeTasks` ownership so shutdown can abort outstanding work deterministically.
- `PumasApi` remains the facade boundary; transport code adapts requests and responses but does not own domain state.
- Startup establishes runtime ownership, but freshness-sensitive read paths may
  still trigger bounded reconcile work before returning state.
- Migration report generation and execution must operate on reconciled library
  state rather than stale SQLite projections.

## Revisit Triggers
- A new host-facing API surface needs different lifecycle guarantees.
- Startup or reconcile ownership moves out of the current composition root.
- Client and primary roles diverge enough to justify separate public facades.

## Dependencies
**Internal:** `crate::model_library`, `crate::network`, `crate::process`, `crate::models`.
**External:** standard library path/collections and async primitives.

## Related ADRs
- None identified as of 2026-03-11.
- Reason: runtime ownership and facade decisions are currently tracked in architecture docs and implementation plans, not ADRs.
- Revisit trigger: API lifecycle or ownership semantics become binding across multiple repos or processes.

## Usage Examples
```rust
let net = api.get_network_status_response().await;
println!("offline={}", net.is_offline);
```

## API Consumer Contract
- Consumers call `PumasApi` methods as the stable facade regardless of whether the instance is primary or attached as a client.
- Startup ordering is backend-owned: callers construct the API, then use methods; they do not manually start watcher or reconcile loops.
- Read paths may trigger bounded on-demand reconcile when the backend marks the
  library dirty or runtime freshness is unknown.
- Migration dry-run and execute calls may force a full-library reconcile before
  generating artifacts so the returned report reflects current library state.
- Errors are surfaced as backend errors rather than partial transport-specific states.
- Compatibility policy is facade-first: internal reconcile and startup sequencing may evolve without changing host-facing method shapes unless a documented breaking change is introduced.

## Structured Producer Contract
- None identified as of 2026-03-11.
- Reason: this directory orchestrates facades and lifecycle behavior but does not itself define a persisted structured artifact format.
- Revisit trigger: API-layer code begins producing machine-consumed manifests, config files, or persisted metadata contracts.
