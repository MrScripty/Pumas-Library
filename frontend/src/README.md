# frontend src

## Purpose
React renderer source for the Pumas desktop UI. This directory contains app composition, API adapters, shared types, state hooks, and view components.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `App.tsx` | Top-level UI composition and page-level orchestration. |
| `api/` | Frontend API wrappers over the exposed Electron preload bridge. |
| `components/` | Reusable and page-level React components. |
| `hooks/` | Custom hooks for state, status polling, and domain workflows. |
| `types/` | TypeScript contracts for API and UI models. |
| `utils/` | Formatting, logging, and helper utilities. |

## Problem
Render launcher and model-library functionality in the desktop UI without
turning the renderer into a competing source of truth for backend-owned state
or transport semantics.

## Constraints
- Backend-owned data must remain authoritative.
- The renderer consumes a preload/API boundary rather than importing backend
  infrastructure directly.
- UI-only state must stay separate from business state that affects behavior or
  persistence.
- Shared transport contracts need to remain type-safe across Electron,
  frontend, and backend layers.

## Decision
- Keep backend-owned data in API/hooks flows; components are primarily
  presentation and action dispatch.
- Centralize transport contracts in `types/api.ts` to keep UI call sites
  type-safe.
- Use source-root subdirectories to keep feature composition, shared hooks, and
  DTOs separated by responsibility rather than mixing them in one flat layer.

## Alternatives Rejected
- Let components call backend infrastructure directly: rejected because it
  weakens the transport boundary and testability.
- Keep transport types distributed across components and hooks: rejected
  because it increases drift and makes cross-layer changes harder to audit.

## Invariants
- Backend-owned state is consumed through typed API wrappers and hooks.
- Renderer-local state is limited to presentation and interaction concerns.
- Shared UI and transport contracts remain discoverable from this source root
  rather than hidden in page components.

## Revisit Triggers
- The renderer adopts a new application shell or routing model that changes the
  current source-root boundaries.
- Transport contracts or UI state management move into their own dedicated
  packages.
- Feature growth makes the current top-level directory split insufficiently
  clear.

## Dependencies
**Internal:** preload-exposed RPC methods, local UI component modules.
**External:** React, TypeScript, and Vite build/runtime dependencies.

## Related ADRs
- None identified as of 2026-04-10.
- Reason: frontend structure is currently documented through source READMEs and
  implementation plans rather than ADRs.
- Revisit trigger: a major renderer architecture or state-management decision
  becomes long-lived and cross-team.

## Usage Examples
```tsx
import App from './App';

export default App;
```

## API Consumer Contract
- Code in this source root consumes the preload/API wrapper and shared frontend
  DTO contracts.
- Callers should treat returned backend state as authoritative and avoid
  optimistic updates for persisted model-library data.
- Errors must surface through shared UI feedback paths rather than ad hoc local
  fallback semantics.
- Compatibility is internal-to-repo but additive: shared types and API wrapper
  semantics should evolve without hidden breaking changes for sibling modules.

## Structured Producer Contract
- `types/` defines frontend-consumed contract shapes for API and UI models.
- Presentation modules may interpret backend enums and labels, but they should
  not redefine machine-consumed contract semantics independently.
- Revisit trigger: the frontend begins generating machine-consumed persisted
  artifacts, schemas, or exported configuration.
