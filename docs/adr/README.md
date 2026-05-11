# Architecture Decision Records

## Purpose

This directory contains durable architecture decisions for cross-layer Pumas
changes that affect runtime boundaries, persisted contracts, or public
integration behavior.

## Contents

| File | Description |
| ---- | ----------- |
| `0001-onnx-runtime-provider-model.md` | Decision record for the provider model, provider-scoped routes, and ONNX Runtime embedding serving contract. |

## Problem

Some implementation plans change enough runtime ownership that the decision
must remain findable after the plan completes. ADRs preserve the accepted
architecture, rejected alternatives, invariants, and revisit triggers for those
changes.

## Constraints

- ADRs must be stable project documentation, not implementation scratch notes.
- ADRs must link to the plan or code area that requires the decision.
- ADRs must record compatibility and migration impact when persisted artifacts
  or external contracts change.

## Decision

Use numbered Markdown ADRs under `docs/adr/` for durable architecture decisions.
Implementation plans can link to ADRs when a plan slice depends on an accepted
cross-layer contract.

## Alternatives Rejected

- Keep decisions only in implementation plans: rejected because plans are
  execution artifacts and become harder to discover after the work completes.
- Store ADRs under `docs/architecture/`: rejected because that directory
  describes current architecture snapshots, while ADRs record decisions and
  alternatives over time.

## Invariants

- ADR numbers are append-only.
- Accepted ADRs are not rewritten to hide historical decisions; later changes
  add a superseding ADR or an explicit amendment section.
- ADRs that describe structured contracts name the owner and compatibility
  policy or link to the plan section that does.

## Revisit Triggers

- A second durable decision format is introduced.
- Existing architecture documents become ADR-like decision ledgers.
- Release or PR tooling starts validating ADR metadata.

## Dependencies

**Internal:** `docs/plans/`, `docs/architecture/`, local Coding Standards.

**External:** None.

## Related ADRs

- None identified as of 2026-05-11.
- Reason: This is the first ADR directory for the current Rust/Electron
  architecture.
- Revisit trigger: Another ADR establishes shared numbering, metadata, or
  supersession conventions.

## Usage Examples

Plan traceability example:

```markdown
- ADR added/updated: `docs/adr/0001-onnx-runtime-provider-model.md`
```

## API Consumer Contract

- ADR consumers may rely on file names being stable once committed.
- ADR status values are plain Markdown text such as `Accepted`, `Superseded`,
  or `Proposed`.
- ADRs are documentation only; runtime code must not parse these files.

## Structured Producer Contract

- Stable producer fields are top-level headings, status, date, decision,
  consequences, alternatives, invariants, and revisit triggers.
- ADR numbering is manual and monotonically increasing.
- If an ADR is superseded, add a new ADR and link both records instead of
  renumbering files.
