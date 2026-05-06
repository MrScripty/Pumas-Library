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
- **Policy-free launch helpers**: Provider/profile policy is owned by higher
  runtime services. This module accepts explicit launch config, PID paths,
  environment, and health URLs; it does not decide model routes, provider
  capabilities, or CPU/GPU placement.

## Dependencies

### Internal
- `crate::platform` - Cross-platform process termination and cmdline scanning
- `crate::system` - `ResourceTracker` for per-process CPU/RAM/GPU monitoring
- `crate::error` - `PumasError` / `Result`

## Runtime Profile Boundary

Managed local runtime profiles may use this module for profile-scoped process
spawn/stop mechanics, but profile identity and provider-specific launch
arguments are derived before this boundary. Broad singleton cleanup remains a
legacy app-level behavior and must not be reused for profile-scoped stop
operations.

### External
- `sysinfo` - Process table access
