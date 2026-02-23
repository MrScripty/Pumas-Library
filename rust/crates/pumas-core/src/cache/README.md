# Cache

## Purpose

Unified SQLite-backed caching layer with TTL expiration and namespace isolation. Used by
multiple subsystems (HuggingFace search, GitHub releases, plugin configuration) to store
and retrieve cached data through a single shared database.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `traits.rs` | `CacheBackend` trait, `CacheConfig`, `CacheEntry`, `CacheMeta`, `CacheStats` types |
| `sqlite.rs` | `SqliteCache` - Concrete implementation using SQLite with `Arc<Mutex<Connection>>` |

## Design Decisions

- **Namespace-based isolation**: A single SQLite database serves all cache consumers. Each
  subsystem uses a unique namespace string, avoiding the overhead of separate database files.
- **LRU eviction**: When `max_size_bytes` is reached and `enable_eviction` is enabled, the
  least-recently-accessed entries are evicted first.
- **Trait abstraction**: The `CacheBackend` trait allows swapping the SQLite implementation
  for an in-memory backend in tests without changing consumer code.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `rusqlite` - SQLite database access
- `chrono` - TTL expiration timestamps
