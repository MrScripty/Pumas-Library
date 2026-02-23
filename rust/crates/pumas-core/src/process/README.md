# Process

## Purpose

Process lifecycle management for ComfyUI, Ollama, and other managed applications. Handles
detection of running processes (via PID files and process table scans), launching new
instances with log capture and health checks, and stopping/terminating process trees.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `detection.rs` | `ProcessDetector` - Finds running processes via PID files (primary) and process table scan (fallback) |
| `launcher.rs` | `ProcessLauncher` / `LaunchConfig` - Spawns detached processes with stdout/stderr capture and health polling |
| `manager.rs` | `ProcessManager` - High-level orchestrator combining detection, launching, stopping, and resource tracking |

## Design Decisions

- **Dual detection strategy**: PID files are checked first (most reliable, created at launch),
  with process table scanning as a fallback for externally started instances. This covers both
  managed and pre-existing processes.
- **Detached process spawning**: Processes are launched in their own process group
  (`setsid` on Unix, `CREATE_NEW_PROCESS_GROUP` on Windows) so they survive launcher restarts.

## Dependencies

### Internal
- `crate::platform` - Cross-platform process termination and cmdline scanning
- `crate::system` - `ResourceTracker` for per-process CPU/RAM/GPU monitoring
- `crate::error` - `PumasError` / `Result`

### External
- `sysinfo` - Process table access
