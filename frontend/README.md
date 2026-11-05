# Frontend

## Purpose

The React renderer for the Pumas desktop app. It presents backend-owned library
and runtime state and collects user intent; the backend and the Rust API remain
usable without it.

## Responsibility

- Components own presentation and short-lived interaction drafts.
- Hooks own async work, cancellation, cleanup, stale-invocation protection and
  renderer subscriptions.
- `src/api/` adapts the sandboxed `window.electronAPI` preload bridge. The
  renderer has no filesystem, process or network access of its own.

The Rust backend owns model availability, categories, download state, repair
outcomes, runtime installations, runtime profiles, serving instances and status
telemetry. The frontend never decides durable model status.

Stack: React 19, TypeScript, Vite 6, Tailwind CSS 4 with semantic tokens,
Framer Motion, React Aria interactions, and Vitest with Testing Library/jsdom.

## Invariants

- Values from Electron, the backend or browser persistence are untrusted until
  decoded. Requests and responses use the generated desktop contract
  (`src/generated/desktop-contract*`, produced from the Rust export; see
  [Electron](../electron/README.md#generated-catalog-and-download-contract)).
  TypeScript types alone are not validation.
- A malformed or failed read leaves the last valid state in place and reports
  the failure; it never becomes empty data or a default.
- Mutations (install, remove, switch, launch, stop, setup, conversion, saves)
  are sent once and never retried automatically. A malformed or lost response
  is an unconfirmed outcome, not proof that no effect happened; the UI says so
  and lets the user re-read before acting again.
- A success flag means only what the producer reports: an accepted start or
  cancellation request, a completed selection write, or a launch whose bounded
  readiness check finished. It does not prove that work finished, a process is
  healthy, or a runtime or dependency is ready.
- Responses are matched to the model or selection that requested them; a reply
  for another identity cannot replace the current view.
- Cached startup data is a projection whose age, provenance, refresh state and
  degraded outcome stay visible. The library snapshot stores when it was
  written and a per-library revision. Its rows stay display-only until the
  first load succeeds; later refreshes keep live rows actionable and downgrade
  them only when the refresh fails.
- A failed library load, scan or search, download-list read or runtime-state
  refresh keeps its reason on screen. Disk usage that was never read shows as
  unavailable. None of these fall back to an empty list, an idle status or a
  0% reading.
- Do not infer that a model is complete from its name, repository or a partial
  set of files. One Hugging Face repository can hold several files or
  quantizations, and the same weights in another repository are a separate
  model.
- A model's category is backend metadata. The **Used as** choice in the
  metadata window sends `set_model_category`; the backend keeps that choice
  across re-detection. Rows show the inputs/outputs the model handles and mark
  adapters from the backend's metadata.

## Accessibility and Themes

Theme tokens live in `src/index.css` as CSS custom properties. Use semantic
tokens rather than hard-coded colors.

Prefer native semantic controls. Composite widgets must provide complete
keyboard, focus, naming, dismissal and state behavior. The lint policy requires
React Aria `useHover` instead of raw mouse hover handlers. `ui/Tooltip` opens
on hover and on keyboard focus, closes on Escape, and describes its trigger
through `aria-describedby`, so a hint on a control is never hover-only.
Progress rings are decorative; the control that owns one states its progress
as text. Routine progress updates are not live announcements; one terminal
region announces failure assertively and cancellation or success politely.
The renderer follows the operating-system reduced-motion preference. jsdom
tests do not prove real focus, layout, contrast, reduced motion or
assistive-technology behavior.

## Entry Points

From the repository root:

```bash
npm run -w frontend dev
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w frontend test:library-only
npm run -w frontend build
npm run -w frontend build:library-only
pnpm --dir frontend test:desktop-contract
```

The default build includes inference integrations; `build:library-only` removes
their UI. `test:library-only` runs `*.library-only.test.tsx` with the same
compiled-out feature flag and module replacements as that build; `test:run`
does not include them. `test:desktop-contract` crosses the real bundled preload and renderer;
[Development](../docs/DEVELOPMENT.md) describes its prerequisites. For
representative desktop behavior, build and run through the root launcher.
Open frontend findings are tracked in the
[current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
