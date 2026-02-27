# frontend utils

## Purpose
Utility functions for formatting, logging, drag math, network status helpers, and shared pure helper logic.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `logger.ts` | Frontend logging helper wrapper. |
| `formatters.ts` | Generic formatting helpers. |
| `appVersionState.ts` | Version state helper functions. |
| `dragAnimations.ts` | Drag animation helper utilities. |
| `networkStatusMonitor.ts` | Connectivity/status monitor helpers. |

## Design Decisions
- Keep utilities pure and side-effect light where practical.
- Avoid embedding domain orchestration in utils; use hooks/services for that.

## Dependencies
**Internal:** hooks and components consuming helper functions.
**External:** none.

## Usage Examples
```ts
const label = formatBytes(bytes);
```
