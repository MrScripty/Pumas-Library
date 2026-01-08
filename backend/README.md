# Backend Architecture

This document explains the architecture, design decisions, and organization of the ComfyUI Launcher backend.

## Overview

The backend is a Python application that provides:
- Version management for multiple ComfyUI installations
- Process lifecycle management (launch, monitor, terminate)
- Resource management (models, custom nodes, shared storage)
- GitHub integration for fetching releases and metadata
- System resource monitoring (CPU, GPU, RAM, disk)
- Model library with import, download, and mapping capabilities

## Technology Stack

- **Python 3.12+** - Core language
- **PyWebView** - Desktop GUI framework (GTK/WebKit backend on Linux)
- **Type Hints** - Full type coverage with mypy enforcement
- **Structured Logging** - Centralized logging system
- **SQLite** - Model library indexing
- **Git** - Repository cloning and management

## Architecture Patterns

### 1. API Layer Pattern

The backend uses a clean API layer to separate concerns:

```
┌─────────────────────────────────────┐
│  PyWebView Bridge (main.py)         │  ← JavaScript API exposed to frontend
└─────────────────┬───────────────────┘
                  │
┌─────────────────▼───────────────────┐
│  Public API (api/core.py)           │  ← Rate-limited, validated entry points
└─────────────────┬───────────────────┘
                  │
         ┌────────┴────────┐
         │                 │
┌────────▼──────┐  ┌──────▼────────┐
│  Managers      │  │  Resources    │   ← Business logic
│  (version,     │  │  (models,     │
│   metadata,    │  │   custom      │
│   github)      │  │   nodes)      │
└────────────────┘  └───────────────┘
```

**Key Files:**
- [main.py](main.py) - PyWebView JavaScript API bindings
- [api/core.py](api/core.py) - `ComfyUISetupAPI` class with rate limiting and validation
- Manager modules - Business logic implementation

**Design Rationale:**
- **Separation of concerns**: UI bindings don't contain business logic
- **Rate limiting**: Prevents destructive actions from being called too rapidly
- **Input validation**: All external inputs validated at API boundary
- **Testability**: Managers can be tested without PyWebView dependency

### 2. Mixin Composition Pattern

Complex managers like `VersionManager` use mixin-based composition instead of inheritance:

```python
# version_manager_components/
class ConstraintsMixin:      # Handles dependency constraints
class DependenciesMixin:     # Manages virtual environments
class InstallationMixin:     # Orchestrates installation
class LauncherMixin:         # Process launching and health checks
class StateMixin:            # Active/default version state

# Composed into:
class VersionManager(
    ConstraintsMixin,
    DependenciesMixin,
    InstallationMixin,
    LauncherMixin,
    StateMixin
):
    ...
```

**Design Rationale:**
- **Modularity**: Each mixin handles one concern
- **Testability**: Mixins can be tested independently
- **Maintainability**: Changes to one aspect don't affect others
- **Clarity**: Clear separation of responsibilities

See [version_manager_components/](version_manager_components/) for implementation details.

### 3. Metadata Management

All persistent state is stored as JSON with atomic write guarantees:

```
launcher-data/
├── metadata/
│   ├── versions/           # Per-version metadata
│   │   ├── v0.5.1.json
│   │   └── v0.6.0.json
│   ├── github_cache.json   # Cached release data
│   └── size_cache.json     # Cached download sizes
└── active_version.json     # Currently active version
```

**Atomic Write Pattern:**
```python
from backend.file_utils import atomic_write_json

# Atomic write with automatic locking
atomic_write_json(path, data, lock=self._lock)
```

**Design Rationale:**
- **Crash safety**: No corruption from interrupted writes (temp file + rename)
- **Thread safety**: File locking prevents concurrent writes
- **JSON validation**: Schema checked before commit
- **Backup**: Previous version preserved on write

See [file_utils.py](file_utils.py) for implementation.

### 4. Resource Management

Resources (models, custom nodes) are shared across ComfyUI versions using symlinks:

```
shared-resources/
├── models/
│   ├── models.db           # SQLite index
│   ├── checkpoints/
│   ├── loras/
│   └── ...
└── custom_nodes/
    └── <node-repos>/
```

Each ComfyUI version gets symlinks to shared resources:
```
comfyui-versions/v0.6.0/
├── models -> ../../shared-resources/models/checkpoints
├── custom_nodes -> ../../shared-resources/custom_nodes
└── ...
```

**Design Rationale:**
- **Disk efficiency**: Models stored once, used by all versions
- **Consistency**: All versions use the same models
- **Flexibility**: App-specific overrides via mapping configs

See [docs/architecture/MODEL_LIBRARY.md](../docs/architecture/MODEL_LIBRARY.md) for details.

### 5. Error Handling

Custom exception hierarchy provides specific error types:

```python
# backend/exceptions.py
ComfyUILauncherError        # Base exception
├── InstallationError       # Installation failures
├── DependencyError         # Dependency issues
├── NetworkError            # Network operations
├── ValidationError         # Input validation
├── MetadataError           # Metadata corruption
└── ProcessError            # Process management
```

**Usage Pattern:**
```python
try:
    do_something()
except FileNotFoundError as e:
    raise MetadataError(f"Metadata file not found: {e}") from e
except json.JSONDecodeError as e:
    raise MetadataError(f"Invalid metadata format: {e}") from e
```

**Design Rationale:**
- **Specific handling**: Catch specific exceptions, not generic `Exception`
- **Cause chaining**: Preserve original error context with `from e`
- **Logging required**: Every `except` block must log (unless re-raising only)
- **Type safety**: Custom exceptions carry relevant context (URLs, paths, etc.)

See [CONTRIBUTING.md](../CONTRIBUTING.md) for exception handling standards.

### 6. Network Operations

All network operations use retry logic with exponential backoff:

```python
from backend.retry_utils import retry_operation

success = retry_operation(
    operation=lambda: download_file(url, dest),
    max_retries=3,
    operation_name="download ComfyUI release"
)
```

**Backoff Strategy:**
- Retry delays: 2s, 4s, 8s (exponential with jitter)
- Automatic logging of retry attempts
- Configurable max delay and retries

**Design Rationale:**
- **Reliability**: Transient network failures don't fail operations
- **User experience**: Automatic recovery without manual intervention
- **Observability**: All retry attempts logged for debugging

See [retry_utils.py](retry_utils.py) for implementation.

### 7. Progress Tracking

Long-running operations report progress via callbacks:

```python
def install_version(
    tag: str,
    progress_callback: Optional[Callable[[str, float], None]] = None
) -> bool:
    progress_callback("Cloning repository", 0.1)
    # ... work ...
    progress_callback("Installing dependencies", 0.5)
    # ... work ...
    progress_callback("Complete", 1.0)
```

**Design Rationale:**
- **Responsiveness**: UI shows real-time progress
- **Cancellation**: Callbacks can check cancellation flags
- **Flexibility**: Same operation usable from UI or CLI

See [installation_progress_tracker.py](installation_progress_tracker.py) for weighted progress implementation.

## Module Organization

### Core Modules

| Module | Purpose | Key Classes |
|--------|---------|-------------|
| [main.py](main.py) | PyWebView JavaScript API | `JavaScriptAPI` |
| [api/core.py](api/core.py) | Public API layer | `ComfyUISetupAPI` |
| [version_manager.py](version_manager.py) | Version orchestration | `VersionManager` |
| [metadata_manager.py](metadata_manager.py) | Metadata persistence | `MetadataManager` |
| [github_integration.py](github_integration.py) | GitHub API client | `GitHubReleasesFetcher`, `DownloadManager` |

### Resource Management

| Module | Purpose |
|--------|---------|
| [resources/resource_manager.py](resources/resource_manager.py) | Model and custom node management |
| [model_library/library.py](model_library/library.py) | Model library indexing |
| [model_library/downloader.py](model_library/downloader.py) | Model downloads from HuggingFace |
| [model_library/importer.py](model_library/importer.py) | Local model imports |
| [model_library/mapper.py](model_library/mapper.py) | App-specific model mapping |

### Utilities

| Module | Purpose |
|--------|---------|
| [logging_config.py](logging_config.py) | Centralized logging setup |
| [exceptions.py](exceptions.py) | Custom exception hierarchy |
| [validators.py](validators.py) | Input validation functions |
| [file_utils.py](file_utils.py) | Atomic file operations |
| [retry_utils.py](retry_utils.py) | Network retry logic |
| [config.py](config.py) | Configuration constants |

### Process Management

| Module | Purpose |
|--------|---------|
| [api/process_manager.py](api/process_manager.py) | Process lifecycle management |
| [api/process_resource_tracker.py](api/process_resource_tracker.py) | CPU/GPU/RAM monitoring |
| [api/system_utils.py](api/system_utils.py) | System information utilities |

## Configuration

All configuration lives in [config.py](config.py):

```python
class AppConfig:
    GITHUB_REPO = "comfyanonymous/ComfyUI"
    GITHUB_API_BASE = "https://api.github.com"

class NetworkConfig:
    MAX_RETRIES = 3
    TIMEOUT_SECONDS = 30

class PathsConfig:
    LAUNCHER_ROOT = Path.cwd()
    VERSIONS_DIR = LAUNCHER_ROOT / "comfyui-versions"
    SHARED_RESOURCES = LAUNCHER_ROOT / "shared-resources"
```

**Design Rationale:**
- **No hardcoded values**: All constants in config.py
- **Type safety**: Dataclass-based configuration
- **Centralization**: Single source of truth

## Logging System

Centralized logging with structured output:

```python
from backend.logging_config import get_logger

logger = get_logger(__name__)

logger.debug("Detailed diagnostic info")
logger.info("Informational message")
logger.warning("Warning message")
logger.error("Error occurred", exc_info=True)
```

**Log Levels:**
- `DEBUG`: Detailed diagnostic information
- `INFO`: General informational messages
- `WARNING`: Recoverable issues
- `ERROR`: Failures requiring attention

**Design Rationale:**
- **No print() statements**: Enforced via pre-commit hooks
- **Structured output**: Consistent log format
- **Flexible routing**: Can log to file, console, or both
- **Exception context**: `exc_info=True` captures stack traces

See [logging_config.py](logging_config.py) for setup details.

## Type Safety

Full type coverage with mypy enforcement:

```python
from typing import Optional, Callable, Protocol
from pathlib import Path

def install_version(
    tag: str,
    progress_callback: Optional[Callable[[str, float], None]] = None
) -> bool:
    """Install a specific ComfyUI version."""
    ...
```

**Type Checking:**
- `mypy` runs on every commit via pre-commit hooks
- All functions have complete type hints
- Protocols define duck-typed interfaces
- `from __future__ import annotations` for deferred evaluation

**Design Rationale:**
- **Catch bugs early**: Type errors caught before runtime
- **Documentation**: Types serve as inline documentation
- **IDE support**: Better autocomplete and refactoring
- **Maintainability**: Easier to understand and modify code

See [CONTRIBUTING.md](../CONTRIBUTING.md) for type safety standards.

## Security

### Input Validation

All external inputs validated before use:

```python
from backend.validators import (
    validate_version_tag,
    validate_url,
    sanitize_path,
    validate_package_name
)

# Version tags: alphanumeric + dash/dot only
if not validate_version_tag(tag):
    raise ValidationError(f"Invalid version tag: {tag}")

# File paths: no .. traversal, must be within base
safe_path = sanitize_path(user_path, base_directory)

# URLs: https:// only
if not validate_url(url):
    raise ValidationError(f"Invalid URL: {url}")
```

**Design Rationale:**
- **Defense in depth**: Validate at API boundary
- **Path traversal prevention**: No `..` in file paths
- **HTTPS enforcement**: Reject insecure protocols
- **Injection prevention**: Sanitize shell inputs

See [validators.py](validators.py) for validation functions.

### Dependency Scanning

Regular security audits:
```bash
pip-audit              # Python dependencies
cd frontend && npm audit  # Node.js dependencies
```

**Pre-release checklist:**
- Run `pip-audit` and fix critical/high severity issues
- Run `npm audit` and update vulnerable packages
- Regenerate SBOM with `./launcher sbom`

## Testing

See [docs/TESTING.md](../docs/TESTING.md) for comprehensive testing documentation.

**Test Structure:**
```
tests/
├── unit/              # Fast, isolated unit tests
│   ├── test_metadata_manager.py
│   ├── test_process_manager.py
│   └── ...
├── integration/       # Integration tests with real I/O
└── conftest.py        # Shared fixtures
```

**Current Coverage:** ~27% (target: 80%)

**Testing Philosophy:**
- New files: ≥80% coverage required
- Modified files: maintain or improve coverage
- Mock external I/O (network, subprocess)
- Use real filesystem in temp directories

## Performance Considerations

### Caching Strategy

- **GitHub releases**: Cached with TTL (default: 1 hour)
- **Download sizes**: Cached permanently (immutable)
- **Constraints**: Cached per Python version (stable)

### Async Operations

- **Background refresh**: GitHub cache refreshed in background thread
- **Progress tracking**: Long operations report progress incrementally
- **Resource monitoring**: System stats polled asynchronously

### Disk Usage

- **Shared resources**: Models stored once, symlinked to versions
- **Pip cache**: Centralized pip cache shared across versions
- **Log rotation**: Logs rotated to prevent unbounded growth

## Future Enhancements

See planning documents in [docs/architecture/](../docs/architecture/):
- [MODEL_LIBRARY.md](../docs/architecture/MODEL_LIBRARY.md) - Model management roadmap
- [MULTI_APP_SYSTEM.md](../docs/architecture/MULTI_APP_SYSTEM.md) - Multi-application support
- [FRONTEND_ARCHITECTURE.md](../docs/architecture/FRONTEND_ARCHITECTURE.md) - Frontend integration

## Related Documentation

- [CONTRIBUTING.md](../CONTRIBUTING.md) - Development standards and guidelines
- [docs/TESTING.md](../docs/TESTING.md) - Testing guide
- [docs/SECURITY.md](../docs/SECURITY.md) - Security practices
- [docs/CODING_STANDARDS.md](../docs/CODING_STANDARDS.md) - Code style standards
