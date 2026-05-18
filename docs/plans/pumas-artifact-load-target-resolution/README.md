# Pumas Artifact Load Target Resolution

## Purpose
This directory contains the accepted Pantograph-facing proposal and execution
plan for adding a Pumas-owned artifact load-target resolver.

## Contents
| File | Description |
| ---- | ----------- |
| `proposal.md` | Reviewed cross-repository proposal describing the API contract, ownership boundaries, standards review, risks, and completion criteria. |
| `plan.md` | Pumas implementation plan with thin vertical slices, verification gates, issue tracking, and commit traceability. |

## Problem
Pantograph needs an execution-ready local artifact load target without taking
ownership of Pumas storage layout, selected-artifact semantics, or
external-reference validation.

## Constraints
- Pumas remains the owner of model identity, selected artifact identity,
  artifact availability, external-reference validation, and local load paths.
- Pantograph must not join Pumas roots, scan model files, or repair selected
  artifact refs.
- Read-only consumers must not mutate metadata, SQLite, cache rows, or
  external-reference validation state.
- Resolver behavior must be shared by owner, read-only, API, RPC, and local
  client surfaces rather than reimplemented per surface.
- The resolver must not deep-scan roots, regenerate package facts, or compute
  fingerprints on the hot path.

## Decision
Implement one model-library resolver core with explicit `OwnerFresh` and
`ReadOnlyIndexed` modes. Add public DTOs in the model contract layer, keep
transport/client surfaces thin, and expose the capability in validated slices
with atomic commits after each successful slice.

## Alternatives Rejected
- Pantograph joins library roots or scans local files: rejected because it
  duplicates Pumas ownership and cannot safely support external references.
- Python workers call Pumas directly: rejected because workers should receive a
  pre-approved load target, not storage-resolution authority.
- Wrapping `resolve_model_package_facts` or
  `resolve_model_execution_descriptor`: rejected because those are broader
  model-level APIs with hydration, primary-file, and mutation behavior that does
  not match exact selected-artifact resolution.

## Invariants
- `PumasModelRef` is the authoritative selected-artifact reference.
- Caller-observed facts are stale-check inputs only.
- `PumasReadOnlyLibrary` rejects `OwnerFresh`; it never silently downgrades or
  mutates.
- Returned load paths are Pumas-approved paths and include `StorageKind` plus
  `AssetValidationState`.
- Resolver response states remain typed; consumers must not parse diagnostic
  message text.

## Revisit Triggers
- Existing indexed/cache state cannot represent required resolver states.
- Implementation requires a schema or persisted artifact migration.
- Owner freshness requires new background tasks, polling, retries, queues, or
  lifecycle management.
- Any public surface cannot enforce its allowed resolver modes.
- Correctness requires deep scans, full package-facts regeneration, or hot-path
  fingerprint computation.

## Dependencies
**Internal:** `pumas-core` model contracts, model library, package-facts cache,
model index, API state, IPC local client, `pumas-rpc`, and Pantograph runtime
integration.

**External:** None for the first read-only/core slice beyond existing SQLite and
filesystem dependencies.

## Related ADRs
- None identified as of 2026-05-17.
- Reason: This is an implementation plan for a new API surface, not yet a
  stable architecture decision record.
- Revisit trigger: The resolver becomes the long-term replacement path for
  existing serving primary-file resolution call sites.

## Usage Examples
Read `proposal.md` for accepted design constraints, then execute `plan.md`
slice by slice. Update milestone status, issue notes, verification evidence,
and commit references as each slice lands.

## API Consumer Contract
- The runtime API contract is defined by the DTOs and behavior described in
  `proposal.md`.
- This planning directory itself is internal documentation, but changes to the
  runtime API must update the implementation plan, source READMEs, tests, and
  fixture contracts in the same slice.

## Structured Producer Contract
- Stable planning fields are milestone status, issue register entries,
  verification evidence, and commit references.
- The plan may evolve during implementation, but material deviations must be
  recorded before dependent code proceeds.
