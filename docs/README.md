# Documentation

The documentation set is intentionally small. Current behavior belongs in the
nearest authoritative document; completed plans and obsolete audits remain
available through Git history rather than in the working tree.

## Current Guides

| Document | Purpose |
| --- | --- |
| [Project README](../README.md) | Product overview and quick start |
| [Architecture](ARCHITECTURE.md) | Process, state, storage, and contract ownership |
| [Development](DEVELOPMENT.md) | Setup, standards routing, and verification |
| [Contributing](../CONTRIBUTING.md) | Change, planning, documentation, and commit expectations |
| [Releasing](../RELEASING.md) | Current release flow, required evidence, and known gaps |
| [Security](SECURITY.md) | Reporting and current trust-boundary guidance |
| [Native bindings](native-bindings.md) | Binding status, generation, and packaging |

## Durable Decisions

- [ADR 0001: ONNX Runtime provider model](adr/0001-onnx-runtime-provider-model.md)

## Point-in-Time Audits

- [2026-09-03 current-standards audit](audits/current-standards-2026-09-03/README.md)

Audits describe the named repository and standards commits. They are evidence
and remediation inputs, not current operating instructions.

## Active and Planned Work

- [2026-09-03 current-standards remediation program](plans/current-standards-remediation-2026-09-03/plan.md)
- [Local intent API and transport-independent domain language](plans/local-intent-api-2026-09-12/plan.md) — complete within the recorded local scope; native resolution, acquisition, durable declarations and existing IPC/RPC projections accepted; nodes/fleets/new networking deferred until after the next release

An implementation invocation must name the exact focused `plan.md` and an
explicit `start`, `continue`, or `verify` operation. Plans remain temporary
execution authority; durable accepted decisions move to the appropriate guide
or ADR when the program finishes.

## Subsystem Guides

- [Frontend](../frontend/README.md)
- [Electron desktop shell](../electron/README.md)
- [Rust workspace](../rust/README.md)
- [Core Rust crate](../rust/crates/pumas-core/README.md)
- [Torch sidecar](../torch-server/README.md)
- [Developer scripts](../scripts/README.md)
- [Generated bindings](../bindings/README.md)
- [Plugin manifests](../launcher-data/plugins/README.md)

When guidance conflicts with executable configuration or code, treat that as
documentation drift and fix the document and owning behavior together.
