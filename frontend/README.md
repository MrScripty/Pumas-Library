# Frontend

The frontend is the React renderer for the Pumas desktop app. It presents
backend-owned library and runtime state; it does not access the filesystem,
spawn processes, or decide durable model status.

## Stack and Boundary

- React 19 and TypeScript
- Vite 6
- Tailwind CSS 4 and semantic CSS tokens
- Framer Motion
- React Aria interaction primitives
- Vitest, Testing Library, and jsdom

The renderer calls the typed adapter under `src/api/`, which delegates to the
sandboxed Electron preload API. Values received from Electron, the backend, or
browser persistence are untrusted at runtime even when a TypeScript interface
describes their expected shape. Decode them before they enter application
state.

```text
components and hooks
  -> frontend API adapter
    -> window.electronAPI
      -> Electron IPC and local RPC
```

## State Ownership

- Hooks own async work, cancellation, cleanup, stale-invocation protection, and
  renderer subscriptions.
- Components own presentation and short-lived interaction drafts.
- The Rust backend owns model availability, download state, repair outcomes,
  runtime profiles, serving instances, and status telemetry.
- Cached startup data is a projection. Its age, provenance, refresh state, and
  degraded outcome must remain visible to the caller and, where relevant, the
  user.

Do not infer that a model is complete from its name, repository, or a partial
set of files. Repository identity and artifact identity are distinct: one
Hugging Face repository may contain multiple files or quantizations, while the
same model published in another repository remains a separate model.

Model conversion is available through the backend and typed desktop APIs, but
the GUI workflow is incomplete. Library rows intentionally expose no conversion
button until [FE-I23](../docs/plans/current-standards-remediation-2026-09-03/frontend-and-ui/issues.md)
delivers and verifies setup consent, capability/readiness, start, progress and
cancellation. This is not removal of backend conversion support.

## Source Guide

| Path | Responsibility |
| --- | --- |
| `src/api/` | Electron/backend adapter and response normalization |
| `src/components/` | Screens, panels, and reusable UI |
| `src/hooks/` | Async workflows and subscriptions |
| `src/types/` | Consumer-side TypeScript projections |
| `src/config/` | Feature and theme mappings |
| `src/utils/` | Pure presentation and provider utilities |
| `src/test/` | Shared test setup and helpers |

Prefer grouping code by owned capability over adding another cross-cutting
utility or an explanatory README in every directory.

## Themes and Accessibility

Theme tokens are defined in `src/index.css`; programmatic theme mappings live
in `src/config/theme.ts`. Use semantic tokens instead of hard-coded colors.

Prefer native semantic controls. Composite widgets must provide complete
keyboard, focus, naming, dismissal, and state behavior. The lint policy
requires React Aria `useHover` instead of raw mouse hover handlers. jsdom tests
are useful for component contracts but do not prove real focus, layout,
contrast, reduced motion, or assistive-technology behavior.

Installation details expose named, determinate progress. Routine progress
updates are not live announcements; one atomic terminal region announces
failure assertively and cancellation or success politely. This installation
surface exists only in the default build.

The renderer follows the operating-system reduced-motion preference. Reduced
mode removes nonessential Popover entry and dismissal translation while
retaining opacity feedback, and bounds CSS animations and transitions to a
near-zero duration and one iteration.

## Commands

From the repository root:

```bash
npm run -w frontend dev
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w frontend build
npm run -w frontend build:library-only
```

The default build includes inference integrations. `build:library-only`
removes their UI.

For representative desktop behavior, build and run through the root launcher.
See [Development](../docs/DEVELOPMENT.md) and the
[current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
