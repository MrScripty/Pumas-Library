# Pumas-Library: Python to Rust Port Plan

## Overview

Port the Python backend (~106 files, 70+ API methods) to Rust while preserving the React/Electron frontend unchanged. The Rust implementation will provide both:
1. **Headless library crate** (`pumas-core`) - usable programmatically in other Rust applications
2. **JSON-RPC server binary** (`pumas-rpc`) - drop-in replacement for the Python backend

**Key Constraint:** The Python backend remains fully functional throughout development. Both backends coexist and can be switched via environment variable.

## Current Architecture

```
React Frontend → Electron IPC → HTTP JSON-RPC → Python Backend
                                    ↓
                              ComfyUISetupAPI (70+ methods)
                                    ↓
                    ┌───────────────┼───────────────┐
                    ↓               ↓               ↓
            VersionManager    ModelLibrary    SystemUtils
```

**Key Integration Points:**
- JSON-RPC 2.0 over HTTP on `127.0.0.1:{port}`
- Endpoint: `POST /rpc` for all API calls, `GET /health` for health check
- Port printed to stdout as `RPC_PORT={port}` for Electron to capture
- Response wrapping via `wrap_response()` function (see `backend/rpc_server.py`)

---

## Project Structure (Coexisting with Python)

The Rust crates live **inside the existing repository** alongside Python:

```
Pumas-Library/                          # Existing repo root
├── backend/                            # UNCHANGED - Python backend
│   ├── api/
│   ├── model_library/
│   ├── rpc_server.py
│   └── ...
├── frontend/                           # UNCHANGED - React frontend
├── electron/                           # MODIFIED - Add backend selection
│   └── src/
│       └── python-bridge.ts            # Updated to support Rust backend
├── rust/                               # NEW - Rust workspace
│   ├── Cargo.toml                      # Workspace manifest
│   ├── target/                         # Build output (gitignored)
│   └── crates/
│       ├── pumas-core/                 # Headless library crate
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs
│       │       ├── config.rs
│       │       ├── error.rs
│       │       ├── models/
│       │       ├── metadata/
│       │       ├── version_manager/
│       │       ├── model_library/
│       │       ├── github/
│       │       ├── process/
│       │       ├── system/
│       │       └── network/
│       └── pumas-rpc/                  # JSON-RPC server binary
│           ├── Cargo.toml
│           └── src/
│               ├── main.rs
│               ├── server.rs
│               ├── handler.rs
│               └── wrapper.rs
├── launcher                            # UNCHANGED - Bash launcher
└── launcher-data/                      # UNCHANGED - App data
```

### Backend Selection in Electron

The `electron/src/python-bridge.ts` will be updated to support both backends:

```typescript
// Environment variable switches between Python and Rust
const USE_RUST_BACKEND = process.env.PUMAS_RUST_BACKEND === '1';

function getBackendCommand(): { cmd: string; args: string[] } {
  if (USE_RUST_BACKEND) {
    const rustBinary = path.join(projectRoot, 'rust', 'target', 'release', 'pumas-rpc');
    return { cmd: rustBinary, args: ['--port', '0'] };
  } else {
    return { cmd: pythonPath, args: ['rpc_server.py', '--port', '0'] };
  }
}
```

This means:
- **Default behavior:** Python backend (current behavior, no changes needed)
- **Opt-in Rust:** Set `PUMAS_RUST_BACKEND=1` to use Rust backend
- **Both coexist:** You can switch at any time during development

---

## Technology Stack

| Purpose | Crate | Rationale |
|---------|-------|-----------|
| Async runtime | `tokio` | Industry standard, excellent ecosystem |
| HTTP server | `axum` | Modern, type-safe, Tokio-native |
| HTTP client | `reqwest` | Full-featured, streaming support |
| SQLite + FTS5 | `rusqlite` (bundled, fts5) | Direct SQLite binding with FTS5 |
| Serialization | `serde`, `serde_json` | De-facto standard |
| Error handling | `thiserror`, `anyhow` | Ergonomic error types |
| System info | `sysinfo` | Cross-platform CPU/RAM/disk |
| GPU monitoring | `nvml-wrapper` | NVIDIA GPU stats |
| Process mgmt | `nix` (Unix), `windows-sys` | Platform-specific process control |
| File locking | `fs2` | Cross-platform file locks |
| Hashing | `sha2`, `blake3` | SHA256 and BLAKE3 hashes |
| TTL cache | `mini-moka` | High-performance caching |
| Archive | `zip`, `tar`, `flate2` | Extract releases |
| Logging | `tracing` | Structured logging |

---

## API Compatibility Layer

### Response Wrapping

The frontend expects responses in specific formats. The `wrap_response()` function in `backend/rpc_server.py:41-189` must be replicated exactly:

```rust
// pumas-rpc/src/wrapper.rs
pub fn wrap_response(method: &str, result: Value) -> Value {
    match method {
        // List wrappers: {success: true, versions/nodes/etc: [...]}
        "get_available_versions" | "get_installed_versions" =>
            json!({"success": true, "versions": result}),
        "get_custom_nodes" =>
            json!({"success": true, "nodes": result}),

        // Bool methods: {success: bool}
        "install_version" | "remove_version" | "switch_version" =>
            json!({"success": result.as_bool().unwrap_or(false)}),

        // Passthrough methods (already correctly formatted)
        "get_status" | "get_disk_space" | "search_hf_models" => result,

        // ... 60+ other methods
        _ => result,
    }
}
```

### TypeScript Contract

All response types are defined in `frontend/src/types/api.d.ts` (~1130 lines). The Rust models must serialize to match these exactly.

---

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
**Goal:** Core infrastructure, build system, data models

- [ ] Create workspace with `pumas-core` and `pumas-rpc` crates
- [ ] Implement `config.rs` with all constants from `backend/config.py`
- [ ] Implement `error.rs` with error types matching Python exceptions
- [ ] Implement all data models in `models/` matching `backend/models.py`
- [ ] Implement atomic file operations in `metadata/atomic.rs`
- [ ] Set up `tracing` logging system

**Critical files:**
- `backend/config.py` - All configuration constants
- `backend/models.py` - TypedDict definitions → Rust structs

### Phase 2: Metadata & Storage (Week 3-4)
**Goal:** JSON persistence, SQLite with FTS5

- [ ] Implement `MetadataManager` for versions.json, models.json, etc.
- [ ] Implement SQLite model index with FTS5 virtual table
- [ ] Implement `LinkRegistry` for symlink/hardlink tracking
- [ ] Thread-safe access with proper locking

**Critical files:**
- `backend/metadata_manager.py` - JSON persistence patterns
- `backend/model_library/index.py` - SQLite FTS5 schema

### Phase 3: Network Layer (Week 5-6)
**Goal:** HTTP client, GitHub API, caching

- [ ] Implement HTTP client with retry logic and exponential backoff
- [ ] Implement circuit breaker for network resilience
- [ ] Implement GitHub releases fetcher with TTL caching
- [ ] Implement download manager with progress callbacks

**Critical files:**
- `backend/github_integration.py` - GitHub API integration
- `backend/model_library/network/` - Network utilities

### Phase 4: Version Management (Week 7-9)
**Goal:** Core version manager functionality

- [ ] Implement version state tracking (active, installed, default)
- [ ] Implement version installer with progress reporting
- [ ] Implement dependency manager (uv/pip integration)
- [ ] Implement process launcher with health checks
- [ ] Implement installation cancellation
- [ ] Add Ollama version manager variant

**Critical files:**
- `backend/version_manager.py` - Main version manager
- `backend/version_manager_components/` - Mixin modules

### Phase 5: Model Library (Week 10-12)
**Goal:** Model management, HuggingFace integration

- [ ] Implement model library interface
- [ ] Implement model importer with hash verification
- [ ] Implement HuggingFace client (search, download, metadata)
- [ ] Implement model mapper (symlinks/hardlinks)
- [ ] Implement batch import with progress

**Critical files:**
- `backend/model_library/` - All model library modules
- `backend/model_library/hf/` - HuggingFace integration

### Phase 6: System & Process (Week 13-14)
**Goal:** System utilities, process management

- [ ] Implement system resource monitoring (CPU, GPU, RAM, disk)
- [ ] Implement process manager (launch, stop, detect)
- [ ] Implement shortcut manager (desktop/menu shortcuts)
- [ ] Platform-specific implementations (Linux focus, Windows support)

**Critical files:**
- `backend/api/system_utils.py` - System utilities
- `backend/api/process_manager.py` - Process management
- `backend/api/shortcut_manager.py` - Shortcuts

### Phase 7: JSON-RPC Server (Week 15-16)
**Goal:** Complete HTTP server, full API surface

- [ ] Implement Axum HTTP server with JSON-RPC 2.0
- [ ] Implement all 70+ method handlers
- [ ] Implement response wrapping for frontend compatibility
- [ ] Add `/health` endpoint
- [ ] Port output `RPC_PORT={port}` format for Electron

**Critical files:**
- `backend/rpc_server.py` - Reference implementation
- `backend/api/core.py` - All API methods (~90KB)

### Phase 8: Testing & Integration (Week 17-18)
**Goal:** Comprehensive testing, Electron integration

- [ ] Unit tests for all modules
- [ ] Integration tests for API compatibility
- [ ] Contract tests validating TypeScript types
- [ ] Update `electron/src/python-bridge.ts` to support Rust backend
- [ ] Parallel deployment testing (Python vs Rust)

---

## Headless Library API

The `pumas-core` crate exposes `PumasApi` for programmatic use:

```rust
use pumas_core::{PumasApi, Config};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api = PumasApi::new("/path/to/pumas").await?;

    // Version management
    let versions = api.get_available_versions(false, None).await?;
    api.install_version("v0.4.0", None).await?;

    // Model management
    let results = api.search_models_fts("llama", 10, 0).await?;
    api.download_model_from_hf(params).await?;

    // Progress tracking via channels
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    api.install_version_with_progress("v0.4.0", Some(tx)).await?;
    while let Some(progress) = rx.recv().await {
        println!("{:?}", progress);
    }

    Ok(())
}
```

---

## Migration Strategy

### Incremental Port Approach

The Rust backend will be developed incrementally while Python remains the default:

1. **Phase A - Scaffold:** Create Rust workspace, implement basic JSON-RPC server with `/health` endpoint
2. **Phase B - Read-only APIs:** Port status/query methods first (no side effects)
3. **Phase C - Core Actions:** Port version install/remove, model download
4. **Phase D - Full Parity:** Complete all 70+ methods
5. **Phase E - Default Switch:** Make Rust the default, Python as fallback

### Testing Strategy During Migration

```bash
# Run Python backend (default)
./launcher

# Run Rust backend (opt-in)
PUMAS_RUST_BACKEND=1 ./launcher

# Compare responses (development testing)
./scripts/compare-backends.sh  # We'll create this script
```

### API Compatibility Testing

Create a test harness that:
1. Sends identical requests to both backends
2. Compares JSON responses field-by-field
3. Reports any discrepancies

This ensures the Rust backend is a true drop-in replacement.

---

## Verification Plan

1. **Unit Tests:** Each module has unit tests with >80% coverage
2. **Contract Tests:** Validate all responses match TypeScript types in `api.d.ts`
3. **Integration Tests:**
   - Install/remove/switch versions
   - Model download and import
   - FTS5 search functionality
4. **Manual Testing:**
   - Launch Electron app with Rust backend
   - Test all UI workflows end-to-end
   - Verify progress updates work correctly
5. **Regression Testing:** Run existing Python backend tests, ensure Rust produces same results

---

## Key Considerations

### Thread Safety
- Use `Arc<RwLock<T>>` for shared mutable state
- Use `Arc<AtomicBool>` for cancellation flags
- Use channels for progress updates

### Error Handling
- Define comprehensive error types with `thiserror`
- Map errors to JSON-RPC error codes (-32600 to -32603)
- Preserve error messages for frontend display

### Backward Compatibility
- Existing JSON metadata files must remain compatible
- SQLite schema must match Python implementation exactly
- No changes to frontend code required

### Platform Support
- Primary: Linux (Electron target)
- Secondary: Windows, macOS
- Use conditional compilation for platform-specific code
