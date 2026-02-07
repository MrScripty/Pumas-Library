# Registry Module

Global SQLite registry for cross-process library path discovery and instance coordination.

## Purpose

When pumas-core is embedded in multiple host applications, each needs to find registered
library roots without user intervention. This module provides a shared SQLite database
at the platform config directory that stores:

- **Library entries**: registered library paths with metadata
- **Instance entries**: running pumas-core instances (PID + TCP port) for each library

## Location

The registry database lives at the platform-standard config directory:

| Platform | Path |
|----------|------|
| Linux | `~/.config/pumas/registry.db` |
| macOS | `~/Library/Application Support/pumas/registry.db` |
| Windows | `%APPDATA%\pumas\registry.db` |

## Design Decisions

**SQLite over daemon**: Using SQLite with WAL mode allows multiple processes to
read concurrently while serializing writes. This avoids the complexity of a
long-running daemon process and leverages the existing `rusqlite` dependency.

**Path canonicalization**: All paths are canonicalized before storage to prevent
duplicate entries from symlinks or relative paths.

**Best-effort registration**: Registry operations never block API initialization.
If the registry is unavailable, pumas-core still works with an explicit path.

## Files

- `mod.rs` - Module exports
- `library_registry.rs` - `LibraryRegistry` with all CRUD operations and tests

## Concurrency

- `PRAGMA journal_mode=WAL` for concurrent readers + serialized writers
- `PRAGMA busy_timeout=5000` for cross-process contention
- `Arc<Mutex<Connection>>` for thread safety within a process
