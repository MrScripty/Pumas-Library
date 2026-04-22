# Standards Adoption Map

## Purpose
This document maps Pumas Library's local standards work to the shared standards library at `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.

## Adoption Status
| Standard | Project Status | Enforcement | Primary Follow-Up |
| --- | --- | --- | --- |
| `CODING-STANDARDS.md` | Partially adopted | TypeScript strict checks, local file-size script, manual review | Decompose large Rust/frontend modules and restore file-size/complexity ratchets. |
| `DOCUMENTATION-STANDARDS.md` | Partially adopted | Existing module READMEs, audit docs | Add missing source-root READMEs and required contract sections. |
| `ARCHITECTURE-PATTERNS.md` | Partially adopted | Rust crate/module layout, launcher composition | Define explicit crate roles and executable desktop/RPC contracts. |
| `FRONTEND-STANDARDS.md` | Partially adopted | React strict typing, Vitest tests, ESLint | Consolidate polling ownership and split large workflow components. |
| `ACCESSIBILITY-STANDARDS.md` | Partially adopted | `eslint-plugin-jsx-a11y`, React Aria local standard | Replace remaining generic interactive elements or document/test exceptions. |
| `SECURITY-STANDARDS.md` | Partially adopted | Electron sandbox settings, some URL filtering | Add Electron IPC validation and validated path types. |
| `CONCURRENCY-STANDARDS.md` | Partially adopted | Some timer cleanup tests and Rust async isolation | Add task ownership for spawned Rust work and Torch model-manager locking. |
| `CROSS-PLATFORM-STANDARDS.md` | Partially adopted | Platform modules and package targets | Add CI matrix and platform contract documentation. |
| `INTEROP-STANDARDS.md` | Partially adopted | Preload bridge, JSON-RPC, UniFFI/Rustler crates | Replace hand-maintained method drift with a registry and boundary validation. |
| `DEPENDENCY-STANDARDS.md` | Partially adopted | Lockfiles, workspace dependency declarations, package-local TypeScript tool ownership | Continue dependency audits for Rust crates and release tooling. |
| `TOOLING-STANDARDS.md` | Partially adopted | Launcher test entrypoint, frontend lint/typecheck scripts, Rust workspace verification script and CI job | Add hooks, Rust workspace lints, and staged artifact validation. |
| `TESTING-STANDARDS.md` | Partially adopted | Colocated frontend tests, Rust tests, launcher tests | Add cross-layer contract checks and Python/Torch tests. |
| `LANGUAGE-BINDINGS-STANDARDS.md` | Partially adopted | UniFFI/Rustler crates and C# smoke harness | Classify binding surfaces and split wrapper modules by domain. |
| `LAUNCHER-STANDARDS.md` | Mostly adopted | Root `launcher.sh`, JS launcher parser/tests | Clarify dev/release artifact semantics and CI GUI smoke contract. |
| `RELEASE-STANDARDS.md` | Partially adopted | Shared versions, changelog, SBOM files | Add artifact naming/checksum contract and release CI. |
| `PLAN-STANDARDS.md` | Partially adopted | Existing docs/plans and standards audit plan | Keep refactor milestones updated as implementation discovers new layers. |

## Current Exception Policy
Existing broad exceptions are allowed only while they are tracked by the standards refactor audit:

- large files above the 500-line target remain until contract and validation boundaries are extracted;
- frontend `max-lines`, `max-lines-per-function`, and `complexity` lint rules remain disabled until first decomposition milestones land;
- Rust unsafe code remains tolerated until the workspace lint policy can isolate OS/FFI modules;
- polling remains tolerated where no backend event stream exists yet.

## Completed Adoption Steps
- 2026-04-21: TypeScript, Node type, and Electron lint tooling declarations were moved to the workspaces that execute those commands, leaving the root manifest focused on root-owned launcher tests.
- 2026-04-21: Source/support directory README contracts were added for Rust crate roles, RPC and binding boundaries, Torch sidecar ownership, script templates, binding smoke harnesses, and launcher plugin manifests.
- 2026-04-21: Release artifact naming, checksum, SBOM, and native binding compatibility rules were consolidated in `docs/contracts/release-artifacts.md`.
- 2026-04-21: Torch sidecar API validation and app construction were hardened with shared validators, LAN opt-in policy, fresh FastAPI app creation, and focused unit tests.
- 2026-04-21: Torch model-manager slot reservations, unload transitions, and model-limit updates were moved behind a manager-level async lock with concurrency tests.
- 2026-04-22: Rust RPC model import/download and process open handlers now parse renderer payloads into typed command structs at the boundary.
- 2026-04-22: Rust RPC CORS policy was narrowed from wildcard origins, methods, and headers to loopback browser origins, `GET`/`POST`, and `Content-Type`.
- 2026-04-22: Rust RPC server startup now returns an owned server task handle with explicit shutdown instead of discarding the spawned Axum task.
- 2026-04-22: UniFFI exports were classified by support tier, and launcher-root, import, and download request strings now validate at the binding boundary.
- 2026-04-22: Renderer bridge access was centralized through `frontend/src/api/adapter.ts`, including Electron-only window and dropped-file utilities, with browser fallback tests.
- 2026-04-22: Frontend ESLint scripts now rely on flat-config file globs instead of legacy `--ext` flags.
- 2026-04-22: The ComfyUI shell template now honors `TMPDIR` for temporary browser profiles instead of hard-coding `/tmp`.
- 2026-04-22: Release CI now follows the documented Rustler exclusion by building/testing the Rust workspace without `pumas_rustler` and publishing only the supported UniFFI native artifacts.
- 2026-04-22: Frontend destructive and state-changing confirmations moved from `window.confirm` to an app-owned accessible alert dialog.
- 2026-04-22: Launcher app icons now keep app selection and launch/stop indicators as sibling native buttons instead of nested custom role-button controls.
- 2026-04-22: The model metadata modal now uses a named dialog, native backdrop button, Escape close handling, and focus restoration.
- 2026-04-22: Rust workspace verification was centralized in `scripts/rust/check.sh` and wired into CI for fmt, check, clippy, tests, doc tests, and no-default-feature checks.
- 2026-04-22: Rust crates now inherit a workspace lint baseline that denies unsafe operations hidden inside unsafe functions while broader unsafe isolation remains tracked.
- 2026-04-22: The conflict-resolution modal now uses a named dialog, native backdrop button, Escape close handling, and focus restoration.
- 2026-04-22: The mapping-preview modal now uses a named dialog, native backdrop button, Escape close handling, and focus restoration.
- 2026-04-22: Version selector rows no longer use custom `role="button"` wrappers; switching, default, and shortcut actions are separate native buttons with tests.
- 2026-04-22: The install-version modal now uses a named dialog, native backdrop button, Escape close handling, and focus restoration.
- 2026-04-22: Standards audits now have `scripts/dev/list-audit-files.sh` to enumerate source files while excluding generated bindings, release output, runtime state, dependency installs, and Rust targets while retaining tracked plugin manifests.
- 2026-04-22: The Rust workspace root now has a standards-complete `README.md`, and `scripts/dev/check-readme-coverage.sh` verifies README coverage for audited source/support roots.
- 2026-04-22: The existing pre-commit configuration now includes a commit-msg hook that validates conventional commit subjects against the shared commit standards.
- 2026-04-22: The root `.editorconfig` now matches the standards template across TypeScript, Rust, Python, shell, C#, YAML/JSON, Docker, Make, and Markdown formatting boundaries.
- 2026-04-22: Release checksum generation now fails when no artifacts are staged and excludes `checksums-sha256.txt` from its own digest list.
- 2026-04-22: Import lookup metadata rows now use a named native expand button instead of a custom role-button wrapper.
- 2026-04-22: The Electron workspace now owns a flat ESLint 9 configuration, declares the TypeScript ESLint tooling it executes, runs lint without the legacy `--ext` flag, and verifies linting in the Electron packaging CI job.
- 2026-04-22: Sidebar background deselection no longer depends on a generic JSX keyboard handler; Escape handling is owned by an explicit listener with focused tests.
- 2026-04-22: Electron IPC validation now has package-local Node tests for RPC method allowlisting, dialog option sanitization, and external URL scheme validation.
- 2026-04-22: The desktop RPC method allowlist now lives in `electron/src/rpc-method-registry.ts` with ownership, stability, validation, and deferred schema metadata consumed by Electron IPC validation.
- 2026-04-22: Electron backend bridge timers now use an injectable timer controller with package-local tests for health-check rescheduling, restart backoff replacement, and stop-time cleanup.
- 2026-04-22: Launcher update state moved from `App.tsx` into `frontend/src/hooks/useLauncherUpdates.ts`, with tests for backend metadata ownership, stale-state clearing, URL preference, and API-unavailable no-ops.
- 2026-04-22: Model starring and backend link-exclusion preferences moved from `App.tsx` into `frontend/src/hooks/useModelPreferences.ts`, including rollback tests and stale-load protection for optimistic link changes.
- 2026-04-22: Setup dependency installation state moved from `App.tsx` into `frontend/src/hooks/useDependencyInstaller.ts`, with tests for success, pending state, failure reset, and API-unavailable behavior.
- 2026-04-22: The frontend file-size baseline now ratchets `src/App.tsx` to its reduced 376 effective-line ceiling after the root state-owner extractions.
- 2026-04-22: Model manager filter state moved from `ModelManager.tsx` into `frontend/src/hooks/useModelManagerFilters.ts`, with tests for local filters, remote kind filters, mode switching, and developer search, and the file-size baseline now ratchets `ModelManager.tsx` to 388 effective lines.
- 2026-04-22: Model manager file-picker import dialog state moved into `frontend/src/hooks/useModelImportPicker.ts`, with tests for selected paths, close/reset behavior, completion callback, and API-unavailable no-op behavior; the file-size baseline now ratchets `ModelManager.tsx` to 368 effective lines.
- 2026-04-22: Existing-library chooser pending state moved from `ModelManager.tsx` into `frontend/src/hooks/useExistingLibraryChooser.ts`, with tests for success, pending state, duplicate suppression, and missing-callback no-ops; the file-size baseline now ratchets `ModelManager.tsx` to 361 effective lines.
- 2026-04-22: Hugging Face auth prompt visibility moved from `ModelManager.tsx` into `frontend/src/hooks/useHfAuthPrompt.ts`, with tests for new auth-required errors, repeated existing errors, non-auth errors, and explicit open/close actions; the file-size baseline now ratchets `ModelManager.tsx` to 355 effective lines.
- 2026-04-22: Delayed model-list refresh scheduling after download completion moved from `ModelManager.tsx` into `frontend/src/hooks/useDownloadCompletionRefresh.ts`, with timer cleanup tests; the file-size baseline now ratchets `ModelManager.tsx` to 333 effective lines.
- 2026-04-22: Related-model expansion rendering moved from `LocalModelsList.tsx` into `frontend/src/components/RelatedModelsPanel.tsx`, with loading, error, empty, and URL-opening tests; the file-size baseline now ratchets `LocalModelsList.tsx` to 402 effective lines.
- 2026-04-22: The launcher `--test` action and CI launcher verification now run the Torch sidecar Python unit suite through platform-specific Python module commands.
- 2026-04-22: Launcher development and release actions now pass explicit backend binary paths into Electron, so `--run` uses the debug artifact from `--build` while release flows use release artifacts from `--build-release`.
- 2026-04-22: Launcher dependency installation now has injectable plan tests for command checks, workspace install invocation, check/install/recheck behavior, failed installs, and missing runtime dependencies.
- 2026-04-22: Torch sidecar Python now has pinned Ruff development tooling, a root `ruff.toml`, launcher and CI lint/format checks, and formatted Python sources.
- 2026-04-22: Torch sidecar LAN binding now requires both explicit LAN opt-in and an API token, with token middleware enforced for sidecar API routes when configured.
- 2026-04-22: Frontend file-size checks now use a committed baseline ratchet in local scripts and CI so existing oversized files cannot grow while new files keep the 300-line ceiling.
- 2026-04-22: Frontend hook and app-panel section READMEs now document polling ownership, timer guardrails, and event-stream replacement triggers for remaining polling paths.
- 2026-04-22: Root CI now checks workspace dependency ownership so TypeScript, ESLint, Vite, Vitest, Electron, and related tooling stay declared by the packages that execute them.
- 2026-04-22: Release-facing version alignment is now checked locally and in CI across root, frontend, Electron, and Rust workspace manifests.

## Revisit Triggers
- Adding or changing an IPC/RPC method.
- Adding externally supplied filesystem paths or URLs.
- Adding a new runtime process, listener, plugin artifact, or binding surface.
- Adding a dependency to a package that does not directly execute it.
- Keeping a source file above the decomposition threshold after touching its owning feature.
