# Pumas Fast Model Snapshot And Client API

## Purpose

This directory contains the proposal and implementation plan for replacing
slow per-model selector hydration and hidden Rust API transport behavior with a
fast selector snapshot, explicit Pumas instance/client roles, and a core-owned
subscriber model.

## Contents

| File | Description |
| ---- | ----------- |
| `proposal.md` | Reviewed design proposal and Pantograph feedback resolution. |
| `plan.md` | Standards-compliant implementation plan with thin vertical slices, risks, verification, and worker boundaries. |

## Problem

Pantograph needs fast, safe access to Pumas model facts for selector and graph
authoring workflows. The current API shape encourages expensive per-model
hydration and hides whether Rust calls are direct in-process operations or
transport-backed calls to another Pumas instance.

## Constraints

- Pumas remains the canonical owner of model identity, selected artifact state,
  package facts, update cursors, and local library integrity.
- Pantograph owns runtime scheduling, workflow semantics, queueing, and node
  execution policy.
- Direct Rust APIs must not secretly route through RPC.
- Cross-process clients may use local transport, but attachment must be
  explicit and discoverable.
- Snapshot reads must stay backed by SQLite/indexed state and avoid filesystem
  scans or deep per-model resolution.

## Decision

Keep the reviewed proposal as design context and execute through `plan.md`.
The implementation starts with the smallest useful vertical slice: materialized
selector rows, canonical model references, and direct/read-only snapshot access
that Pantograph can consume without hydrating every listed model. Broader
transport and batch hydration work follows only after that fast path is proven.

## Alternatives Rejected

- Expanding transparent `PumasApi` client mode: rejected because it hides
  whether Rust API calls are direct or transport-backed.
- Making Pantograph use GUI RPC by default: rejected because RPC is a transport
  adapter, not the canonical API for non-GUI clients.
- Deferring Pantograph integration until all transport work is complete:
  rejected because the current breakage is the slow selector path and should be
  proven early.

## Invariants

- `indexed_path` is never the executable model contract for API consumers.
- A selector `entry_path` is executable only when `entry_path_state == Ready`
  and `artifact_state == Ready`.
- Snapshot-to-subscription handoff must not miss committed update events.
- Direct/read-only selector snapshots must not perform filesystem scans,
  metadata JSON loads, RPC calls, or per-model deep resolution.
- Local-client snapshots must use one transport request per snapshot, not one
  request per row.

## Revisit Triggers

- Pantograph requires fields that cannot be materialized without deep
  resolution.
- The selected local transport cannot meet the measured local-client target.
- The core update feed cannot provide atomic snapshot/subscription handoff.
- Splitting `PumasApi` produces a larger public API break than expected.

## Dependencies

**Internal:** `pumas-core` model index, model-library metadata/cache paths,
local registry, current IPC/SSE update forwarding, Electron bridge update
subscription, and Pantograph-facing Rust API consumers.

**External:** SQLite and platform local IPC capabilities such as Unix sockets,
Windows named pipes, or localhost TCP fallback.

## Related ADRs

- None identified as of 2026-05-06.
- Reason: This work is still an implementation plan and proposal, not yet a
  durable architecture decision record.
- Revisit trigger: The explicit instance/client API split is accepted as the
  long-term public contract or shipped in a release.

## Usage Examples

Read `proposal.md` for design rationale, then execute `plan.md` slice by slice.
Update the milestone status and execution notes as each verified slice is
committed.

## API Consumer Contract

- `proposal.md` describes the intended future API consumer contract for
  Pantograph and other clients.
- `plan.md` is not itself a runtime API, but every implementation slice must
  keep API consumer semantics explicit: ownership role, access mode, lifecycle,
  error behavior, and compatibility impact.
- Compatibility policy: transparent `PumasApi` auto-client behavior is not a
  preserved compatibility requirement.

## Structured Producer Contract

- The files in this directory are human-maintained Markdown planning
  artifacts.
- Stable fields are section headings, milestone checklists, verification
  criteria, risks, and execution notes.
- Implementation updates must preserve traceability from proposal requirement
  to milestone, verification, commit, and completion summary.
