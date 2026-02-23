# System

## Purpose

System-level utilities for hardware monitoring and environment checks. Provides GPU
monitoring (NVIDIA via nvidia-smi), system and per-process resource tracking (CPU, RAM,
disk), and utility functions for opening paths in file managers, URLs in browsers, and
checking for required system tools.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `gpu.rs` | `GpuMonitor` / `NvidiaSmiMonitor` - GPU utilization, memory, and temperature via nvidia-smi |
| `resources.rs` | `ResourceTracker` - System-wide and per-process CPU, RAM, GPU snapshots with polling interval |
| `utils.rs` | `SystemUtils` - Disk space, file manager, URL opening; `check_git`, `check_brave`, `check_setproctitle` |

## Design Decisions

- **nvidia-smi parsing over NVML bindings**: Parsing nvidia-smi CSV output avoids a native
  library dependency and works across all NVIDIA driver versions without version-specific
  linking.
- **Polling-based resource tracking**: `ResourceTracker` caches snapshots with a configurable
  interval to avoid expensive per-request system calls, especially for GPU queries.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `sysinfo` - CPU, RAM, and process-level resource monitoring
- `std::process::Command` - nvidia-smi subprocess for GPU data
