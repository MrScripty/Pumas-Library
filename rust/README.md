# Rust Workspace

Cargo workspace for Pumas Library's native domain crates, RPC server, app manager, and host-language binding crates.

## Purpose
This directory owns the Rust build boundary for Pumas Library. It keeps workspace membership, shared dependency versions, lockfile state, lint policy, and release profiles together so native crates can be verified with one Cargo contract.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `Cargo.toml` | Workspace manifest that defines members, default test members, shared package metadata, workspace lints, shared dependency versions, and release profile policy. |
| `Cargo.lock` | Committed application lockfile used by CI and release builds to keep native dependency resolution reproducible. |
| `crates/` | Workspace members for the core library, app manager, RPC server, UniFFI bindings, and Rustler bindings. |
| `target/` | Local Cargo build output. This is generated state and must stay out of standards audits and commits. |

## Problem
The desktop app, bindings, and launcher all depend on native Rust behavior, but those consumers need a single place to discover which crates are supported by default, which crate is responsible for each boundary, and which verification commands apply.

## Constraints
- `pumas_rustler` links through the BEAM runtime, so default Cargo test/check flows exclude it until a host-aware CI job exists.
- `Cargo.lock` is committed because this repository builds an application and release artifacts, not only reusable crates.
- Workspace lint policy must ratchet gradually because legacy OS and FFI unsafe blocks still need isolation.
- Generated build output under `target/` can be large and must never drive source audits.

## Decision
Keep shared Rust ownership at the workspace root and crate-specific ownership under `crates/`. The workspace root defines membership, defaults, shared versions, and broad lint policy; each crate remains responsible for its own README, feature surface, tests, and host-facing contracts.

## Alternatives Rejected
- **Per-crate dependency version ownership only:** Rejected because common dependencies would drift across crates and make release builds harder to reproduce.
- **Including `pumas_rustler` in default workspace checks:** Rejected until CI has an Erlang/OTP environment that can link and exercise the NIF boundary reliably.

## Invariants
- `Cargo.lock` stays committed and changes in the same commit as dependency manifest changes.
- Default workspace verification excludes only crates that require unavailable host runtimes.
- Workspace lints apply to every member crate unless a crate has a documented standards exception.
- `target/` remains generated local state and is excluded from commits and audit file lists.

## Revisit Triggers
- A new Rust crate is added or a crate changes its host-facing support tier.
- A dedicated BEAM-aware Rustler job is added to CI.
- Workspace dependency policy changes, such as moving to `cargo deny` or another duplicate-dependency gate.
- Unsafe isolation completes and the workspace can ratchet additional lint levels.

## Dependencies
### Internal
- `crates/` - Implements all workspace member crates and owns crate-level contracts.
- `../scripts/rust/` - Provides the shared Rust verification command used by local development and CI.
- `../docs/contracts/` - Defines release artifact, native binding, and desktop RPC contracts that Rust crates must satisfy.

### External
- `cargo` - Resolves, builds, tests, lints, and documents the Rust workspace.
- `rustfmt` - Formats Rust code through Cargo.
- `clippy` - Enforces Rust lint checks through Cargo.

## Related ADRs
- `None identified as of 2026-04-22.`
- `Reason: The current workspace shape is documented through standards adoption and contract docs rather than an ADR.`
- `Revisit trigger: Add an ADR before introducing a new native runtime boundary or splitting the workspace into multiple independent release units.`

## Usage Examples
Run the default standards-aligned Rust verification contract:

```bash
../scripts/rust/check.sh
```

Inspect workspace metadata without building dependencies:

```bash
cargo metadata --manifest-path Cargo.toml --no-deps --format-version 1
```

## API Consumer Contract
- `None identified as of 2026-04-22.`
- `Reason: This directory is a Cargo workspace boundary, not a directly imported API surface. Host-facing API contracts live under the owning crates and `docs/contracts/`.`
- `Revisit trigger: Add a root-level API contract if external consumers begin depending on workspace-level generated artifacts or command wrappers from this directory.`

## Structured Producer Contract
- `Cargo.toml` is the stable workspace manifest. Consumers may rely on workspace members, default members, shared package metadata, workspace lints, shared dependency versions, and release profile settings.
- `Cargo.lock` is the reproducible dependency resolution snapshot for application and release builds.
- Changes to workspace members, dependency versions, lint levels, or release profile settings must be committed with any required code, documentation, and lockfile updates.
- `target/` is intentionally volatile generated state. Consumers must not persist references to paths inside it.

## Testing
Use the shared Rust check script for the default CI-equivalent path:

```bash
../scripts/rust/check.sh
```

Crate-specific tests may be run from this directory with `cargo test -p <crate>`, except for host-runtime crates that document separate requirements.
