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
| `TOOLING-STANDARDS.md` | Partially adopted | Launcher test entrypoint, frontend lint/typecheck scripts | Add hooks, CI, Rust workspace lints, and staged artifact validation. |
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

## Revisit Triggers
- Adding or changing an IPC/RPC method.
- Adding externally supplied filesystem paths or URLs.
- Adding a new runtime process, listener, plugin artifact, or binding surface.
- Adding a dependency to a package that does not directly execute it.
- Keeping a source file above the decomposition threshold after touching its owning feature.
