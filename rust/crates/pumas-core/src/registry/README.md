# Registry Module

Global SQLite registry for cross-process library path discovery and instance coordination.

## Purpose

When pumas-core is embedded in multiple host applications, each needs to find registered
library roots without user intervention. This module provides a shared SQLite database
at the platform config directory that stores:

- **Library entries**: registered library paths with metadata
- **Instance entries**: primary claim or ready-instance rows for each library

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

**Strict primary claim**: Primary startup claims ownership in the registry before
starting watcher, reconciliation, or IPC-owned background work. For a given
launcher root, only one live process can hold that claim at a time.

**Ready-after-IPC promotion**: Claim rows start as `status='claiming'` with
`port=0`. The winning process starts IPC first, then promotes the row to
`status='ready'` with the assigned port. Clients only attach to ready rows.

**Crash recovery**: If a claimed or ready instance row belongs to a dead PID,
the next starter can replace it. Live claiming rows are treated as startup in
progress and should be awaited by constructors and wrapper layers rather than
overwritten.

## Files

- `mod.rs` - Module exports
- `library_registry.rs` - `LibraryRegistry` with all CRUD operations and tests

## Concurrency

- `PRAGMA journal_mode=WAL` for concurrent readers + serialized writers
- `PRAGMA busy_timeout=5000` for cross-process contention
- `Arc<Mutex<Connection>>` for thread safety within a process
- Claim lifecycle:
  - insert or replace `instances` row as `claiming`
  - start IPC server
  - promote matching claim token to `ready`
  - unregister the row on primary shutdown
