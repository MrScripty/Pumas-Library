# Version Manager

## Purpose

Manages the full lifecycle of application versions: discovery via GitHub releases, installation
with progress tracking, Python virtual environment and dependency management, launching with
health checks, and removal. Supports ComfyUI (Python/git), Ollama (pre-built binary), and
plugin-defined applications.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | `VersionManager` - Top-level coordinator: state queries, install/remove/launch orchestration |
| `state.rs` | `VersionState` - Tracks installed, active, and default versions; validates disk presence |
| `installer.rs` | `VersionInstaller` - Installs versions from GitHub releases with progress and cancellation |
| `dependencies.rs` | `DependencyManager` - Python dependency checking and installation via pip/uv |
| `launcher.rs` | `VersionLauncher` - Process launching with health checks and log capture |
| `progress.rs` | `InstallationProgressTracker` - Real-time progress updates via `mpsc` channels |
| `constraints.rs` | `ConstraintsManager` - PyPI constraint resolution for reproducible installs |
| `ollama.rs` | `OllamaVersionManager` - Ollama-specific binary download and installation |
| `size_calculator.rs` | `SizeCalculator` - Release size estimation using bundled package sizes and PyPI HEAD requests |

## Design Decisions

- **Cancellation via `AtomicBool`**: Cooperative cancellation flag checked at each installation
  phase, avoiding forced process termination.
- **Install lock with `tokio::sync::Mutex`**: Serializes installations to prevent concurrent
  installs of different versions competing for disk I/O.
- **Progress cleanup delay**: A 5-second delay after completion allows the frontend to poll the
  final status before the tracker state is cleared.
- **Stale entry validation**: On startup, installed versions are validated against disk to remove
  entries for directories that no longer exist.

## Dependencies

### Internal
- `pumas_library::config` - `AppId`, `PathsConfig` for app-specific paths
- `pumas_library::metadata` - `MetadataManager` for JSON persistence of version metadata
- `pumas_library::network` - `GitHubClient` for fetching releases
- `pumas_library::models` - Shared DTOs (`InstallationProgress`, `DependencyStatus`)

### External
- `tokio` - Async runtime, channels, locks
- `reqwest` - HTTP downloads
- `chrono` - Date-based constraint resolution
- `tempfile` - Test fixtures
