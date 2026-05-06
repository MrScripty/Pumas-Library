# Pumas API Caller Inventory

## Purpose

Record the Milestone 1 inventory of current `PumasApi` construction and
documentation references before source behavior is split into explicit
instance, local-client, and read-only roles.

## Source Caller Classification

| Path | Current Call | Future Role | Rationale |
| ---- | ------------ | ----------- | --------- |
| `rust/crates/pumas-rpc/src/main.rs:95` | `PumasApi::builder(&launcher_root).auto_create_dirs(true).build()` | `PumasLibraryInstance` owner | The desktop sidecar owns the launcher root, starts backend lifecycle, serves RPC, and should publish local service endpoints explicitly. |
| `rust/crates/pumas-rpc/src/server.rs:202` | `PumasApi::new(&launcher_root)` | Test-only owner | Server startup test constructs a backend API for Axum server tests. Future tests should use explicit owner helpers. |
| `rust/crates/pumas-uniffi/src/bindings.rs:333` | `PumasApi::new(&launcher_root)` after eager IPC client attempt | Transitional FFI owner/local-client split | UniFFI currently exposes explicit `FfiApiInner::{Primary, Client}` internally but hides that behind `FfiPumasApi`. Future bindings need explicit role constructors or names. |
| `rust/crates/pumas-uniffi/src/bindings.rs:349` | `PumasApi::builder(&launcher_root).build()` after eager IPC client attempt | Transitional FFI owner/local-client split | Configured construction can own the instance or attach as a client today. Future surface should make that explicit across language bindings. |
| `rust/crates/pumas-core/src/lib.rs:232` | `PumasApiBuilder::new(launcher_root)` | Legacy facade constructor | Public builder entry point currently owns or attaches. Future role-specific builders should replace hidden attachment. |
| `rust/crates/pumas-core/src/tests.rs:118-150,172,236,265,289` | `PumasApi::new` / `PumasApi::builder` | Test-only owner | Unit tests create isolated temp launcher roots and should migrate to owner test helpers when explicit roles land. |
| `rust/crates/pumas-core/src/tests.rs:161-185` | second `PumasApi::new` and `PumasApi::discover` returning clients | Test-only local-client behavior | These tests directly encode the superseded transparent client contract and must be replaced by explicit `PumasLocalClient` tests. |
| `rust/crates/pumas-core/tests/api_tests.rs:81-831` | repeated `PumasApi::builder(...).build()` | Test-only owner | Integration tests use isolated temp roots for API behavior. They should migrate to explicit owner construction after role split. |
| `rust/crates/pumas-core/examples/basic_usage.rs:14` | `PumasApi::builder(&path)` | Example read-only consumer | Example lists models and should migrate to `PumasReadOnlyLibrary` or a direct selector snapshot once available. |
| `rust/crates/pumas-core/examples/search_models.rs:17` | `PumasApi::builder(path)` | Example read/query consumer | Search example likely fits `PumasReadOnlyLibrary` after selector snapshot exists. |
| `rust/crates/pumas-core/examples/reconcile_library_state.rs:35` | `PumasApi::builder(&launcher_root)` | Example owner | Reconciliation mutates/repairs library state and must remain owner-only. |
| `rust/crates/pumas-core/src/api/builder.rs:341-349` | occupied root returns IPC-backed `PumasApi` | Legacy local-client fallback | Builder currently mixes owner construction with same-device client attachment. This is the main source behavior to split later. |
| `rust/crates/pumas-core/src/api/builder.rs:587-599` | `PumasApi { inner: ApiInner::Primary(...) }` | Owner construction internals | This is the primary construction path that should become the backing implementation for `PumasLibraryInstance`. |

## Documentation References Needing Migration

| Path | Current Reference | Future Role | Rationale |
| ---- | ----------------- | ----------- | --------- |
| `docs/architecture/SYSTEM_ARCHITECTURE.md:49` | `PumasApi::new()` converges automatically to primary or client | Architecture migration required | This describes current legacy behavior but is no longer the target contract. |
| `README.md:50,126,138` | top-level transparent-mode and `PumasApi` quickstart references | Public docs migration required | Top-level docs promoted hidden primary/client convergence to new consumers. Updated to frame it as transitional compatibility. |
| `rust/crates/pumas-core/README.md:19,35` | crate quickstart and builder options | Crate docs migration required | The crate README now needs to steer new integrations toward explicit future roles while preserving current examples. |
| `rust/crates/pumas-core/src/README.md:49,86` | `PumasApi` as stable host-facing facade | Architecture migration required | Future docs must distinguish legacy facade from explicit owner/client/read-only roles. |
| `rust/crates/pumas-core/src/api/README.md:88` | callers use `PumasApi` regardless of primary/client mode | Architecture migration required | This directly contradicts the explicit-role plan. |
| `rust/crates/pumas-core/src/lib.rs:97-103,242` | transparent mode and client-backed constructor docs | Source doc migration required | Rustdoc currently advertises hidden transport behavior. |
| `rust/crates/pumas-core/src/ipc/mod.rs` and `rust/crates/pumas-core/src/ipc/README.md` | transparent instance convergence terminology | Transport doc migration required | IPC should be documented as explicit local-client transport, not direct API semantics. |
| `docs/contracts/native-bindings-surface.md:17-18` | `FfiPumasApi` validates then primary or IPC-backed construction | Binding contract migration required | Binding constructors need explicit role semantics or a documented transitional compatibility note. |
| `rust/crates/pumas-uniffi/src/README.md:79` | `FfiPumasApi` primary or IPC-backed client constructors | Binding docs migration required | Binding README promoted one hidden equivalent constructor surface. Updated to frame this as transitional compatibility. |
| `rust/crates/pumas-core/README.md` and `rust/crates/pumas-core/examples/README.md` | examples use `PumasApi::builder` | Example migration required | Examples should be updated with explicit owner/read-only construction as the new surfaces land. |

## Anti-Patterns Found

- `PumasApi::new()` and `PumasApi::builder(...).build()` currently hide whether
  calls are direct in-process operations or IPC-backed client calls.
- Existing tests intentionally encode the superseded hidden client behavior.
- Architecture docs present transparent convergence as the current stable
  contract, which encourages new consumers to depend on it.
- UniFFI already has an internal primary/client split but exports one
  `FfiPumasApi` object, so foreign-language callers cannot choose topology
  explicitly.
- Generated files under `rust/target/` contain stale `PumasApi` references and
  should be ignored for migration inventory unless bindings are regenerated.

## Milestone 1 Decision

Do not change source behavior in the inventory slice. Treat the current
transparent `PumasApi` convergence as legacy transitional behavior. New work
must target explicit roles:

- `PumasLibraryInstance` for owning lifecycle and writes;
- `PumasReadOnlyLibrary` for direct SQLite snapshot reads with no lifecycle
  ownership;
- `PumasLocalClient` for explicit same-device transport to a running instance.
