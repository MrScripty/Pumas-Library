# Process (App Manager)

## Purpose

Trait-based abstraction for managing application processes at the app-manager level. Provides
a common interface for launching, stopping, and monitoring different application types, with
a factory that creates the appropriate process manager based on each app's plugin configuration.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `traits.rs` | `AppProcessManager` trait, `ProcessHandle`, `ProcessStatus` - Common interface for all app types |
| `factory.rs` | `ProcessManagerFactory` - Creates app-specific managers based on `InstallationType` from plugin config |

## Design Decisions

- **Trait-based dispatch**: `AppProcessManager` is an `async_trait` object-safe trait, allowing
  the factory to return `Arc<dyn AppProcessManager>` and the caller to be agnostic about
  which concrete implementation is running.
- **Cached managers**: `ProcessManagerFactory` caches created managers in a `RwLock<HashMap>` so
  repeated calls for the same app ID reuse the existing instance rather than recreating it.

## Dependencies

### Internal
- `pumas_library::plugins` - `PluginLoader`, `PluginConfig`, `InstallationType` for app configuration
- `pumas_library::Result` - Error handling

### External
- `async-trait` - Async trait object safety
