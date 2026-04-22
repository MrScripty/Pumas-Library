# Rust Crates Workspace

## Purpose
This directory contains the Rust workspace members that provide Pumas Library's core domain logic, desktop RPC server, application-version management, and host-language binding surfaces.

## Crate Role Map
| Crate | Role | Primary Consumers | Boundary |
| --- | --- | --- | --- |
| `pumas-library` | Headless domain library for models, metadata, indexing, process management, and filesystem-backed state. | `pumas-rpc`, `pumas-app-manager`, `pumas-uniffi`, `pumas-rustler`, Rust tests. | Owns durable data and domain invariants. |
| `pumas-app-manager` | Application version and plugin-adjacent service clients for ComfyUI, Ollama, and Torch. | `pumas-rpc`, integration tests. | Owns external app lifecycle coordination, not model-library storage. |
| `pumas-rpc` | JSON-RPC server process used by Electron. | Electron main process, launcher smoke checks. | Owns network ingress, request parsing, and trust-boundary validation. |
| `pumas-uniffi` | UniFFI cdylib and binding generator entrypoint. | C#, future host languages. | Owns supported FFI DTOs and compatibility tiers. |
| `pumas_rustler` | Rustler cdylib for BEAM hosts. | Elixir/Erlang hosts. | Owns NIF-safe wrappers around core operations. |

## Producer Contract
Workspace crates must expose stable boundaries through crate APIs, command binaries, or generated binding artifacts. Shared dependency versions belong in `rust/Cargo.toml`; crate-specific tools or features belong in the owning crate manifest.

Workspace crates inherit the lint baseline from `rust/Cargo.toml`. Unsafe
operations inside unsafe functions are denied across the workspace. Existing
standalone unsafe blocks remain allowed while OS/FFI modules are isolated and
documented with `SAFETY:` comments in later refactor slices.

## Consumer Contract
Consumers should depend on the narrowest crate that owns the operation they need. New host surfaces should not bypass `pumas-rpc`, `pumas-uniffi`, or `pumas_rustler` boundary validation.

## Non-Goals
None. Reason: this directory is the top-level Rust ownership map. Revisit trigger: add a section if a new crate type, such as benchmarks-only or generated-only, is introduced.
