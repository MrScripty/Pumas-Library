# Standards Refactor Audit - 2026-04-21

## Purpose
This directory records an iterative standards-compliance audit of Pumas Library against `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.

## Scope
The audit covered the checked-in source, manifests, launcher tooling, documentation, and binding surfaces. Runtime/build outputs ignored by git, such as `rust/target/`, `launcher-data/cache/`, and `launcher-data/profiles/`, were treated as generated state and not counted as refactor targets.

## Passes
| Pass | File | Focus |
| --- | --- | --- |
| 1 | `pass-01-global-governance.md` | Repository structure, docs, tooling, dependencies, release governance |
| 2 | `pass-02-frontend-electron.md` | React frontend, Electron IPC, accessibility, frontend testing |
| 3 | `pass-03-rust-backend.md` | Rust architecture, API shape, async, security, bindings |
| 4 | `pass-04-python-launcher.md` | Torch server, launcher scripts, shell/script standards |
| 5 | `pass-05-overlap-refactor-plan.md` | Depth-ordered refactor plan for overlapping findings |
| Deferred | `deferred-issues-register.md` | Bugs, missing tests, and risks not solved by standards refactors alone |

## Summary
The codebase has strong signs of active hardening: strict TypeScript settings, JSX a11y plugin wiring, many colocated tests, Rust crate READMEs, a root launcher contract, and SBOM artifacts. The main compliance gaps are structural rather than isolated syntax issues:

- large modules exceed decomposition triggers by wide margins, especially Rust model-library and API modules;
- IPC/API contracts are duplicated across TypeScript, Electron preload, Rust JSON-RPC dispatch, Rust API state, and binding wrappers without one executable schema owner;
- runtime boundary validation is uneven, especially generic Electron IPC payloads, file paths, Torch server configuration, and RPC method parameters;
- async/background task ownership is incomplete in several Rust and Electron paths;
- docs and automation do not yet enforce the standards library requirements.

The synthesis plan treats this as a four-depth refactor because some files are affected by four overlapping issue classes: contract ownership, boundary validation, decomposition, and verification/tooling.
