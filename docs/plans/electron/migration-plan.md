# PyWebView to Electron Migration Plan

**Version**: 1.0
**Last Updated**: 2026-01-11

---

## Executive Summary

This document outlines the migration from **PyWebView + GTK3** to **Electron** for the Pumas Library desktop application. The primary motivation is to eliminate GTK3/X11 legacy issues and adopt a modern, actively-maintained GUI framework with native Wayland support and long-term viability.

### Current State

| Component | Technology | Issues |
|-----------|------------|--------|
| Desktop GUI | PyWebView 5.x | GTK3-based on Linux |
| WebKit Engine | WebKit2GTK 4.x | Limited to GTK3/X11 mode |
| Drag-and-Drop | Custom GTK3 bridge | Freezes, deadlocks (see commit 1b02701) |
| Window Management | GTK3 | X11-specific issues |
| Backend | Python 3.12+ | Will be preserved |
| Frontend | React 19 + Vite | Will be preserved |

### Target State

| Component | Technology | Benefits |
|-----------|------------|----------|
| Desktop GUI | Electron 38+ | Chromium-based, modern features |
| Rendering Engine | Chromium M140+ | Full web platform support |
| Wayland Support | Native (Ozone) | No X11 fallback needed |
| GTK Version | GTK4 (optional) | Modern theming, fractional scaling |
| Backend | Python 3.12+ | Unchanged, IPC bridge |
| Frontend | React 19 + Vite | Unchanged |

---

## Why Electron?

### GTK3 End-of-Life Concerns

GTK3 is in maintenance-only mode. GNOME and the broader Linux ecosystem are transitioning to GTK4, which:
- Has native Wayland support
- Supports fractional scaling
- Has better compositor integration
- Fixes numerous X11-specific rendering bugs

PyWebView on Linux uses **WebKit2GTK**, which is tightly coupled to GTK3. Upgrading PyWebView's rendering backend requires waiting for upstream changes that may never come.

### Electron's GTK4/Wayland Support

Starting with **Electron 38** (released September 2025):
- Default `--ozone-platform=auto` enables native Wayland
- GTK4 theming support via `--gtk-version=4` flag
- Full multi-monitor support on Wayland
- No more X11/XWayland compatibility layer required

### Current Issues Solved by Migration

1. **Drag-and-Drop Freezes**: WebKit2GTK's DND implementation conflicts with GTK3 event handling, causing desktop freezes (see [commit 1b02701](https://github.com/...))

2. **Window Decoration Issues**: PyWebView frameless windows have inconsistent behavior across compositors

3. **Fractional Scaling**: GTK3 + X11 cannot properly handle fractional display scaling common on modern laptops

4. **Input Method Support**: GTK3/WebKit2GTK has poor IME support compared to Chromium

5. **Developer Tooling**: Electron provides full Chromium DevTools, while WebKit2GTK debugging is limited

---

## Electron Version Selection

### Recommended: Electron 38.x (Stable) or 39.x

| Version | Chrome | Node | Stable Release | EOL Date | Status |
|---------|--------|------|----------------|----------|--------|
| **38.x** | M140 | v22.18 | 2025-Sep-02 | 2026-Mar-10 | ✅ Stable, LTS candidate |
| **39.x** | M142 | v22.20 | 2025-Oct-28 | 2026-May-05 | ✅ Stable |
| 40.x | M144 | TBD | 2026-Jan-13 | 2026-Jun-30 | ⚠️ Just released |

**Recommendation**: Start with **Electron 38.x** for maximum stability, with a clear upgrade path to 39.x.

### Electron Support Policy

Electron supports the **latest 3 major versions**. With an 8-week release cadence:
- Each version is supported for ~24 weeks (6 months)
- Security patches are applied to all supported versions
- Breaking changes require 2 major versions notice

For long-term projects, plan for:
- **Version upgrades every 8-16 weeks** (every 1-2 releases)
- Automated dependency updates via Dependabot or Renovate
- Electron's upgrade guide for each major version

---

## Architecture Overview

### Current Architecture (PyWebView)

```
┌─────────────────────────────────────────────────────────────┐
│                     Python Process                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  PyWebView (GTK3 + WebKit2GTK)                      │   │
│  │  ┌───────────────────────────────────────────────┐  │   │
│  │  │  React Frontend (Vite bundle)                  │  │   │
│  │  │  - Direct access to window.pywebview.api      │  │   │
│  │  └───────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Python Backend (ComfyUISetupAPI)                   │   │
│  │  - Model management                                 │   │
│  │  - Version management                               │   │
│  │  - System integration                               │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Target Architecture (Electron + Python Sidecar)

```
┌─────────────────────────────────────────────────────────────┐
│                   Electron Main Process                     │
│  - Window management                                        │
│  - Python sidecar lifecycle                                 │
│  - IPC bridge to renderer                                   │
└─────────────────────┬───────────────────────────────────────┘
                      │ IPC (contextBridge)
┌─────────────────────▼───────────────────────────────────────┐
│                   Electron Renderer Process                 │
│  ┌───────────────────────────────────────────────────────┐ │
│  │  React Frontend (Vite bundle)                          │ │
│  │  - Access via window.electronAPI (same signatures)    │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                      │ JSON-RPC / HTTP / Unix Socket
┌─────────────────────▼───────────────────────────────────────┐
│                   Python Sidecar Process                    │
│  ┌───────────────────────────────────────────────────────┐ │
│  │  Python Backend (ComfyUISetupAPI) - UNCHANGED         │ │
│  │  + JSON-RPC server wrapper                            │ │
│  └───────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Key Architectural Decisions

1. **Python Sidecar**: Keep Python backend as a separate process, managed by Electron
2. **IPC Bridge**: Replace `window.pywebview.api` with `window.electronAPI` using contextBridge
3. **JSON-RPC Communication**: Electron ↔ Python via JSON-RPC over HTTP or Unix domain sockets
4. **Frontend Unchanged**: React components remain identical, only API adapter changes
5. **Type Safety Preserved**: TypeScript interfaces remain the same

---

## Migration Phases

### Phase 0: Preparation (Estimated: 1 week)

**Goal**: Set up Electron infrastructure without breaking existing PyWebView functionality.

#### Tasks

- [ ] **0.1** Create Electron project structure alongside existing code
  - `electron/` directory for Electron-specific code
  - `electron/main.ts` - Main process
  - `electron/preload.ts` - Context bridge
  - `electron/python-bridge.ts` - Python sidecar management

- [ ] **0.2** Install Electron dependencies
  ```json
  {
    "devDependencies": {
      "electron": "^38.0.0",
      "electron-builder": "^24.0.0",
      "@electron-forge/cli": "^7.0.0",
      "@types/electron": "latest"
    }
  }
  ```

- [ ] **0.3** Create Electron configuration files
  - `electron-builder.json` - Packaging configuration
  - `forge.config.js` (optional) - Electron Forge config

- [ ] **0.4** Set up dual-mode launcher script
  - `./launcher` continues to use PyWebView (default)
  - `./launcher --electron` runs Electron mode

- [ ] **0.5** Document new environment setup

#### Deliverables

- Electron can be launched separately from PyWebView
- Both modes can coexist during migration
- No changes to existing PyWebView functionality

---

### Phase 1: Python Sidecar (Estimated: 2 weeks)

**Goal**: Create a JSON-RPC server wrapper for the Python backend.

#### Tasks

- [ ] **1.1** Create JSON-RPC server wrapper
  - `backend/rpc_server.py` - JSON-RPC HTTP server
  - Uses `aiohttp` or `httpx` for async HTTP
  - Exposes all `ComfyUISetupAPI` methods
  - Preserves method signatures exactly

- [ ] **1.2** Create health check endpoint
  - `/health` - Returns server status
  - Used by Electron to verify sidecar is running

- [ ] **1.3** Implement graceful shutdown
  - Handle SIGTERM from Electron
  - Clean up resources before exit
  - Notify frontend of shutdown

- [ ] **1.4** Create sidecar launcher module
  - `electron/python-bridge.ts`
  - Spawns Python process on app start
  - Monitors process health
  - Restarts on crash (configurable)

- [ ] **1.5** Add development mode support
  - Hot-reload Python changes (optional)
  - Console output forwarding
  - Debug port for Python debugger

#### API Wrapper Example

```python
# backend/rpc_server.py
from aiohttp import web
import json

class RPCServer:
    def __init__(self, api: ComfyUISetupAPI):
        self.api = api

    async def handle_rpc(self, request: web.Request) -> web.Response:
        data = await request.json()
        method = data.get("method")
        params = data.get("params", {})

        handler = getattr(self.api, method, None)
        if handler is None:
            return web.json_response({"error": f"Unknown method: {method}"}, status=400)

        try:
            result = handler(**params) if params else handler()
            return web.json_response({"result": result})
        except Exception as e:
            return web.json_response({"error": str(e)}, status=500)
```

#### Deliverables

- Python backend can run standalone as HTTP server
- All existing API methods accessible via JSON-RPC
- Electron can spawn and manage Python process

---

### Phase 2: Electron Main Process (Estimated: 1 week)

**Goal**: Implement Electron main process with window management.

#### Tasks

- [ ] **2.1** Create main process entry point
  - `electron/main.ts`
  - BrowserWindow configuration matching current UI
  - Frameless window with custom title bar

- [ ] **2.2** Implement window configuration
  ```typescript
  const mainWindow = new BrowserWindow({
    width: 1024,  // Match UI.WINDOW_WIDTH
    height: 700,  // Match UI.WINDOW_HEIGHT
    resizable: false,
    frame: false,  // Frameless
    backgroundColor: '#000000',
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    }
  });
  ```

- [ ] **2.3** Handle app lifecycle
  - `app.whenReady()` - Window creation
  - `app.on('window-all-closed')` - Quit app
  - `app.on('before-quit')` - Cleanup Python sidecar

- [ ] **2.4** Implement development mode
  - Load from `http://localhost:3000` (Vite dev server)
  - Enable DevTools
  - Hot-reload support

- [ ] **2.5** Implement production mode
  - Load from `frontend/dist/index.html`
  - Disable DevTools by default
  - Handle `--debug` flag

- [ ] **2.6** Configure Wayland support
  ```typescript
  // In main.ts, before app.whenReady()
  app.commandLine.appendSwitch('ozone-platform', 'auto');
  app.commandLine.appendSwitch('enable-features', 'WaylandWindowDecorations');
  app.commandLine.appendSwitch('gtk-version', '4');
  ```

#### Deliverables

- Electron window renders correctly
- Native Wayland/GTK4 support enabled
- Development and production modes working

---

### Phase 3: Preload & Context Bridge (Estimated: 1 week)

**Goal**: Create secure IPC bridge matching PyWebView API.

#### Tasks

- [ ] **3.1** Create preload script with all API methods
  - `electron/preload.ts`
  - Match `window.pywebview.api` signatures exactly
  - Use `contextBridge.exposeInMainWorld`

- [ ] **3.2** Implement IPC handlers in main process
  - Forward all calls to Python sidecar
  - Handle errors consistently
  - Maintain type safety

- [ ] **3.3** Create API adapter for frontend
  - `frontend/src/api/electron.ts`
  - Same interface as `pywebview.ts`
  - Runtime detection of environment

- [ ] **3.4** Implement file dialog
  - Replace PyWebView's `create_file_dialog`
  - Use Electron's `dialog.showOpenDialog`
  - Match existing file type filters

- [ ] **3.5** Implement drag-and-drop
  - Native Electron drag-and-drop
  - No GTK workarounds needed
  - Full file path access

#### Preload Script Example

```typescript
// electron/preload.ts
import { contextBridge, ipcRenderer } from 'electron';

const api = {
  // Status methods
  getStatus: () => ipcRenderer.invoke('api:getStatus'),
  getDiskSpace: () => ipcRenderer.invoke('api:getDiskSpace'),
  getSystemResources: () => ipcRenderer.invoke('api:getSystemResources'),

  // Action methods
  installDeps: () => ipcRenderer.invoke('api:installDeps'),
  togglePatch: () => ipcRenderer.invoke('api:togglePatch'),
  closeWindow: () => ipcRenderer.invoke('api:closeWindow'),

  // Version management
  getAvailableVersions: (forceRefresh?: boolean) =>
    ipcRenderer.invoke('api:getAvailableVersions', forceRefresh),
  installVersion: (tag: string) =>
    ipcRenderer.invoke('api:installVersion', tag),

  // ... all other methods matching pywebview.d.ts
};

contextBridge.exposeInMainWorld('electronAPI', api);
```

#### Frontend Adapter Example

```typescript
// frontend/src/api/adapter.ts
import type { PyWebViewAPI } from '../types/pywebview';

// Detect runtime environment
const isElectron = typeof window !== 'undefined' &&
                   'electronAPI' in window;

// Export unified API
export const api: PyWebViewAPI = isElectron
  ? (window as any).electronAPI
  : window.pywebview?.api;
```

#### Deliverables

- Frontend works identically in both PyWebView and Electron
- All 60+ API methods bridged correctly
- Drag-and-drop works natively without GTK hacks

---

### Phase 4: Feature Parity (Estimated: 2 weeks)

**Goal**: Ensure all features work correctly in Electron.

#### Tasks

- [ ] **4.1** Test all API methods
  - Create test script that exercises each method
  - Compare responses between PyWebView and Electron
  - Document any behavioral differences

- [ ] **4.2** Implement window controls
  - Close button (matches existing)
  - Minimize/maximize if needed
  - Drag region for frameless window

- [ ] **4.3** Implement native file dialogs
  - Model import dialog
  - Path selection dialogs
  - Filter by file type

- [ ] **4.4** Implement external URL handling
  - `openUrl` → `shell.openExternal`
  - `openPath` → `shell.openPath`

- [ ] **4.5** Test drag-and-drop thoroughly
  - Single file drops
  - Multiple file drops
  - Folder drops
  - Edge cases (permissions, broken symlinks)

- [ ] **4.6** Test system tray / notifications (if used)

- [ ] **4.7** Test keyboard shortcuts

- [ ] **4.8** Test window focus/blur events

#### Testing Matrix

| Feature | PyWebView Status | Electron Status | Notes |
|---------|------------------|-----------------|-------|
| Window rendering | ✅ | ⬜ | |
| API bridge | ✅ | ⬜ | |
| Drag-and-drop | ⚠️ (workarounds) | ⬜ | |
| File dialogs | ✅ | ⬜ | |
| External links | ✅ | ⬜ | |
| Frameless window | ✅ | ⬜ | |
| DevTools | Limited | ⬜ | |
| Wayland | ⚠️ (X11 fallback) | ⬜ | |

#### Deliverables

- All features working in Electron
- No regressions from PyWebView version
- Drag-and-drop works without freezes

---

### Phase 5: Packaging & Distribution (Estimated: 1 week)

**Goal**: Create distributable Electron application.

#### Tasks

- [ ] **5.1** Configure electron-builder
  ```json
  {
    "appId": "com.pumas.library",
    "productName": "Pumas Library",
    "linux": {
      "target": ["AppImage", "deb", "rpm"],
      "category": "Development",
      "maintainer": "Your Name"
    },
    "files": [
      "electron/**/*",
      "frontend/dist/**/*",
      "backend/**/*",
      "!backend/__pycache__",
      "!**/*.pyc"
    ],
    "extraResources": [
      {
        "from": "venv",
        "to": "venv",
        "filter": ["**/*", "!**/__pycache__", "!**/*.pyc"]
      }
    ]
  }
  ```

- [ ] **5.2** Bundle Python runtime
  - Option A: Embed Python with PyInstaller
  - Option B: Require system Python (smaller package)
  - Option C: Use `python-shell` with embedded interpreter

- [ ] **5.3** Create Linux packages
  - AppImage (universal)
  - .deb (Debian/Ubuntu)
  - .rpm (Fedora/RHEL)

- [ ] **5.4** Configure auto-updates (optional)
  - `electron-updater`
  - GitHub Releases integration

- [ ] **5.5** Test installation on clean systems
  - Fresh Ubuntu 24.04
  - Fedora 40
  - Arch Linux

- [ ] **5.6** Update launcher script
  - Remove PyWebView support
  - Single Electron-based launcher

#### Deliverables

- Distributable Linux packages
- Self-contained application (no external dependencies)
- Updated installation documentation

---

### Phase 6: Cleanup & Deprecation (Estimated: 1 week)

**Goal**: Remove PyWebView code and finalize migration.

#### Tasks

- [ ] **6.1** Remove PyWebView dependencies
  - Remove from `requirements.txt`
  - Remove GTK-related system dependencies from docs

- [ ] **6.2** Remove PyWebView-specific code
  - `backend/main.py` - Remove PyWebView window creation
  - GTK drop handler workarounds
  - PyWebView DOM event handlers

- [ ] **6.3** Update documentation
  - Installation guide
  - Development setup
  - Architecture documentation

- [ ] **6.4** Update CI/CD
  - Build Electron packages
  - Run Electron tests

- [ ] **6.5** Create migration guide for developers
  - Document API changes (if any)
  - Document new development workflow

- [ ] **6.6** Archive PyWebView version
  - Tag final PyWebView release
  - Document in CHANGELOG

#### Deliverables

- Clean codebase without PyWebView remnants
- Updated documentation
- Clear upgrade path for existing users

---

## File Structure After Migration

```
Pumas-Library/
├── electron/                    # NEW: Electron-specific code
│   ├── main.ts                  # Main process entry
│   ├── preload.ts               # Context bridge
│   ├── python-bridge.ts         # Sidecar management
│   └── electron-builder.json    # Packaging config
│
├── frontend/                    # UNCHANGED (mostly)
│   ├── src/
│   │   ├── api/
│   │   │   ├── adapter.ts       # NEW: Runtime API adapter
│   │   │   ├── pywebview.ts     # Keep for reference/fallback
│   │   │   └── electron.ts      # NEW: Electron-specific calls
│   │   └── ...
│   └── ...
│
├── backend/                     # MODIFIED
│   ├── main.py                  # MODIFIED: Remove PyWebView, keep for CLI
│   ├── rpc_server.py            # NEW: JSON-RPC HTTP server
│   ├── api/                     # UNCHANGED
│   └── ...
│
├── launcher                     # MODIFIED: Electron-based
├── package.json                 # MODIFIED: Add Electron deps
└── requirements.txt             # MODIFIED: Remove PyWebView
```

---

## Risk Assessment

### High Risk

| Risk | Mitigation |
|------|------------|
| Python sidecar startup latency | Pre-spawn Python process; show loading screen |
| Python process crashes | Implement auto-restart with exponential backoff |
| IPC complexity | Use typed JSON-RPC; extensive testing |
| Package size increase (~150MB) | Acceptable trade-off for reliability |

### Medium Risk

| Risk | Mitigation |
|------|------------|
| API behavior differences | Comprehensive integration tests |
| Wayland-specific bugs | Test on multiple compositors (GNOME, KDE, Sway) |
| Electron version upgrades | Automated dependency updates; semantic versioning |

### Low Risk

| Risk | Mitigation |
|------|------------|
| Frontend changes | API adapter pattern minimizes changes |
| Developer workflow | Document new setup; provide dev scripts |

---

## Testing Strategy

### Unit Tests

- Python JSON-RPC server
- Electron IPC handlers
- API adapter

### Integration Tests

- Full roundtrip: Frontend → Electron → Python → Response
- Error handling across all layers
- Concurrent request handling

### End-to-End Tests

- All user flows from PyWebView version
- Drag-and-drop workflows
- Installation and update flows

### Platform Tests

- Ubuntu 24.04 (GNOME Wayland)
- Ubuntu 24.04 (X11)
- Fedora 40 (GNOME Wayland)
- Arch Linux (Sway)
- Linux Mint (Cinnamon/X11)

---

## Success Criteria

### Must Have

- [ ] All existing features work identically
- [ ] No drag-and-drop freezes
- [ ] Native Wayland rendering
- [ ] Package size < 200MB
- [ ] Startup time < 3 seconds

### Should Have

- [ ] DevTools available for debugging
- [ ] Hot-reload in development
- [ ] Auto-update mechanism

### Nice to Have

- [ ] Reduced memory footprint vs PyWebView
- [ ] Faster IPC than PyWebView
- [ ] Windows support path (future)

---

## References

### Electron Documentation

- [Electron Releases](https://releases.electronjs.org/)
- [Electron Timelines](https://www.electronjs.org/docs/latest/tutorial/electron-timelines)
- [Electron 38.0.0 Release Notes](https://www.electronjs.org/blog/electron-38-0)
- [Electron Security Best Practices](https://www.electronjs.org/docs/latest/tutorial/security)

### Wayland Support

- [Electron Wayland Issue #9056](https://github.com/electron/electron/issues/9056)
- [ArchWiki - Electron](https://wiki.archlinux.org/title/Electron)
- [Chromium Ozone Wayland 2025](https://www.phoronix.com/news/Chromium-Ozone-Wayland-2025)

### Python Integration

- [electron-python](https://github.com/nicjansma/electron-python)
- [python-shell](https://github.com/nicjansma/python-shell)
- [Building Python Apps with Electron](https://dnmtechs.com/building-python-applications-with-electron-framework/)

### PyWebView Comparison

- [PyWebView Documentation](https://pywebview.flowrl.com/guide/)
- [PyWebView vs Electron Comparison](https://www.saashub.com/compare-electron-vs-pywebview)

---

## Appendix A: API Method Inventory

Complete list of methods to bridge (from `pywebview.d.ts`):

### Status Methods
- `getStatus()`
- `getDiskSpace()`
- `getSystemResources()`

### Action Methods
- `installDeps()`
- `togglePatch()`
- `toggleMenu(tag?)`
- `toggleDesktop(tag?)`
- `closeWindow()`
- `launchComfyui()`
- `stopComfyui()`

### Version Management (26 methods)
- `getAvailableVersions(forceRefresh?)`
- `getInstalledVersions()`
- `validateInstallations()`
- `getInstallationProgress()`
- `installVersion(tag)`
- `cancelInstallation()`
- `calculateReleaseSize(tag, forceRefresh?)`
- `calculateAllReleaseSizes()`
- `removeVersion(tag)`
- `switchVersion(tag)`
- `getActiveVersion()`
- `getDefaultVersion()`
- `setDefaultVersion(tag?)`
- `checkVersionDependencies(tag)`
- `installVersionDependencies(tag)`
- `getVersionStatus()`
- `getVersionInfo(tag)`
- `getVersionShortcuts(tag)`
- `getAllShortcutStates()`
- `setVersionShortcuts(tag, enabled)`
- `toggleVersionMenu(tag)`
- `toggleVersionDesktop(tag)`
- `launchVersion(tag, extraArgs?)`
- `getReleaseSize(tag, archiveSize)`
- `getReleaseSizeBreakdown(tag)`
- `getReleaseDependencies(tag, topN?)`

### Model Management (20 methods)
- `getModels()`
- `refreshModelIndex()`
- `refreshModelMappings(appId?)`
- `importModel(localPath, family, officialName, repoId?)`
- `downloadModelFromHf(...)`
- `startModelDownloadFromHf(...)`
- `getModelDownloadStatus(downloadId)`
- `cancelModelDownload(downloadId)`
- `searchHfModels(query, kind?, limit?)`
- `getModelOverrides(relPath)`
- `updateModelOverrides(relPath, overrides)`
- `importBatch(specs)`
- `lookupHfMetadataForFile(filename, filePath?)`
- `detectShardedSets(filePaths)`
- `validateFileType(filePath)`
- `getNetworkStatus()`
- `getLibraryStatus()`
- `getFileLinkCount(filePath)`
- `checkFilesWritable(filePaths)`
- `markMetadataAsManual(modelId)`
- `searchModelsFts(query, limit?, offset?, modelType?, tags?)`

### Custom Nodes (4 methods)
- `getCustomNodes(versionTag)`
- `installCustomNode(gitUrl, versionTag, nodeName?)`
- `updateCustomNode(nodeName, versionTag)`
- `removeCustomNode(nodeName, versionTag)`

### Utility Methods (11 methods)
- `openPath(path)`
- `openActiveInstall()`
- `openUrl(url)`
- `openModelImportDialog()`
- `scanSharedStorage()`
- `getLinkHealth(versionTag?)`
- `getGithubCacheStatus()`
- `hasBackgroundFetchCompleted()`
- `resetBackgroundFetchFlag()`
- `getLauncherVersion()`
- `checkLauncherUpdates(forceRefresh?)`
- `applyLauncherUpdate()`
- `restartLauncher()`

**Total: 60+ methods**

---

## Appendix B: GTK3 Issues Resolved

### Drag-and-Drop Freezes (commit 1b02701)

**Problem**: WebKit2GTK's internal drag-and-drop handling conflicts with GTK3 signals, causing the desktop to freeze when dropping files.

**Current Workaround**:
- Bypass WebView widget, attach drop handlers to GTK Window
- Use `GLib.idle_add()` to defer JavaScript execution
- Use `Gtk.drag_finish()` to immediately signal completion

**Electron Solution**: Native Chromium drag-and-drop, no GTK involvement.

### X11-Specific Rendering

**Problem**: GTK3 defaults to X11 mode even on Wayland, causing:
- Blurry rendering on HiDPI displays
- Input latency
- Compositor conflicts

**Electron Solution**: Ozone platform auto-detection enables native Wayland.

### WebKit2GTK Limitations

**Problem**: WebKit2GTK lags behind Safari/WebKit by months, missing:
- Modern CSS features
- JavaScript APIs
- DevTools capabilities

**Electron Solution**: Chromium M140+ with full web platform support.

---

**End of Migration Plan**
