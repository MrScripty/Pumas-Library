# Platform

## Purpose

Cross-platform abstraction layer centralizing all OS-specific code. All `#[cfg]` blocks for
platform-dependent behavior live here rather than scattered throughout the codebase, making it
straightforward to add support for new platforms. Covers filesystem paths, file permissions,
and process management.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, `current_platform()`, `is_supported_platform()`, re-exports |
| `paths.rs` | Platform-specific directories: config dir, registry DB path, venv Python path, desktop/apps dirs |
| `permissions.rs` | `set_executable` - Sets executable bits on Unix, no-op on Windows |
| `process.rs` | Process utilities: `find_processes_by_cmdline`, `is_process_alive`, `terminate_process_tree` |

## Design Decisions

- **Centralized `#[cfg]` blocks**: Rather than sprinkling `#[cfg(target_os)]` across every module,
  all platform branches are isolated here. Callers use platform-agnostic function signatures.
- **Linux-first, Windows-ready**: Full Linux support is the primary target. Windows paths and
  process management are implemented but macOS remains architecture-ready with pending
  implementation.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `sysinfo` - Process table scanning (used in `process.rs`)
- `dirs` - Standard user directory lookup
