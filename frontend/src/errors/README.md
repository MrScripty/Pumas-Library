# frontend errors

## Purpose
Shared frontend error classes and helpers for consistent API/UI error handling.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `index.ts` | Error type definitions and common constructors. |

## Design Decisions
- Centralize error shape definitions for predictable handling in hooks/components.

## Dependencies
**Internal:** API wrappers and hooks.
**External:** none.

## Usage Examples
```ts
throw new APIError('API not available');
```
