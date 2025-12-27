# Linux ComfyUI Launcher - Developer Documentation

A modern desktop application for managing multiple ComfyUI installations on Linux. Built with PyWebView + React for a native desktop experience with zero runtime dependencies for end users.

## Overview

This is a comprehensive ComfyUI version manager and launcher that provides:
- Multi-version installation and management
- Shared resource storage (models, custom nodes, workflows)
- Desktop integration with version-specific shortcuts
- Real-time installation progress tracking
- Advanced dependency management with version pinning

**For End Users**: Single executable, no dependencies required
**For Developers**: Python 3.12+ backend, React 19 frontend, modern build tooling

## Features

### Core Features
- **Multi-Version Management**: Install, switch between, and launch multiple ComfyUI versions
- **Shared Resource System**: Symlink-based model/custom node sharing across versions
- **Real-Time Installation**: 5-stage progress tracking with speed/ETA
- **Desktop Integration**: System shortcuts with version-specific icons
- **Zero Dependencies**: Single executable (484MB) with bundled Python runtime

### Advanced Features
- **Installation Progress System**:
  - 5 weighted stages (Download, Extract, Venv, Dependencies, Setup)
  - Real-time download speed and ETA calculation
  - Package-by-package installation tracking
  - Installation cancellation with cleanup
  - Background installation support
  - Auto-close on completion

- **Size Calculation System**:
  - Pre-installation total size calculation
  - PyPI package size resolution (platform-aware)
  - Size breakdown (archive + dependencies)
  - Smart caching per release

- **Constraints-Based Dependency Pinning**:
  - Automatic version pinning based on release dates
  - Constraints file generation for reproducible installations
  - PyPI historical version resolution

- **Process Management**:
  - Download progress via process I/O tracking (psutil)
  - PID file tracking per version
  - Process group management (clean SIGTERM → SIGKILL)
  - Previous instance cleanup

- **Validation System**:
  - Startup validation of all installations
  - Orphaned installation detection and cleanup
  - Incomplete installation recovery

## Technology Stack

### Frontend
- **React 19** + **TypeScript** - Modern UI framework with type safety
- **Vite 6.2** - Fast build tool with HMR
- **Framer Motion 12** - Smooth animations
- **Lucide React 0.561** - Icon library
- **Tailwind CSS** - Utility-first styling (via inline classes)

### Backend
- **Python 3.12+** - Core application logic
- **PyWebView 5.x** - Desktop GUI framework (GTK backend)
- **PyInstaller 6.x** - Single executable packaging
- **psutil 5.9** - Process monitoring and I/O tracking
- **setproctitle 1.3** - Process naming
- **Pillow 10.x** - Icon generation with version overlays
- **packaging 23-24** - Dependency metadata parsing

### System Integration
- **GTK3** + **WebKitGTK** - Native Linux desktop integration (system-provided)
- **xdg-utils** - Desktop/menu shortcut creation
- **wmctrl** - Window management
- **GitHub REST API** - Release fetching
- **PyPI JSON API** - Package size resolution

### Target Platform
- **OS**: Linux x86_64 (Debian-based: Ubuntu, Mint, Debian, Pop!_OS)
- **Runtime**: System GTK3/WebKitGTK (pre-installed on most distros)
- **Distribution**: Single executable (484MB)

## Architecture

### Directory Structure

```
Linux-ComfyUI-Launcher/
├── frontend/                      # React application (9 files)
│   ├── src/
│   │   ├── App.tsx               # Main React component (25KB, 875 lines)
│   │   ├── main.tsx              # React entry point
│   │   ├── components/           # UI components
│   │   │   ├── VersionSelector.tsx      # Version dropdown and install dialog
│   │   │   ├── InstallDialog.tsx        # Installation progress modal
│   │   │   ├── ProgressRing.tsx         # Circular progress indicator
│   │   │   └── SpringyToggle.tsx        # Animated toggle switches
│   │   └── hooks/
│   │       └── useVersions.ts           # Version data management hook
│   ├── dist/                     # Built frontend (production)
│   ├── package.json              # Frontend dependencies
│   ├── tsconfig.json             # TypeScript configuration
│   └── vite.config.ts            # Vite build configuration
│
├── backend/                       # Python backend (31 files)
│   ├── main.py                   # PyWebView entry point & JS API bridge
│   ├── api.py                    # Main business logic API (75KB, central coordinator)
│   ├── version_manager.py        # Version installation & management (76KB)
│   ├── resource_manager.py       # Shared storage & symlink management
│   ├── github_integration.py     # GitHub releases API & downloads
│   ├── metadata_manager.py       # JSON metadata persistence
│   ├── config.py                 # Centralized configuration
│   ├── models.py                 # TypedDict data models
│   │
│   ├── api/                      # Modular API components
│   │   ├── core.py              # Core status and state management
│   │   ├── shortcut_manager.py  # Desktop/menu shortcut creation
│   │   ├── patch_manager.py     # Main.py patching for setproctitle
│   │   ├── process_manager.py   # Launch and process lifecycle
│   │   └── ...
│   │
│   └── resources/                # Resource management modules
│       ├── model_manager.py     # Model discovery and metadata
│       ├── shared_storage.py    # Shared resource initialization
│       ├── custom_nodes_manager.py  # Custom node management
│       └── symlink_manager.py   # Symlink creation and validation
│
├── launcher-data/                 # Runtime data (created on first run)
│   ├── metadata/                 # JSON metadata files
│   │   ├── versions.json        # Installed versions metadata
│   │   ├── active_version.json  # Current active version
│   │   └── size_cache.json      # Installation size cache
│   ├── cache/                    # Download and API caches
│   │   ├── github_releases.json # GitHub API response cache (1hr TTL)
│   │   ├── constraints/         # Generated constraints files
│   │   ├── downloads/           # Downloaded archives
│   │   └── pip/                 # Pip cache directory
│   ├── logs/                     # Installation and launch logs
│   │   ├── installation-[version]-[timestamp].log
│   │   └── launch-[version]-[timestamp].log
│   ├── profiles/                 # Browser profiles (Brave)
│   │   └── [version]/           # Isolated profile per version
│   ├── shortcuts/                # Launch scripts
│   │   └── launch-[version].sh  # Shell script per version
│   └── icons/                    # Generated version-specific icons
│       └── comfyui-[version].png
│
├── shared-resources/              # Shared across all versions
│   ├── models/                   # 16 model categories
│   │   ├── checkpoints/
│   │   ├── loras/
│   │   ├── vae/
│   │   ├── controlnet/
│   │   ├── embeddings/
│   │   ├── clip/
│   │   ├── upscale_models/
│   │   └── ... (9 more)
│   ├── custom_nodes_cache/       # Git repository cache
│   │   └── [node-repo-name]/    # Cloned repositories
│   └── user/                     # User data
│       ├── workflows/            # Shared workflows
│       └── settings/             # Shared settings
│
├── comfyui-versions/              # Installed ComfyUI versions
│   ├── v0.4.0/                   # Example version
│   │   ├── main.py              # ComfyUI entry point
│   │   ├── venv/                # Isolated virtual environment
│   │   ├── models/              # Symlink → ../../shared-resources/models
│   │   ├── custom_nodes/        # Version-specific custom nodes
│   │   ├── input/               # ComfyUI input directory
│   │   ├── output/              # ComfyUI output directory
│   │   ├── requirements.txt     # Original requirements
│   │   └── constraints.txt      # Generated version constraints
│   ├── v0.5.1/
│   └── v0.6.0/
│
├── dist/                          # Built executable
│   └── comfyui-setup             # Single executable (484MB)
│
├── build/                         # PyInstaller build artifacts (temp)
│
├── venv/                          # Development virtual environment
│
├── tests/                         # Test files
│   ├── test_api.py
│   ├── test_version_manager.py
│   ├── test_resource_manager.py
│   └── test_github_integration.py
│
├── scripts/                       # Scripts directory
│   ├── templates/                # Template files for generated scripts
│   │   └── comfyui_run.sh       # Reference template for ComfyUI run scripts
│   ├── dev/                      # Developer-only scripts
│   │   ├── setup.sh             # Development environment setup
│   │   ├── build.sh             # Production build script
│   │   └── run-dev.sh           # Quick development launcher
│   └── system-check.sh          # System dependency checker
│
├── install.sh                    # End-user installation script
├── launcher                      # Launcher wrapper (generated by install.sh)
├── requirements.txt              # Python dependencies
├── comfyui-setup.spec           # PyInstaller configuration
└── diagnose_imports.py          # Import diagnostics script
```

### Module Architecture

#### Backend Core Modules

**[main.py](backend/main.py)** - PyWebView Entry Point
- Initializes PyWebView window (GTK backend)
- Exposes `JavaScriptAPI` class to frontend
- Handles development vs production mode detection
- Configures window properties (size, resizable, debug)

**[api.py](backend/api.py)** - Central API Coordinator (75KB)
- Main business logic orchestration
- Delegates to specialized managers
- Status aggregation from multiple sources
- Version lifecycle management (install, switch, remove)
- Launch coordination

**[version_manager.py](backend/version_manager.py)** - Version Management (76KB)
- GitHub release installation
- 5-stage installation process
- Progress tracking and reporting
- Virtual environment creation
- Dependency installation with constraints
- Installation cancellation
- Validation and cleanup

**[resource_manager.py](backend/resource_manager.py)** - Resource Management
- Shared storage initialization
- Symlink creation and validation
- Model directory discovery
- Custom node management
- Resource metadata tracking

**[github_integration.py](backend/github_integration.py)** - GitHub API
- Release fetching with pagination
- Download manager with progress tracking
- API response caching (1-hour TTL)
- Rate limiting and retry logic

**[metadata_manager.py](backend/metadata_manager.py)** - Persistence
- JSON file reading/writing
- Metadata schema management
- Atomic file updates
- Default value handling

**[config.py](backend/config.py)** - Configuration
- Centralized path definitions
- Directory structure constants
- Feature flags
- System-wide settings

**[models.py](backend/models.py)** - Type Definitions
- TypedDict definitions for type safety
- Shared data structures
- API request/response models

#### Backend API Modules

**[api/core.py](backend/api/core.py)** - Core State Management
- Status aggregation
- Active version tracking
- Installation state queries

**[api/shortcut_manager.py](backend/api/shortcut_manager.py)** - Desktop Integration
- Desktop file generation (`.desktop` files)
- Icon installation (freedesktop spec)
- Launch script creation
- xdg-utils integration

**[api/patch_manager.py](backend/api/patch_manager.py)** - Patching
- main.py patching for setproctitle
- Patch reversal
- Backup management

**[api/process_manager.py](backend/api/process_manager.py)** - Process Lifecycle
- ComfyUI server launching
- Browser opening (Brave/default)
- Server readiness polling
- PID tracking
- Instance cleanup

#### Backend Resource Modules

**[resources/model_manager.py](backend/resources/model_manager.py)** - Model Management
- Model discovery from folder_paths.py
- 16 model category support
- Model metadata tracking
- Size calculation

**[resources/shared_storage.py](backend/resources/shared_storage.py)** - Shared Storage
- Directory structure initialization
- Model category creation
- Shared resource setup

**[resources/custom_nodes_manager.py](backend/resources/custom_nodes_manager.py)** - Custom Nodes
- Per-version custom node directories
- Git repository caching
- Installation/update/removal

**[resources/symlink_manager.py](backend/resources/symlink_manager.py)** - Symlink Management
- Symlink creation with validation
- Broken link detection
- Automatic re-linking

#### Frontend Architecture

**[App.tsx](frontend/src/App.tsx)** - Main Application (25KB)
- Application state management
- PyWebView API integration
- Status polling (500ms interval)
- User interaction handling
- Error display

**[VersionSelector.tsx](frontend/src/components/VersionSelector.tsx)** - Version UI
- Version dropdown rendering
- Install dialog trigger
- Version switching
- Force refresh

**[InstallDialog.tsx](frontend/src/components/InstallDialog.tsx)** - Installation UI
- Available versions list
- Installation progress display
- 5-stage progress breakdown
- Cancellation button
- Size calculation display

**[ProgressRing.tsx](frontend/src/components/ProgressRing.tsx)** - Progress Indicator
- Circular progress bar
- Animated stroke
- Percentage display

**[SpringyToggle.tsx](frontend/src/components/SpringyToggle.tsx)** - Toggle Switches
- Animated toggle switches (Framer Motion)
- Spring physics
- Status display

**[useVersions.ts](frontend/src/hooks/useVersions.ts)** - Version Hook
- Version data fetching
- Filtering logic (pre-releases, installed)
- Collapsing to latest patch per minor version
- Sorting

## For End Users

### Running the Application

Download the latest release and run:

```bash
chmod +x comfyui-setup
./comfyui-setup
```

**System Requirements:**
- Linux (Debian-based: Ubuntu, Mint, Debian, Pop!_OS, etc.)
- GTK3 and WebKitGTK (pre-installed on most distributions)
- 500MB+ free disk space for the launcher
- Additional space for ComfyUI installations and models

**No Python, Node.js, or development tools required for end users.**

See [README.md](README.md) for user documentation.

## For Developers

### Prerequisites

**Required:**
- **Python 3.12+** (3.13 recommended)
- **Node.js 18+** and npm (for building frontend)
- **Git** (for cloning and version control)

**System Dependencies** (usually pre-installed):
- **GTK3** - `libgtk-3-0`
- **WebKitGTK** - `libwebkit2gtk-4.1-0`
- **Python GI bindings** - `python3-gi`, `gir1.2-gtk-3.0`, `gir1.2-webkit2-4.1`

### Initial Setup

Clone the repository and run the setup script:

```bash
git clone <repository-url>
cd Linux-ComfyUI-Launcher
scripts/dev/setup.sh
```

**What `scripts/dev/setup.sh` does:**
1. Creates Python virtual environment with `--system-site-packages` (for GTK access)
2. Upgrades pip to latest version
3. Installs Python dependencies from [requirements.txt](requirements.txt):
   - pywebview >= 5.0
   - pyinstaller >= 6.0
   - psutil >= 5.9
   - setproctitle >= 1.3
   - Pillow >= 10.0
   - packaging >= 23.0
   - click >= 8.1
4. Checks and installs system dependencies via apt (requires sudo):
   - `libgtk-3-0`
   - `libwebkit2gtk-4.1-0`
   - `gir1.2-webkit2-4.1`
   - `python3-gi`
   - `gir1.2-gtk-3.0`
5. Installs frontend npm packages in [frontend/](frontend/)

**Manual Setup** (if script fails):
```bash
# Backend
python3 -m venv venv --system-site-packages
source venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt

# System dependencies
sudo apt update
sudo apt install -y libgtk-3-0 libwebkit2gtk-4.1-0 gir1.2-webkit2-4.1 \
                    python3-gi gir1.2-gtk-3.0

# Frontend
cd frontend
npm install
cd ..
```

### Development Workflow

#### Running in Development Mode

Development mode requires two terminal windows running simultaneously:

**Terminal 1 - Frontend Dev Server:**
```bash
cd frontend
npm run dev
```
- Starts Vite dev server on `http://127.0.0.1:3000`
- Provides hot module replacement (HMR)
- Enables instant UI updates without rebuilding

**Terminal 2 - Python Application:**
```bash
scripts/dev/run-dev.sh
```

Or manually:
```bash
source venv/bin/activate
python3 backend/main.py
```

**Note**: The `run-dev.sh` script automatically activates the venv for convenience.

**How Development Mode Works:**
1. The Python app checks if `frontend/dist/index.html` exists
2. If not found, it connects to Vite dev server at `http://127.0.0.1:3000`
3. PyWebView loads the dev server URL instead of local files
4. All API bridge functionality remains active

**Benefits:**
- Hot module replacement for React changes (instant updates)
- Faster iteration on UI/UX
- Browser DevTools available (press F12)
- Full PyWebView API bridge functionality
- No rebuild required for frontend changes

**Limitations:**
- Requires both terminals running simultaneously
- First load may be slower (Vite compilation)
- Must use correct Python launch method for API to work

**Verifying Development Mode:**
- Terminal shows: `Development mode detected, using Vite dev server`
- Browser DevTools (F12) shows Vite connection messages
- Changes to `.tsx` files update UI instantly

#### Building for Production

To create a production build:

```bash
scripts/dev/build.sh
```

**What `scripts/dev/build.sh` does:**
1. **Builds Frontend** (`npm run build` in [frontend/](frontend/))
   - Compiles TypeScript to JavaScript
   - Bundles React components with Vite
   - Optimizes and minifies assets
   - Outputs to `frontend/dist/`
   - Typical build time: 10-30 seconds

2. **Bundles with PyInstaller** (`pyinstaller comfyui-setup.spec`)
   - Packages Python 3.12+ runtime
   - Includes all pip dependencies
   - Embeds `frontend/dist/` files
   - Strips debug symbols for smaller size
   - Creates single executable
   - Typical build time: 2-4 minutes

**Build Output:**
- **Executable**: `dist/comfyui-setup`
- **Size**: 484MB (includes Python runtime + all dependencies)
- **Contents**:
  - Python 3.12+ interpreter
  - PyWebView + GTK bindings
  - All Python packages (psutil, Pillow, etc.)
  - React build files (HTML, JS, CSS)
  - Application logic (backend/)

**Build Artifacts** (can be deleted):
- `build/` - PyInstaller temporary files
- `comfyui-setup.spec.lock` - Build lock file

**What's NOT Bundled** (uses system):
- GTK3 libraries
- WebKitGTK libraries
- System fonts and themes

This approach keeps the binary size manageable (~484MB) while ensuring compatibility across Debian-based distributions.

#### Running Production Build

```bash
./dist/comfyui-setup
```

**Testing Production Build:**
1. Move or delete `frontend/dist/` to prevent dev mode fallback
2. Run the executable
3. Verify all features work (install, launch, shortcuts)
4. Check console for errors
5. Test on clean system (VM) before distributing

### JavaScript ↔ Python Communication

The PyWebView bridge provides seamless communication between the React frontend and Python backend.

**Python Side** ([backend/main.py](backend/main.py)):
```python
class JavaScriptAPI:
    """Exposed to JavaScript via window.pywebview.api"""

    def __init__(self):
        self.api = ComfyUISetupAPI()

    # Version management
    def get_status(self):
        """Returns current status with all version info"""
        return self.api.get_status()

    def get_available_versions(self, force_refresh=False):
        """Fetches available versions from GitHub"""
        return self.api.get_available_versions(force_refresh)

    def install_version(self, tag):
        """Starts version installation in background"""
        return self.api.install_version(tag)

    def get_installation_progress(self, tag):
        """Gets real-time installation progress"""
        return self.api.get_installation_progress(tag)

    def cancel_installation(self, tag):
        """Cancels ongoing installation"""
        return self.api.cancel_installation(tag)

    def switch_version(self, tag):
        """Switches active version"""
        return self.api.switch_version(tag)

    # Launch and shortcuts
    def launch_version(self, tag):
        """Launches ComfyUI version"""
        return self.api.launch_version(tag)

    def toggle_desktop_shortcuts(self, tag):
        """Creates/removes desktop shortcuts"""
        return self.api.toggle_desktop_shortcuts(tag)

    def toggle_menu_shortcuts(self, tag):
        """Creates/removes menu shortcuts"""
        return self.api.toggle_menu_shortcuts(tag)

    # Size calculation
    def calculate_installation_size(self, tag):
        """Calculates total installation size"""
        return self.api.calculate_installation_size(tag)
```

**JavaScript/TypeScript Side** ([frontend/src/App.tsx](frontend/src/App.tsx)):
```typescript
// TypeScript interface for type safety
interface PyWebViewAPI {
  get_status(): Promise<Status>;
  get_available_versions(force_refresh?: boolean): Promise<Version[]>;
  install_version(tag: string): Promise<{success: boolean; message?: string}>;
  get_installation_progress(tag: string): Promise<InstallationProgress>;
  cancel_installation(tag: string): Promise<{success: boolean}>;
  switch_version(tag: string): Promise<{success: boolean}>;
  launch_version(tag: string): Promise<{success: boolean; message?: string}>;
  toggle_desktop_shortcuts(tag: string): Promise<{success: boolean}>;
  toggle_menu_shortcuts(tag: string): Promise<{success: boolean}>;
  calculate_installation_size(tag: string): Promise<SizeInfo>;
}

// Access the API
declare global {
  interface Window {
    pywebview: {
      api: PyWebViewAPI;
    };
  }
}

// Usage in components
const status = await window.pywebview.api.get_status();
const versions = await window.pywebview.api.get_available_versions(true);
await window.pywebview.api.install_version('v0.6.0');
```

**Data Flow:**
1. User interacts with React UI
2. React calls `window.pywebview.api.method()`
3. PyWebView serializes call and sends to Python
4. Python executes method and returns result
5. PyWebView serializes response and sends to JavaScript
6. React updates UI with result

### Key Features Implementation

#### 5-Stage Installation Progress System

The installation process is divided into 5 weighted stages for accurate progress reporting:

```python
# Stage weights (total = 100%)
STAGE_WEIGHTS = {
    'DOWNLOAD': 15,    # Archive download (network-bound)
    'EXTRACT': 5,      # Archive extraction (disk-bound)
    'VENV': 5,         # Virtual environment creation (CPU-bound)
    'DEPENDENCIES': 70,  # Pip package installation (largest time investment)
    'SETUP': 5,        # Symlink creation (fast)
}
```

**Progress Calculation:**
```python
def calculate_overall_progress(stage, stage_progress):
    """
    Calculates overall 0-100% progress across all stages.

    Example: 50% through DEPENDENCIES stage
    = 15 (DOWNLOAD) + 5 (EXTRACT) + 5 (VENV) + (70 * 0.5) (DEPENDENCIES)
    = 60% overall
    """
    completed_weight = sum(STAGE_WEIGHTS[s] for s in completed_stages)
    current_weight = STAGE_WEIGHTS[stage] * (stage_progress / 100)
    return completed_weight + current_weight
```

**Stage Details:**

1. **DOWNLOAD** (15%):
   - Downloads archive from GitHub releases
   - Tracks speed (MB/s) via psutil I/O counters
   - Calculates ETA based on download speed
   - Updates: `{"current_item": "archive.tar.gz", "speed": "5.2 MB/s", "eta": "2m 30s"}`

2. **EXTRACT** (5%):
   - Extracts archive to `comfyui-versions/[tag]/`
   - Progress based on extracted file count
   - Updates: `{"current_item": "folder_paths.py"}`

3. **VENV** (5%):
   - Creates Python virtual environment with `python3 -m venv`
   - Single-step progress (0% → 100%)
   - Updates: `{"current_item": "Creating virtual environment"}`

4. **DEPENDENCIES** (70%):
   - Installs packages from `requirements.txt` with constraints
   - Tracks package-by-package installation
   - Shows count: "Installing package 12/45"
   - Lists completed packages with sizes
   - Updates: `{"current_item": "torch==2.0.1", "progress": 26.7%, "completed": ["pkg1 (50MB)", "pkg2 (10MB)"]}`

5. **SETUP** (5%):
   - Creates symlinks to shared resources
   - Finalizes installation metadata
   - Updates: `{"current_item": "Creating symlinks"}`

**Installation Cancellation:**
```python
def cancel_installation(tag):
    """
    Cancels installation with cleanup:
    1. Sets cancellation flag
    2. Kills subprocess (download/pip) with SIGTERM → SIGKILL
    3. Deletes partial installation directory
    4. Removes installation metadata
    5. Returns {"success": True}
    """
```

#### Size Calculation System

Pre-calculates installation size before downloading:

```python
def calculate_installation_size(tag):
    """
    Calculates total size = archive_size + dependencies_size

    Process:
    1. Get archive size from GitHub API
    2. Parse requirements.txt from GitHub
    3. Generate constraints file based on release date
    4. For each package:
       - Query PyPI JSON API for version history
       - Find version <= release_date
       - Get linux_x86_64 wheel size or sdist size
    5. Sum all sizes
    6. Cache result (key = requirements.txt hash + release_date)

    Returns:
    {
        "total_size": 2500000000,  # bytes
        "archive_size": 150000000,
        "dependencies_size": 2350000000,
        "breakdown": {
            "torch": 800000000,
            "numpy": 50000000,
            ...
        }
    }
    """
```

**Caching Strategy:**
- Cache key: `hash(requirements.txt) + release_date`
- Stored in: `launcher-data/metadata/size_cache.json`
- Invalidation: Never (release content is immutable)
- Hit rate: ~95% for repeated calculations

#### Constraints-Based Dependency Pinning

Ensures reproducible installations by pinning dependencies to release dates:

```python
def generate_constraints_file(tag, release_date):
    """
    Generates pip constraints file for reproducible installs.

    For each dependency, finds the newest version published <= release_date:
    1. Query PyPI JSON API for package
    2. Get all release versions with dates
    3. Filter to versions <= release_date
    4. Select newest compatible version
    5. Write to constraints.txt: "package==x.y.z"

    Example constraints.txt:
    torch==2.0.1
    torchvision==0.15.2
    numpy==1.24.3
    ...

    Install command:
    pip install -r requirements.txt -c constraints.txt
    """
```

**Why This Matters:**
- ComfyUI v0.4.0 released in June 2023 expected `torch==2.0.0`
- Without constraints, `pip install torch` would get `torch==2.2.0` (incompatible)
- With constraints, installation gets `torch==2.0.1` (latest compatible at that date)
- Result: Reproducible, working installations

#### Shared Resource Management

Symlink-based sharing eliminates model duplication:

```python
def setup_shared_resources(version_dir):
    """
    Creates symlinks from version to shared resources:

    comfyui-versions/v0.6.0/models → ../../shared-resources/models

    Process:
    1. Discover model directories from folder_paths.py
    2. For each model category:
       - Create shared directory if not exists
       - Remove version's directory if exists
       - Create symlink: ln -s ../../shared-resources/models/[category]
    3. Validate all symlinks

    Result:
    - All versions access same models
    - Zero duplication
    - Transparent to ComfyUI (sees normal directories)
    """
```

**16 Model Categories:**
```python
MODEL_CATEGORIES = [
    'checkpoints', 'loras', 'vae', 'controlnet', 'embeddings',
    'clip', 'clip_vision', 'style_models', 'upscale_models',
    'diffusers', 'gligen', 'hypernetworks', 'photomaker',
    'unet', 'vae_approx', 'diffusion_models'
]
```

#### Process Management

Robust process lifecycle handling:

```python
def launch_version(tag):
    """
    Launches ComfyUI with proper cleanup:

    1. Kill previous instance:
       - Read PID from launcher-data/[tag].pid
       - Send SIGTERM (grace period: 5s)
       - Send SIGKILL if still running
       - Delete PID file

    2. Close previous window:
       - wmctrl -c "ComfyUI-v0.6.0" (window class)

    3. Start server:
       - cd comfyui-versions/[tag]
       - venv/bin/python main.py &
       - Capture PID and write to PID file

    4. Poll for readiness:
       - Check http://127.0.0.1:8188/ every 500ms
       - Timeout: 30 seconds
       - Return error if fails

    5. Open browser:
       - brave-browser --new-window --user-data-dir=launcher-data/profiles/[tag] http://127.0.0.1:8188
       - Fallback to xdg-open if Brave not found

    Returns: {"success": True, "pid": 12345}
    """
```

#### Icon Generation

Creates version-specific icons with labels:

```python
def generate_icon(tag):
    """
    Generates icon with version label using Pillow:

    1. Load base ComfyUI icon (PNG)
    2. Create overlay with version text
    3. Draw text with font: DejaVu Sans Bold 48pt
    4. Add shadow for readability
    5. Composite base + overlay
    6. Save to launcher-data/icons/comfyui-[tag].png
    7. Install to system icon directories:
       - ~/.local/share/icons/hicolor/256x256/apps/
       - ~/.local/share/icons/hicolor/128x128/apps/
       - ~/.local/share/icons/hicolor/48x48/apps/
    8. Update icon cache: gtk-update-icon-cache

    Result: Version-specific icons in app menus and desktop
    """
```

#### Development vs Production Mode

Automatic mode detection based on filesystem:

```python
# backend/main.py
FRONTEND_DIST = Path(__file__).parent.parent / "frontend" / "dist"

if (FRONTEND_DIST / "index.html").exists():
    # Production mode: serve from built files
    window = webview.create_window(
        title="ComfyUI Launcher",
        url=str(FRONTEND_DIST / "index.html"),
        width=1200,
        height=800,
    )
else:
    # Development mode: connect to Vite dev server
    window = webview.create_window(
        title="ComfyUI Launcher (Dev)",
        url="http://127.0.0.1:3000",
        width=1200,
        height=800,
    )
```

### Customization

#### Changing Window Size

Edit [backend/main.py](backend/main.py):
```python
window = webview.create_window(
    title="ComfyUI Launcher",
    url=url,
    width=1200,   # Change width
    height=800,   # Change height
    resizable=True,
    ...
)
```

#### Adding New API Methods

**Step 1**: Add method to `ComfyUISetupAPI` class in [backend/api.py](backend/api.py):
```python
class ComfyUISetupAPI:
    def my_new_method(self, param):
        """Business logic here"""
        result = do_something(param)
        return {"success": True, "data": result}
```

**Step 2**: Expose in `JavaScriptAPI` class in [backend/main.py](backend/main.py):
```python
class JavaScriptAPI:
    def my_new_method(self, param):
        """Exposed to JavaScript"""
        return self.api.my_new_method(param)
```

**Step 3**: Add TypeScript definition in [frontend/src/App.tsx](frontend/src/App.tsx):
```typescript
interface PyWebViewAPI {
  // Existing methods...
  my_new_method(param: string): Promise<{success: boolean; data: any}>;
}
```

**Step 4**: Call from React component:
```typescript
const result = await window.pywebview.api.my_new_method("value");
if (result.success) {
  console.log("Data:", result.data);
}
```

#### Modifying UI

Edit React components in [frontend/src/](frontend/src/):
- [App.tsx](frontend/src/App.tsx) - Main application layout and state
- Add new components in [src/components/](frontend/src/components/)
- Use Tailwind CSS classes for styling (e.g., `className="bg-blue-500 text-white"`)
- Use Framer Motion for animations (e.g., `<motion.div animate={{opacity: 1}}`)

**Example new component:**
```typescript
// frontend/src/components/MyComponent.tsx
import { motion } from 'framer-motion';

export function MyComponent({ onAction }: { onAction: () => void }) {
  return (
    <motion.button
      className="px-4 py-2 bg-blue-500 text-white rounded"
      whileHover={{ scale: 1.05 }}
      whileTap={{ scale: 0.95 }}
      onClick={onAction}
    >
      Click Me
    </motion.button>
  );
}
```

### Developer Troubleshooting

#### Build Issues

**Frontend build fails:**
```bash
cd frontend
rm -rf node_modules package-lock.json dist
npm install
npm run build
```

**PyInstaller fails:**
```bash
source venv/bin/activate
pip install --upgrade pyinstaller
rm -rf build dist
scripts/dev/build.sh
```

**Module import errors:**
- Use `scripts/dev/run-dev.sh` for development
- Or use `python3 backend/main.py` after activating venv
- The launcher wrapper handles paths correctly

#### Runtime Issues

**GTK/WebKitGTK errors:**
```bash
sudo apt update
sudo apt install -y libgtk-3-0 libwebkit2gtk-4.1-0 gir1.2-webkit2-4.1 \
                    python3-gi gir1.2-gtk-3.0
```

**Window doesn't open:**
```bash
# Check GTK backend availability
python3 -c "import gi; gi.require_version('Gtk', '3.0'); from gi.repository import Gtk"

# Run with debug mode
# Edit backend/main.py and set: webview.create_window(..., debug=True)
scripts/dev/run-dev.sh
```

**Development mode not working:**
1. Ensure Vite dev server is running: `cd frontend && npm run dev`
2. Verify no `frontend/dist/index.html` exists (move it away if present)
3. Check terminal for "Development mode detected" message

**Hot Module Replacement not working:**
- Ensure Vite dev server shows "ready in X ms"
- Check browser console (F12) for Vite connection errors
- Try clearing browser cache

See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for comprehensive troubleshooting guide.

### Best Practices

#### Code Style

**Python** (PEP 8):
- Use type hints where beneficial: `def method(param: str) -> dict:`
- Descriptive variable names: `installation_progress` not `ip`
- Docstrings for public methods
- Keep functions focused and single-purpose

**TypeScript/React**:
- Use functional components with hooks
- Prefer `const` over `let`
- Destructure props: `function MyComponent({ title, onAction }: Props)`
- Use TypeScript interfaces for type safety

**Comments**:
- Explain WHY, not WHAT: `# Use constraints to pin deps to release date` ✓
- Avoid obvious comments: `# Increment counter` ✗
- Document complex algorithms and business logic

**Naming Conventions**:
- Python: `snake_case` for variables/functions, `PascalCase` for classes
- TypeScript: `camelCase` for variables/functions, `PascalCase` for components
- Be consistent with existing codebase

#### Architecture Principles

1. **Separation of Concerns**:
   - Frontend: UI and user interactions only
   - Backend: Business logic, file system, external APIs
   - Never mix presentation with business logic

2. **Modular Design**:
   - Each module has a single responsibility
   - Clear interfaces between modules
   - Easy to test in isolation

3. **Error Handling**:
   - All API methods return `{"success": bool, "message": str}` format
   - Log errors to appropriate log files
   - Display user-friendly error messages in UI

4. **Data Flow**:
   - Single source of truth for state
   - Unidirectional data flow (Python → JavaScript)
   - No shared mutable state

### Performance Considerations

**Bundle Size:**
- Executable: 484MB (includes Python runtime + all dependencies)
- Reasonable for desktop application
- Users download once, use indefinitely

**Startup Time:**
- Cold start: <3 seconds on modern hardware
- Development mode: +1-2 seconds (Vite compilation)
- Production mode: ~2 seconds to ready

**Memory Usage:**
- Base: ~80-150 MB (PyWebView + WebKitGTK)
- During installation: +100-200 MB (subprocess overhead)
- Multiple versions running: +500-1000 MB per ComfyUI instance

**Optimization Tips:**
- Use background installations (non-blocking UI)
- Cache GitHub API responses (1-hour TTL)
- Cache size calculations (immutable releases)
- Reuse PyPI queries across versions

### Security Considerations

**Network Requests:**
- GitHub API: Release fetching (HTTPS)
- PyPI API: Package size queries (HTTPS)
- No telemetry or analytics
- No user data transmitted

**File System Access:**
- All operations within launcher directory
- Symlinks point to shared resources (within launcher)
- No access to user's home directory (except `~/.local/share` for shortcuts)

**Subprocess Execution:**
- Only trusted commands: `python3`, `pip`, `git`
- No shell injection vulnerabilities (use list args, not strings)
- Subprocess timeouts to prevent hangs

**Permissions:**
- No root/sudo required for normal operation
- Desktop shortcut installation uses user directories only
- System package installation (dev-setup.sh) requires sudo (interactive prompt)

**Best Practices:**
- Validate all user input (version tags, paths)
- Sanitize file paths (no `..` traversal)
- Use subprocess with list args: `["pip", "install", package]` not `f"pip install {package}"`
- Never execute arbitrary code from network sources

## Building for Distribution

### Creating a Release

**1. Update Version Numbers** (if applicable):
```python
# backend/config.py
VERSION = "1.0.0"  # Update this
```

**2. Test the Build:**
```bash
scripts/dev/build.sh
./dist/comfyui-setup
```

**3. Test All Features:**
- Install a ComfyUI version
- Switch between versions
- Create desktop/menu shortcuts
- Launch ComfyUI
- Cancel an installation mid-progress
- Check logs for errors

**4. Test on Clean System:**
- Use a VM or fresh Linux Mint/Ubuntu install
- Copy only the executable (no source code)
- Verify it runs without any dependencies
- Test all features work

**5. Create Release Archive:**
```bash
cd dist
tar -czf comfyui-launcher-linux-x64-v1.0.0.tar.gz comfyui-setup
sha256sum comfyui-launcher-linux-x64-v1.0.0.tar.gz > checksums.txt
```

**6. Create GitHub Release:**
- Tag the commit: `git tag -a v1.0.0 -m "Release v1.0.0"`
- Push tags: `git push origin v1.0.0`
- Create release on GitHub
- Attach `.tar.gz` and `checksums.txt`
- Write release notes highlighting new features/fixes

### Distribution Checklist

**Pre-Release:**
- [ ] Update version in [backend/config.py](backend/config.py)
- [ ] Run full build: `scripts/dev/build.sh`
- [ ] Test on development system
- [ ] Test on clean VM (Ubuntu, Linux Mint, Debian)
- [ ] Verify executable runs without dependencies
- [ ] Check file permissions: `chmod +x comfyui-setup`
- [ ] Test all core features:
  - [ ] Version installation (full cycle)
  - [ ] Installation cancellation
  - [ ] Version switching
  - [ ] Desktop shortcuts
  - [ ] Menu shortcuts
  - [ ] Launch ComfyUI
  - [ ] Size calculation
  - [ ] Pre-release filtering
- [ ] Check logs for errors in `launcher-data/logs/`
- [ ] Verify symlinks created correctly

**Release:**
- [ ] Create release archive with version number
- [ ] Generate SHA256 checksum
- [ ] Tag release in git: `v1.0.0`
- [ ] Create GitHub release with notes
- [ ] Update [README.md](README.md) with download link
- [ ] Test download link works

**Post-Release:**
- [ ] Monitor for issues
- [ ] Respond to bug reports promptly
- [ ] Document known issues

## Contributing

### Development Process

1. **Fork and Clone:**
   ```bash
   git clone <your-fork-url>
   cd Linux-ComfyUI-Launcher
   ```

2. **Set Up Environment:**
   ```bash
   scripts/dev/setup.sh
   ```

3. **Create Feature Branch:**
   ```bash
   git checkout -b feature/my-new-feature
   ```

4. **Make Changes:**
   - Follow code style guidelines
   - Add comments for complex logic
   - Update documentation if needed

5. **Test Your Changes:**
   - Test in development mode (two terminals)
   - Test production build: `scripts/dev/build.sh && ./dist/comfyui-setup`
   - Verify no regressions

6. **Commit and Push:**
   ```bash
   git add .
   git commit -m "Add feature: description of changes"
   git push origin feature/my-new-feature
   ```

7. **Submit Pull Request:**
   - Describe what changed and why
   - Reference any related issues
   - Include screenshots/logs if relevant

### Testing Checklist

**Manual Testing** (before submitting PR):
- [ ] All UI buttons respond correctly
- [ ] Status updates in real-time (500ms polling works)
- [ ] Version installation completes successfully
- [ ] Installation progress shows all 5 stages
- [ ] Installation cancellation works and cleans up
- [ ] Size calculation displays correctly
- [ ] Version switching updates active version
- [ ] Desktop shortcuts create/remove correctly
- [ ] Menu shortcuts create/remove correctly
- [ ] Launch button starts ComfyUI and opens browser
- [ ] Window closes properly (no zombie processes)
- [ ] Production build works (test with `./dist/comfyui-setup`)
- [ ] No console errors in browser DevTools (F12)
- [ ] No Python exceptions in terminal

**Automated Testing** (future):
- Unit tests for version_manager.py
- Integration tests for API methods
- UI component tests

### Code Review Guidelines

**For Contributors:**
- Keep PRs focused (one feature/fix per PR)
- Write clear commit messages
- Update documentation for API changes
- Add comments for non-obvious code

**For Reviewers:**
- Check code follows style guidelines
- Verify no security issues (input validation, subprocess args)
- Test the changes locally
- Ensure documentation is updated

## License

MIT License - See [LICENSE](LICENSE) file for details.

## Credits

**Built with:**
- [PyWebView](https://pywebview.flowrl.com/) - Python desktop GUI framework
- [React](https://react.dev/) - Frontend UI library
- [Vite](https://vite.dev/) - Fast build tool
- [Framer Motion](https://www.framer.com/motion/) - Animation library
- [PyInstaller](https://pyinstaller.org/) - Python packaging
- [Tailwind CSS](https://tailwindcss.com/) - Utility-first CSS
- [Lucide Icons](https://lucide.dev/) - Beautiful icon set

**ComfyUI** by [comfyanonymous](https://github.com/comfyanonymous/ComfyUI)

## Support

**For Users:**
- Check [README.md](README.md) for user documentation
- Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common issues
- Open an issue on GitHub with:
  - Linux distribution and version
  - Launcher version
  - Log files from `launcher-data/logs/`
  - Steps to reproduce the issue

**For Developers:**
- Check this document for development setup
- Open an issue for bug reports or feature requests
- Submit PRs for contributions
- Discuss major changes in an issue first

---

**Built with care for the ComfyUI community.**
