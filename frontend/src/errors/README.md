# frontend errors

## Purpose
Provide shared frontend error classes and guards so API wrappers, hooks, and
components surface failures consistently at the desktop bridge and UI layers.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `index.ts` | Shared error hierarchy and type guard helpers for frontend callers. |

## Problem
Without one error hierarchy, bridge failures, validation errors, metadata
problems, and process issues would each be represented differently across the
renderer, making boundary handling inconsistent and harder to test.

## Constraints
- Errors should describe boundary failures without turning components into
  hidden business-logic owners.
- API wrappers need a stable error type for missing or unavailable desktop
  bridge methods.
- New error naming should reflect the current desktop shell architecture rather
  than the removed PyWebView runtime.

## Decision
- Keep shared error classes in one module.
- Use `APIError` for desktop-bridge boundary failures.
- Keep lightweight type guards here so hooks and components do not duplicate
  instanceof logic.

## Alternatives Rejected
- Throw raw `Error` everywhere: rejected because callers cannot distinguish
  bridge failures from validation or process errors.
- Define feature-local error classes in each hook/component: rejected because
  that would fragment error semantics.

## Invariants
- `APIError` denotes desktop bridge access/call failures.
- Error helpers stay framework-light and reusable across renderer modules.
- Hooks and components may map errors to UI feedback, but they should not
  redefine the shared error taxonomy.

## Revisit Triggers
- The frontend adopts a richer cross-process error envelope from the backend.
- A second renderer runtime needs a different shared error boundary.
- Error handling becomes generated or schema-driven.

## Dependencies
**Internal:** API wrappers, hooks, and UI error handlers.
**External:** none.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: the error boundary remains local to the frontend renderer and is
  currently documented in module READMEs and plans.
- Revisit trigger: a repo-wide error taxonomy decision spans multiple runtimes.

## Usage Examples
```ts
throw new APIError('API not available');
```

## API Consumer Contract
- Frontend callers should throw or handle these shared error classes rather than
  inventing parallel error shapes.
- `APIError` indicates the desktop bridge is missing, unavailable, or returned a
  boundary-level failure.
- Compatibility expectation: new error subclasses may be added, but existing
  class meanings should remain stable for callers.

## Structured Producer Contract
- This directory does not emit persisted structured artifacts.
- It does produce a stable in-process error taxonomy consumed by hooks and
  components.
- Revisit trigger: errors become serialized into saved or cross-process
  artifacts.
