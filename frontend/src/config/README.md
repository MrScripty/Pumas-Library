# frontend config

## Purpose
Frontend configuration constants and typed app/theme descriptors used by the renderer.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `apps.ts` | App metadata/config used for panel routing and labels. |
| `theme.ts` | Theme tokens and visual configuration. |

## Design Decisions
- Keep app/theme constants centralized to avoid duplicated literals.
- Configuration is data-only; no side-effectful runtime logic.

## Dependencies
**Internal:** shared types and renderer components consuming config.
**External:** none.

## Usage Examples
```ts
import { APPS } from './apps';
```
