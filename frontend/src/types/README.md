# frontend types

## Purpose
TypeScript contracts for API payloads, app metadata, plugin interfaces, and version-related models.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api.ts` | Primary frontend/backend RPC type contracts. |
| `apps.ts` | App-level view/config types. |
| `plugins.ts` | Plugin contract types. |
| `versions.ts` | Version-management related types. |

## Design Decisions
- Define transport contracts once and share them across APIs, hooks, and components.
- Prefer explicit interfaces for stable boundary behavior.

## Dependencies
**Internal:** all API/hook/component modules using typed payloads.
**External:** TypeScript.

## Usage Examples
```ts
import type { LibraryStatusResponse } from './api';
```
