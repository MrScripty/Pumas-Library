# Launcher

## Purpose

Launcher self-management: checking for and applying updates via git, and managing patches
to ComfyUI's `main.py` for process identification via `setproctitle`. These patches enable
the process detection system to reliably identify ComfyUI instances.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `updater.rs` | `LauncherUpdater` - Git-based self-update: fetch, compare commits, pull changes |
| `patch.rs` | `PatchManager` - Injects `setproctitle` calls into ComfyUI's `main.py` for process naming |

## Design Decisions

- **Git-based updates**: The launcher updates itself by pulling from its git remote, keeping
  the update mechanism simple and leveraging git's merge/conflict handling. Update checks
  compare local vs remote HEAD commit SHAs.
- **Regex-based patching**: `PatchManager` uses regex to find insertion points in `main.py`,
  making it resilient to minor formatting changes between ComfyUI versions.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`
- `crate::models` - `CommitInfo` for update check results

### External
- `std::process::Command` - Git subprocess execution
- `regex` - Pattern matching for patch insertion points
