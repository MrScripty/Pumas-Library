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

## Design Decisions
- Keep backend-owned data in API/hooks flows; components are primarily presentation and action dispatch.
- Centralize transport contracts in `types/api.ts` to keep UI-call sites type-safe.

## Dependencies
**Internal:** preload-exposed RPC methods, local UI component modules.
**External:** React, TypeScript, and Vite build/runtime dependencies.

## Usage Examples
```tsx
import App from './App';

export default App;
```
