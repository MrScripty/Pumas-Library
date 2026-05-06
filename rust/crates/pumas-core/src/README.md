# pumas-core src

## Purpose
Core domain and infrastructure library for Pumas. This crate owns model-library
logic, indexing, networking, process control abstractions, runtime composition,
and the host-facing API surface consumed by RPC and language bindings.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api/` | High-level application API methods exposed to adapters. |
| `model_library/` | Model import, metadata, mapping, dependency, and filesystem logic. |
| `index/` | Indexed model catalog and search data structures. |
| `models/` | Public DTOs and response contracts shared across adapters. |
| `network/` | Connectivity checks, HTTP integrations, and circuit-breaker state. |
| `process/` | Process management utilities used by higher-level integrations. |
| `runtime_profiles.rs` | Backend-owned local runtime profile service, provider adapters, model routes, status journal, and managed launch spec generation. |

## Problem
Provide one backend-owned crate that can act as the composition root for
library state, process/runtime ownership, and host-facing operations without
forcing transport layers to re-implement business workflows.

## Constraints
- The crate serves multiple consumers: RPC, Electron, and bindings.
- SQLite-backed model state must remain canonical for queryable library data.
- Runtime ownership boundaries must stay explicit so only the winning primary
  instance starts primary-owned background work.
- API and DTO surfaces must stay stable enough for adapters to evolve without
  duplicating core logic.

## Decision
- Keep domain and infrastructure logic here so adapters (`pumas-rpc`,
  bindings) stay thin and orchestration remains backend-owned.
- Return structured result/response types from crate APIs to stabilize
  cross-language contracts.
- Use crate-local submodules for domain-specific API surfaces so large features
  can be decomposed without changing the `PumasApi` facade.

## Alternatives Rejected
- Move model-library and reconciliation ownership into transport layers:
  rejected because it would fragment lifecycle ownership across process
  boundaries.
- Split each subsystem into a separate runtime-owning crate immediately:
  rejected because the current repo still relies on one composition root for
  startup, registry, reconcile, and adapter coordination.

## Invariants
- `PumasApi` remains the host-facing facade for crate consumers.
- SQLite-backed model-library state remains canonical for queryable model data.
- Only a primary instance may own watcher, reconcile, and other background
  runtime work for a launcher root.
- Transport adapters consume structured contracts from this crate rather than
  re-implementing domain rules.
- Local model-serving runtimes are addressed internally by `profile_id`.
  Provider-specific endpoint URLs, ports, CPU/GPU settings, llama.cpp modes,
  PID paths, generated presets, and status events stay behind the runtime
  profile service.

## Revisit Triggers
- A second app/runtime needs a materially different startup or ownership model.
- File size and responsibility boundaries in this crate can no longer be kept
  manageable through submodule decomposition.
- Cross-language compatibility requirements demand a dedicated contracts crate.

## Dependencies
**Internal:** `pumas-app-manager` (for launcher/version integration at higher layers), internal modules in this crate.
**External:** async runtime (`tokio`), serialization (`serde`), storage/network utilities (`rusqlite`, `reqwest`).

## Related ADRs
- None identified as of 2026-04-10.
- Reason: current runtime and persistence decisions are documented in module
  READMEs and implementation plans rather than standalone ADRs.
- Revisit trigger: startup ownership, persistence contracts, or adapter
  boundaries become cross-repo compatibility commitments.

## Usage Examples
```rust
let status = api.get_library_status().await?;
if status.success {
    println!("models={}", status.model_count);
}
```

## API Consumer Contract
- Consumers construct or discover `PumasApi` and use it as the stable facade for
  library, process, network, and migration operations.
- Read paths may trigger bounded backend-owned reconcile work before returning
  data when runtime freshness is unknown.
- Errors are returned as structured backend errors rather than transport-local
  partial states.
- Compatibility is facade-first: internal module extraction may change, but
  host-facing method signatures and result contracts should remain additive
  unless a documented breaking change is introduced.

## Structured Producer Contract
- This crate produces machine-consumed DTOs and persisted model metadata/index
  state through its submodules.
- `models/` defines response and DTO shapes consumed by adapters and must remain
  stable for external callers.
- `model_library/` produces persisted SQLite and `metadata.json` artifacts and
  owns regeneration rules for that state.
- `runtime_profiles.rs` produces the persisted
  `launcher-data/metadata/runtime-profiles.json` profile/route contract and
  profile-scoped runtime artifacts such as generated llama.cpp presets under
  `launcher-data/runtime-profiles/`.
- Revisit trigger: additional generated schemas or manifests become
  compatibility-critical and need their own versioned contract module.
