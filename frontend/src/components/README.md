# frontend components

## Purpose
UI components for dashboards, dialogs, status displays, app panels, and
model-management workflows. This directory holds presentation and thin
interaction layers over backend-owned state exposed through the preload/API
bridge.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `ModelManager.tsx` | Main model management screen and interactions. |
| `MigrationReportsPanel.tsx` | Displays migration dry-run and execution artifacts and dispatches migration actions. |
| `ModelKindIcon.tsx` | Renders model/task-kind tokens into consistent icons and labels. |
| `VersionSelector.tsx` | Version install/switch/update UI. |
| `ModelImportDialog.tsx` | Import flow for local and remote model files. |
| `MappingPreview.tsx` | Mapping preview and conflict-resolution workflow. |
| `ui/` | Small reusable primitives (buttons, tooltips, list items). |
| `app-panels/` | App-specific panel renderers and sections. |

## Problem
Render backend-owned launcher, model-library, and migration state in a way that
is interactive for operators without allowing React components to become a
second source of truth for business state.

## Constraints
- Backend-owned data must remain authoritative.
- Components must coordinate with the existing preload/API boundary rather than
  talking to infrastructure directly.
- Large screens such as `ModelManager.tsx` still need decomposition reviews when
  they accumulate multiple responsibilities.
- Migration and import workflows must reflect real backend state and not use
  optimistic updates for persisted library data.

## Decision
- Keep feature-level composition in high-level components; primitives stay in
  `ui/`.
- Keep long-running async interactions in hooks, API wrappers, or backend
  methods rather than burying lifecycle ownership in leaf components.
- Preserve additive interpretation components such as `ModelKindIcon.tsx` so
  backend classification improvements can be surfaced without rewriting the
  full page component.

## Alternatives Rejected
- Store authoritative model-library workflow state inside React components:
  rejected because it would violate backend-owned data rules and create drift.
- Collapse all model-management UI into generic primitives only: rejected
  because page-level orchestration still needs feature-aware components.

## Invariants
- Components display backend-owned library and migration state rather than
  inventing alternate business state locally.
- Operator actions go through explicit backend/API calls.
- Presentation-only state such as expansion, hover, or modal visibility may be
  local, but persisted model-library truth is not.

## Revisit Triggers
- `ModelManager.tsx` grows enough that helper extraction is no longer adequate.
- A new frontend state-management boundary is introduced for backend-owned data.
- Migration, import, and library status surfaces diverge enough to require a
  dedicated feature subdirectory split.

## Dependencies
**Internal:** hooks, shared types, API wrappers, config.
**External:** React and styling utilities.

## Related ADRs
- None identified as of 2026-04-10.
- Reason: frontend component structure is currently governed by README guidance
  and implementation plans rather than formal ADRs.
- Revisit trigger: a new frontend architecture or state-management decision
  becomes cross-team or long-lived enough to require an ADR.

## Usage Examples
```tsx
<ModelManager />
```

## API Consumer Contract
- Components in this directory consume typed frontend API wrappers and shared UI
  model types.
- They must treat returned backend state as authoritative.
- Errors should be rendered as operator feedback and not silently converted into
  local fallback state that changes business meaning.
- Compatibility expectation is internal-to-repo but additive: component props
  and shared usage patterns should evolve without hidden breaking semantics for
  sibling callers.

## Structured Producer Contract
- Most components in this directory do not produce persisted structured
  artifacts.
- `ModelKindIcon.tsx` and migration/report views do produce user-visible
  interpretations of backend enums and labels, so label/icon mapping changes
  must stay aligned with backend semantics.
- Revisit trigger: a component starts generating machine-consumed JSON, config,
  or persisted view state.
