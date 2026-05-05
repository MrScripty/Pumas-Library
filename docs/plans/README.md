# Plans

## Purpose
This directory holds implementation plans for cross-module changes that need explicit sequencing, risk controls, and verification before code is written. The plans here are scoped to the current Rust/Electron codebase and are intended to extend existing subsystems rather than introduce parallel workflows.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `strict-primary-claim-and-reconciliation-idempotence-plan.md` | Plan for making reconciliation side effects idempotent and enforcing a strict single-primary-per-launcher-root startup contract. |
| `transparent-client-mode-dispatch-plan.md` | Plan for making raw Rust `PumasApi` callers converge to a real client-backed handle with explicit module-by-module IPC parity. |
| `external-reference-diffusers-implementation-plan.md` | Backend-first implementation plan for external-reference diffusers bundles, including schema, validation, execution descriptor, and regression controls. |
| `directory-import-disambiguation-implementation-plan.md` | Plan for GUI/backend directory import classification so bundle roots and multi-model containers are imported safely and distinctly. |
| `hf-classification-and-library-repair-remediation-plan.md` | Plan for standards remediation, saved HF/local-library evidence, and non-model-specific fixes for model classification and organization drift. |
| `transformers-aligned-artifact-identity-migration/plan.md` | Plan for separating Hugging Face repository identity from selected artifacts, normalizing architecture-family paths, and migrating existing model files safely. |
| `model-library-integrity-event-refresh-plan.md` | Plan for clearing model-library integrity warnings through backend-owned update events and frontend refresh, not migration-specific UI mutation. |
| `cross-platform-desktop-launcher-facade-plan.md` | Plan for moving desktop launcher behavior behind a shared cross-platform core with thin Unix and Windows wrappers plus README contract updates. |
| `npm-audit-and-desktop-bridge-terminology-remediation-plan.md` | Plan for clearing workspace npm audit findings and renaming the Electron desktop bridge away from legacy PyWebView-first terminology without breaking compatibility. |
| `workspace-tooling-dependency-ownership-refactor-plan.md` | Plan for removing the root `jsdom` pin by making frontend test tooling resolve from its owning workspace without relying on incidental npm hoisting. |
| `local-runtime-profiles-and-ollama-version-manager/plan.md` | Plan for stabilizing the Ollama version-manager UI and adding backend-owned local runtime profiles for Ollama and llama.cpp with per-model routing. |

## Problem
Large model-library changes touch persisted metadata, reconciliation, runtime resolution, and UI/API contracts at the same time. Without a written plan, it is easy to create competing paths for import, validation, or execution that weaken reliability.

## Constraints
- Plans must follow the coding-standards plan template and sequencing rules.
- Work in this directory must reflect the current Rust/Electron architecture, not legacy Python-era assumptions.
- Plans must prefer extending existing model-library, index, dependency, and reconciliation systems over adding new registries or runtime facades.

## Decision
Use `docs/plans/` as the location for multi-file implementation plans that need durable traceability. Keep each plan focused on one cross-layer change and tie it back to the current architecture and verification expectations.

## Alternatives Rejected
- Inline issue-comment planning only: rejected because it is not discoverable in the repo and does not help future contributors understand sequencing and risk decisions.
- Storing plans under `docs/architecture/`: rejected because architecture docs describe stable system shape, while implementation plans are temporary execution artifacts.

## Invariants
- Plans in this directory must describe how they integrate with existing subsystems.
- Plans must call out affected structured contracts and persisted artifacts.
- Plans must include milestone-level verification and re-plan triggers.

## Revisit Triggers
- A second active implementation plan is added and the directory structure no longer stays easy to scan.
- The repo adopts a dedicated ADR or RFC directory for execution planning and supersedes this location.

## Dependencies
**Internal:** `docs/README.md`, `docs/architecture/`, coding standards in `/media/jeremy/OrangeCream/Linux Software/Coding-Standards/`.
**External:** None.

## Related ADRs
- None identified as of 2026-03-08.
- Reason: The repo currently documents architecture in `docs/architecture/` and does not yet maintain ADR files.
- Revisit trigger: A cross-team design decision requires a durable architectural record beyond an execution plan.

## Usage Examples
Read the plan before implementation starts, then update milestone status and execution notes as slices are completed.

## API Consumer Contract
- None.
- Reason: This directory is internal project documentation and is not consumed by runtime callers.
- Revisit trigger: Plans are exported to an external process or automation surface.

## Structured Producer Contract
- Files in this directory must follow the repo plan template structure: objective, scope, inputs, definition of done, milestones, verification, risks, re-plan triggers, and completion summary.
- Plans are descriptive artifacts only; they do not directly drive code generation or migrations.
- Compatibility expectation: plans may evolve during implementation, but updates must preserve milestone traceability and record material deviations.
- Revisit trigger: Plan files become machine-consumed inputs to automation or release tooling.
