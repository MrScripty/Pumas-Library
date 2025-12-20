# ComfyUI Version Manager - Implementation Plan

**Project**: Linux ComfyUI Launcher - Multi-Version Management System
**Date**: 2025-12-19
**Last Updated**: 2025-12-19
**Purpose**: Enable installation, management, and switching between multiple ComfyUI versions with shared resources

---

## Implementation Progress

### Completed Phases

✅ **Phase 1: Foundation** (Completed 2025-12-19)
- Metadata management system with JSON storage
- Data models for versions, models, custom nodes, workflows
- Utility functions for path resolution and file operations
- All tests passing

✅ **Phase 2: GitHub Integration** (Completed 2025-12-19)
- GitHub releases API integration with caching (1-hour TTL)
- Release fetching with pagination support
- Download manager with progress tracking and retry logic
- Rate limiting and error handling
- All tests passing

✅ **Phase 3: Resource Manager** (Completed 2025-12-19)
- Shared storage initialization and management
- Symlink creation and validation
- Model management (add, remove, scan)
- Custom node management (install, update, remove)
- Workflow migration support
- Per-version custom node directories
- All tests passing

✅ **Phase 4: Version Manager** (Completed 2025-12-19)
- Version installation from GitHub releases
- Archive extraction (zip/tarball support)
- Virtual environment creation with UV
- Dependency management and checking
- Version switching with symlink validation
- Version removal with safety checks
- Non-blocking version launching
- All tests passing

✅ **Phase 5: Backend API Integration** (Completed 2025-12-19)
- PyWebView API exposure of all version management features
- 17 API methods for version and resource management
- JavaScript bridge integration via JavaScriptAPI class
- Comprehensive error handling and fallback values
- All tests passing

### In Progress

🔄 **Phase 6: Frontend UI Components** (Not Started)
- React components for version management
- Installation dialogs and progress indicators
- Resource browsers for models, custom nodes, workflows
- Custom node manager UI
- Dependency check UI

### Planned

📋 **Phase 7: Migration Tool** (Not Started)
- Detection of existing ComfyUI installations
- Migration preview and execution
- Edge case handling
- Rollback capability

📋 **Phase 8: Testing & Polish** (Not Started)
- End-to-end testing
- UI/UX polish
- Documentation
- Performance testing

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Directory Structure](#directory-structure)
4. [Data Models & Metadata](#data-models--metadata)
5. [Core Systems](#core-systems)
6. [Implementation Phases](#implementation-phases)
7. [UI/UX Design](#uiux-design)
8. [Technical Specifications](#technical-specifications)
9. [Migration & Compatibility](#migration--compatibility)
10. [Testing Strategy](#testing-strategy)

---

## Overview

### Goals

1. **Version Management**: Install multiple ComfyUI versions side-by-side without conflicts
2. **Resource Sharing**: Share models, custom nodes, and user data across versions to save disk space
3. **Version Selection**: Allow users to choose which version to run (only one at a time)
4. **Persistence**: Preserve models and custom nodes when changing versions
5. **Compatibility Tracking**: Track which resources work with which versions
6. **Migration**: Seamlessly migrate existing ComfyUI installations

### Key Design Decisions

- **Multiple Parallel Installations**: Each version in its own directory
- **Single Active Runtime**: Only one version runs at a time
- **Shared Model Storage**: Models live in shared storage, symlinked into versions to save disk space
- **Per-Version Custom Nodes**: Each ComfyUI version has its own isolated custom node installations (snapshots)
- **Per-Version Virtual Environments**: Isolate Python dependencies using UV
- **Per-Version Custom Node Configuration**: Enable/disable nodes per version
- **Launcher-Relative Paths**: All managed directories live within the launcher directory
- **Dependency Conflict Prevention**: Automatic detection and prevention of incompatible custom node dependencies
- **UV Package Manager**: Use UV instead of pip for faster, more reliable dependency management

---

## Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────────┐
│                    React Frontend                        │
│  - Version Selector                                      │
│  - Installation Manager                                  │
│  - Resource Browser (Models, Custom Nodes, Workflows)   │
│  - Settings & Configuration                             │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│              PyWebView Bridge (main.py)                  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                  Backend API (api.py)                    │
│                                                          │
│  ┌──────────────────┬──────────────────┬──────────────┐ │
│  │ Version Manager  │ Resource Manager │ Git/GitHub   │ │
│  │                  │                  │ Integration  │ │
│  │ - Install        │ - Models         │              │ │
│  │ - Switch         │ - Custom Nodes   │ - Fetch      │ │
│  │ - Remove         │ - Workflows      │   releases   │ │
│  │ - Launch         │ - Symlinks       │ - Download   │ │
│  │ - UV Integration │ - Compatibility  │              │ │
│  └──────────────────┴──────────────────┴──────────────┘ │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │            Metadata Management                     │ │
│  │  - versions.json                                   │ │
│  │  - models.json                                     │ │
│  │  - custom_nodes.json                              │ │
│  │  - workflows.json                                 │ │
│  │  - version-configs/*.json                         │ │
│  └────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                   File System                            │
│                                                          │
│  comfyui-versions/     shared-resources/                │
│  launcher-data/                                         │
└─────────────────────────────────────────────────────────┘
```

---

## Directory Structure

### Complete Directory Layout

```
/path/to/Linux-ComfyUI-Launcher/          # Launcher root
├── backend/
│   ├── main.py                            # PyWebView entry point
│   ├── api.py                             # Main API class (existing)
│   ├── version_manager.py                 # NEW: Version management
│   ├── resource_manager.py                # NEW: Resource/symlink management
│   ├── github_api.py                      # NEW: GitHub integration
│   └── metadata_manager.py                # NEW: Metadata handling
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   │   ├── VersionSelector.tsx        # NEW: Version dropdown
│   │   │   ├── InstallDialog.tsx          # NEW: Install new version
│   │   │   ├── ResourceBrowser.tsx        # NEW: Browse models/nodes
│   │   │   ├── CustomNodeManager.tsx      # NEW: Manage custom nodes
│   │   │   └── ...
│   │   └── ...
│   └── ...
├── comfyui-versions/                      # NEW: All ComfyUI versions
│   ├── v0.2.0/                            # Full ComfyUI installation
│   │   ├── venv/                          # Per-version Python venv (UV-managed)
│   │   ├── main.py
│   │   ├── models/                        # Symlinks → shared-resources/models/
│   │   ├── custom_nodes/                  # Per-version custom node snapshots
│   │   │   ├── ComfyUI-Manager/           # Real files (git clone), isolated per version
│   │   │   └── ...
│   │   ├── user/                          # Symlinks → shared-resources/user/
│   │   └── ...
│   ├── v0.2.1/
│   │   ├── venv/
│   │   ├── custom_nodes/                  # Independent custom node snapshots
│   │   │   ├── ComfyUI-Manager/           # May be different version than v0.2.0
│   │   │   └── ...
│   │   └── ...
│   ├── v0.3.0/
│   └── .active-version                    # Tracks selected version
├── shared-resources/                      # NEW: Shared resource storage
│   ├── models/                            # Source of truth for models (shared)
│   │   ├── checkpoints/
│   │   ├── loras/
│   │   ├── vae/
│   │   ├── controlnet/
│   │   ├── clip/
│   │   ├── clip_vision/
│   │   ├── unet/
│   │   ├── diffusion_models/
│   │   ├── embeddings/
│   │   ├── upscale_models/
│   │   └── ...                            # Dynamically discovered from ComfyUI
│   ├── custom_nodes_cache/                # NEW: Git repos cached for faster cloning
│   │   ├── ComfyUI-Manager.git/           # Bare git repo
│   │   ├── ComfyUI-Advanced-ControlNet.git/
│   │   └── ...
│   └── user/                              # Source of truth for user data (shared)
│       ├── workflows/
│       ├── settings/
│       └── ...
├── launcher-data/                         # NEW: Launcher metadata & config
│   ├── metadata/
│   │   ├── versions.json                  # Installed versions
│   │   ├── models.json                    # Model metadata
│   │   ├── custom_nodes.json              # Custom node metadata
│   │   └── workflows.json                 # Workflow metadata
│   ├── config/
│   │   ├── launcher-settings.json         # Launcher preferences
│   │   └── version-configs/
│   │       ├── v0.2.0-config.json         # Per-version config
│   │       ├── v0.2.1-config.json
│   │       └── v0.3.0-config.json
│   └── cache/
│       └── github-releases.json           # Cached release list
├── comfyui-icon.webp                      # Existing
├── run.sh                                 # Existing
├── build.sh                               # Existing
└── ...
```

---

## Data Models & Metadata

### 1. versions.json

Tracks all installed ComfyUI versions.

```json
{
  "installed": {
    "v0.2.0": {
      "path": "comfyui-versions/v0.2.0",
      "installedDate": "2025-01-15T10:30:00Z",
      "pythonVersion": "3.11.5",
      "gitCommit": "abc123def456...",
      "releaseTag": "v0.2.0",
      "releaseDate": "2025-01-10T00:00:00Z",
      "releaseNotes": "Bug fixes and performance improvements",
      "downloadUrl": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.2.0.tar.gz",
      "size": 1250000,
      "requirementsHash": "sha256:abc123...",
      "dependenciesInstalled": true
    },
    "v0.2.1": {
      "path": "comfyui-versions/v0.2.1",
      "installedDate": "2025-01-20T14:15:00Z",
      "pythonVersion": "3.11.5",
      "gitCommit": "def456ghi789...",
      "releaseTag": "v0.2.1",
      "releaseDate": "2025-01-18T00:00:00Z",
      "releaseNotes": "Added new nodes for better workflow control",
      "downloadUrl": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.2.1.tar.gz",
      "size": 1260000,
      "requirementsHash": "sha256:def456...",
      "dependenciesInstalled": true
    }
  },
  "lastSelectedVersion": "v0.2.1",
  "defaultVersion": "v0.2.1"
}
```

### 2. version-configs/vX.X.X-config.json

Per-version configuration (custom nodes, launch args, etc.).

```json
{
  "version": "v0.2.0",
  "customNodes": {
    "ComfyUI-Manager": {
      "enabled": true,
      "gitCommit": "abc123def456",
      "gitTag": "v1.2.0",
      "installDate": "2025-01-15T10:35:00Z",
      "compatibilityStatus": "compatible",
      "requirementsInstalled": true
    },
    "ComfyUI-Advanced-ControlNet": {
      "enabled": false,
      "gitCommit": "def456ghi789",
      "gitTag": "v2.0.1",
      "installDate": "2025-01-15T10:40:00Z",
      "compatibilityStatus": "incompatible",
      "incompatibilityReason": "Requires torch>=2.2.0, ComfyUI requires torch<2.2.0",
      "conflictingPackages": ["torch"],
      "requirementsInstalled": false
    }
  },
  "launchArgs": ["--listen", "0.0.0.0", "--port", "8188"],
  "pythonPath": "comfyui-versions/v0.2.0/venv/bin/python",
  "uvPath": "comfyui-versions/v0.2.0/venv/bin/uv",
  "requirements": {
    "torch": "2.1.0",
    "torchvision": "0.16.0",
    "torchaudio": "2.1.0",
    "...": "..."
  },
  "requirementsHash": "sha256:abc123..."
}
```

### 3. custom_nodes.json

Global metadata about custom nodes (cached git repos for faster installation).

```json
{
  "ComfyUI-Manager": {
    "cacheRepo": "shared-resources/custom_nodes_cache/ComfyUI-Manager.git",
    "gitUrl": "https://github.com/ltdrdata/ComfyUI-Manager.git",
    "lastFetched": "2025-01-15T12:00:00Z",
    "availableTags": ["v1.0.0", "v1.1.0", "v1.2.0"],
    "latestCommit": "abc123def456",
    "hasRequirements": true,
    "tags": ["manager", "essential"],
    "description": "ComfyUI node manager for installing and managing custom nodes",
    "compatibilityCache": {
      "v0.2.0": {
        "status": "compatible",
        "checkedAt": "2025-01-15T10:00:00Z",
        "requirementsHash": "sha256:abc123..."
      },
      "v0.2.1": {
        "status": "compatible",
        "checkedAt": "2025-01-15T10:05:00Z",
        "requirementsHash": "sha256:abc123..."
      }
    }
  },
  "ComfyUI-Advanced-ControlNet": {
    "cacheRepo": "shared-resources/custom_nodes_cache/ComfyUI-Advanced-ControlNet.git",
    "gitUrl": "https://github.com/Kosinkadink/ComfyUI-Advanced-ControlNet.git",
    "lastFetched": "2025-01-12T10:00:00Z",
    "availableTags": ["v2.0.0", "v2.0.1"],
    "latestCommit": "def456ghi789",
    "hasRequirements": true,
    "tags": ["controlnet", "advanced"],
    "description": "Advanced ControlNet implementation for ComfyUI",
    "compatibilityCache": {
      "v0.2.0": {
        "status": "incompatible",
        "reason": "Requires torch>=2.2.0, ComfyUI requires torch<2.2.0",
        "conflictingPackages": ["torch"],
        "checkedAt": "2025-01-12T10:00:00Z",
        "requirementsHash": "sha256:def456..."
      },
      "v0.2.1": {
        "status": "compatible",
        "additionalRequirements": ["opencv-python>=4.5.0"],
        "checkedAt": "2025-01-12T10:05:00Z",
        "requirementsHash": "sha256:def456..."
      }
    }
  }
}
```

### 4. models.json

Metadata about models in shared storage.

```json
{
  "checkpoints/sd_xl_base_1.0.safetensors": {
    "path": "shared-resources/models/checkpoints/sd_xl_base_1.0.safetensors",
    "size": 6938078208,
    "sha256": "31e35c80fc4829d14f90153f4c74cd59c90b779f6afe05a74cd6120b893f7e5b",
    "addedDate": "2025-01-10T08:00:00Z",
    "lastUsed": "2025-01-20T15:30:00Z",
    "tags": ["sdxl", "base", "checkpoint"],
    "modelType": "checkpoint",
    "resolution": "1024x1024",
    "usedByVersions": ["v0.2.0", "v0.2.1"],
    "source": "manual"
  },
  "loras/my_custom_lora.safetensors": {
    "path": "shared-resources/models/loras/my_custom_lora.safetensors",
    "size": 144000000,
    "sha256": "abc123...",
    "addedDate": "2025-01-15T14:00:00Z",
    "lastUsed": "2025-01-20T16:00:00Z",
    "tags": ["lora", "custom"],
    "modelType": "lora",
    "baseModel": "sd_xl_base_1.0",
    "usedByVersions": ["v0.2.1"],
    "source": "civitai"
  }
}
```

### 5. workflows.json

Metadata about user workflows.

```json
{
  "my_workflow_v1.json": {
    "path": "shared-resources/user/workflows/my_workflow_v1.json",
    "createdDate": "2025-01-12T10:00:00Z",
    "modifiedDate": "2025-01-18T14:30:00Z",
    "usedByVersions": ["v0.2.0", "v0.2.1"],
    "tags": ["sdxl", "img2img"],
    "description": "My custom SDXL workflow for image-to-image generation",
    "requiredNodes": ["ComfyUI-Manager", "ComfyUI-Advanced-ControlNet"],
    "requiredModels": ["checkpoints/sd_xl_base_1.0.safetensors"]
  }
}
```

### 6. github-releases.json (cached)

Cached GitHub releases to avoid repeated API calls.

```json
{
  "lastFetched": "2025-01-20T12:00:00Z",
  "ttl": 86400,
  "releases": [
    {
      "tag_name": "v0.3.0",
      "name": "ComfyUI v0.3.0",
      "published_at": "2025-01-25T00:00:00Z",
      "body": "### New Features\n- Added XYZ plot node\n- Improved performance\n\n### Bug Fixes\n- Fixed memory leak",
      "tarball_url": "https://api.github.com/repos/comfyanonymous/ComfyUI/tarball/v0.3.0",
      "zipball_url": "https://api.github.com/repos/comfyanonymous/ComfyUI/zipball/v0.3.0",
      "prerelease": false,
      "assets": []
    },
    {
      "tag_name": "v0.2.1",
      "name": "ComfyUI v0.2.1",
      "published_at": "2025-01-18T00:00:00Z",
      "body": "Bug fixes and stability improvements",
      "tarball_url": "https://api.github.com/repos/comfyanonymous/ComfyUI/tarball/v0.2.1",
      "zipball_url": "https://api.github.com/repos/comfyanonymous/ComfyUI/zipball/v0.2.1",
      "prerelease": false,
      "assets": []
    }
  ]
}
```

---

## Core Systems

### 1. Version Manager

**Module**: `backend/version_manager.py`

**Responsibilities**:
- Fetch available releases from GitHub
- Download and extract ComfyUI versions
- Install version dependencies using UV package manager
- Create per-version Python virtual environments with UV
- Switch active version
- Remove installed versions
- Launch selected version
- Track version metadata
- Pre-installation compatibility checking

**Key Methods**:

```python
class VersionManager:
    def __init__(self, launcher_root: Path):
        self.launcher_root = launcher_root
        self.versions_dir = launcher_root / "comfyui-versions"
        self.metadata_file = launcher_root / "launcher-data/metadata/versions.json"

    def fetch_releases(self, force_refresh=False) -> List[Release]:
        """Fetch available releases from GitHub (with caching)"""

    def check_version_compatibility(self, tag: str) -> CompatibilityReport:
        """Check compatibility before installing a version"""
        # 1. Fetch requirements.txt from GitHub for this tag
        # 2. Check against all installed custom nodes
        # 3. Return compatibility report
        # 4. Cache results

    def install_version(self, tag: str, progress_callback=None) -> bool:
        """Download and install a specific version"""
        # 1. Check compatibility first
        # 2. Download tarball/zipball
        # 3. Extract to comfyui-versions/{tag}/
        # 4. Create Python venv with UV
        # 5. Read requirements.txt
        # 6. Install dependencies using UV
        # 7. Update versions.json metadata
        # 8. Create initial version-config
        # 9. Trigger resource manager to setup symlinks

    def remove_version(self, tag: str) -> bool:
        """Remove an installed version"""
        # Remove entire comfyui-versions/{tag}/ directory

    def get_installed_versions(self) -> List[str]:
        """Get list of installed version tags"""

    def get_active_version(self) -> Optional[str]:
        """Get currently selected version"""

    def set_active_version(self, tag: str) -> bool:
        """Switch to a different version"""
        # 1. Validate symlinks for target version
        # 2. Update .active-version file
        # 3. Update versions.json lastSelectedVersion

    def check_dependencies(self, tag: str) -> DependencyStatus:
        """Check if version's requirements.txt dependencies are installed"""
        # Read comfyui-versions/{tag}/requirements.txt
        # Use UV to check if packages are installed in venv
        # Return status with missing packages

    def install_dependencies(self, tag: str, progress_callback=None) -> bool:
        """Install missing dependencies for a version"""
        # Run: uv pip install -r requirements.txt in version's venv

    def launch_version(self, tag: str) -> bool:
        """Launch a specific ComfyUI version"""
        # 1. Set as active version
        # 2. Ensure dependencies installed
        # 3. Ensure symlinks are current
        # 4. Launch main.py in version's venv
```

### 2. Resource Manager

**Module**: `backend/resource_manager.py`

**Responsibilities**:
- Manage shared-resources directory structure
- Create and maintain symlinks for models and user data
- Manage per-version custom node installations (isolated snapshots)
- Maintain custom node git repository cache
- Migrate real files from version dirs to shared storage
- Scan for new models and dynamically discover model directories
- Handle file operations (add, remove, move)
- Validate and repair broken symlinks
- Check custom node dependency compatibility

**Key Methods**:

```python
class ResourceManager:
    def __init__(self, launcher_root: Path):
        self.launcher_root = launcher_root
        self.shared_dir = launcher_root / "shared-resources"
        self.versions_dir = launcher_root / "comfyui-versions"

    def initialize_shared_storage(self):
        """Create shared-resources directory structure"""
        # Create models/ directory (subdirs created dynamically)
        # Create custom_nodes_cache/ for bare git repos
        # Create user/workflows, user/settings

    def discover_model_directories(self, comfyui_path: Path) -> List[str]:
        """Discover model directories from ComfyUI installation"""
        # Parse folder_paths.py to find model directories
        # Return list of directory names
        # Used to dynamically sync shared model structure

    def sync_shared_model_structure(self, comfyui_version: str):
        """Ensure shared models has all directories from this ComfyUI version"""
        # Discover model dirs from ComfyUI version
        # Create any missing directories in shared-resources/models/
        # NEVER remove directories (preserve models for other versions)

    def setup_version_symlinks(self, version_tag: str):
        """Setup all symlinks for a version"""
        # 1. Symlink models (only categories this version uses)
        # 2. Symlink user data
        # NOTE: Custom nodes are NOT symlinked - they are real files per version

    def validate_and_repair_symlinks(self, version_tag: str) -> RepairReport:
        """Check for broken symlinks and attempt repair"""
        # Find all symlinks in version directory
        # Check if each symlink target exists
        # Recreate broken symlinks if target exists in shared storage
        # Remove broken symlinks if target no longer exists
        # Return report of repairs

    def migrate_existing_files(self, version_path: Path):
        """Scan version directory for real files and move to shared storage"""
        # Find all real files in models/, user/
        # For each real file:
        #   - If doesn't exist in shared: move to shared
        #   - If exists in shared: prompt user (keep both, replace, skip)
        #   - Create symlink in original location
        # Custom nodes are left as-is (per-version snapshots)

    def add_model(self, source_path: Path, category: str) -> bool:
        """Add a model to shared storage"""
        # Copy to shared-resources/models/{category}/
        # Update models.json metadata
        # Return success

    def remove_model(self, model_path: str) -> bool:
        """Remove a model from shared storage"""
        # Delete from shared-resources/models/
        # Update models.json metadata

    def check_custom_node_compatibility(
        self, node_name: str, comfyui_version: str
    ) -> CompatibilityStatus:
        """Check if custom node is compatible with ComfyUI version"""
        # 1. Read custom node requirements.txt
        # 2. Read ComfyUI version requirements.txt
        # 3. Parse and compare version specifiers
        # 4. Detect conflicts (e.g., torch>=2.2 vs torch<2.2)
        # 5. Return compatibility status with details
        # 6. Cache result in custom_nodes.json

    def install_custom_node(
        self, git_url: str, version: str, commit_or_tag: Optional[str] = None
    ) -> bool:
        """Install a custom node for a specific ComfyUI version"""
        # 1. Clone/update bare repo to custom_nodes_cache/
        # 2. Check compatibility with ComfyUI version
        # 3. If incompatible, return error with details
        # 4. Clone specific commit/tag to comfyui-versions/{version}/custom_nodes/
        # 5. Install node requirements using UV (if compatible)
        # 6. Update version config metadata
        # 7. Update custom_nodes.json cache

    def update_custom_node(
        self, node_name: str, version: str, commit_or_tag: Optional[str] = None
    ) -> bool:
        """Update a custom node to a different version/commit"""
        # 1. Fetch latest from cached repo
        # 2. Check compatibility of new version
        # 3. If incompatible, warn and abort
        # 4. Pull/checkout in comfyui-versions/{version}/custom_nodes/{node_name}
        # 5. Update requirements if changed
        # 6. Update metadata

    def remove_custom_node(self, node_name: str, version: str) -> bool:
        """Remove a custom node from a specific ComfyUI version"""
        # Remove from comfyui-versions/{version}/custom_nodes/{node_name}
        # Update version config

    def scan_shared_storage(self) -> ScanResult:
        """Scan shared storage and update metadata"""
        # Scan models/ directory
        # Update models.json metadata
        # Return summary
```

### 3. GitHub API

**Module**: `backend/github_api.py`

**Responsibilities**:
- Fetch ComfyUI releases from GitHub API
- Download release archives
- Cache release data
- Handle rate limiting

**Key Methods**:

```python
class GitHubAPI:
    REPO_OWNER = "comfyanonymous"
    REPO_NAME = "ComfyUI"
    API_BASE = "https://api.github.com"

    def __init__(self, cache_dir: Path):
        self.cache_file = cache_dir / "github-releases.json"
        self.cache_ttl = 86400  # 24 hours

    def fetch_releases(self, force_refresh=False) -> List[Release]:
        """Fetch releases with caching"""
        # Check cache first
        # If expired or force_refresh, fetch from API
        # Cache result
        # Return releases

    def download_release(self, tag: str, dest_dir: Path, progress_callback=None) -> Path:
        """Download and extract a release"""
        # Download tarball
        # Extract to dest_dir
        # Return extracted path

    def get_latest_release(self) -> Optional[Release]:
        """Get the latest non-prerelease version"""
```

### 4. Metadata Manager

**Module**: `backend/metadata_manager.py`

**Responsibilities**:
- Read/write metadata JSON files
- Validate metadata schemas
- Provide atomic updates
- Handle migration of old metadata formats

**Key Methods**:

```python
class MetadataManager:
    def __init__(self, launcher_data_dir: Path):
        self.metadata_dir = launcher_data_dir / "metadata"
        self.config_dir = launcher_data_dir / "config"

    def load_versions(self) -> VersionsMetadata:
        """Load versions.json"""

    def save_versions(self, data: VersionsMetadata):
        """Save versions.json atomically"""

    def load_version_config(self, tag: str) -> VersionConfig:
        """Load version-specific config"""

    def save_version_config(self, tag: str, data: VersionConfig):
        """Save version-specific config"""

    def load_models(self) -> ModelsMetadata:
        """Load models.json"""

    def save_models(self, data: ModelsMetadata):
        """Save models.json"""

    def load_custom_nodes(self) -> CustomNodesMetadata:
        """Load custom_nodes.json"""

    def save_custom_nodes(self, data: CustomNodesMetadata):
        """Save custom_nodes.json"""
```

---

## Implementation Phases

### Phase 1: Foundation & Infrastructure

**Goal**: Setup directory structure, metadata system, and core utilities

**Tasks**:
1. Create directory structure (comfyui-versions/, shared-resources/, launcher-data/)
2. Implement MetadataManager class
3. Implement data models for all metadata files
4. Create utility functions for path resolution
5. Write unit tests for metadata operations

**Files Created**:
- `backend/metadata_manager.py`
- `backend/models.py` (data classes)
- `backend/utils.py` (utilities)

**Estimated Complexity**: Medium

---

### Phase 2: GitHub Integration

**Goal**: Fetch and cache ComfyUI releases from GitHub

**Tasks**:
1. Implement GitHubAPI class
2. Add release caching with TTL
3. Implement download with progress tracking
4. Handle rate limiting and errors
5. Write tests for GitHub integration

**Files Created**:
- `backend/github_api.py`

**Dependencies**: Phase 1

**Estimated Complexity**: Medium

---

### Phase 3: Resource Manager

**Goal**: Manage shared storage and symlinks

**Tasks**:
1. Implement ResourceManager class
2. Create shared storage initialization
3. Implement symlink creation/removal
4. Implement file migration from version dirs to shared storage
5. Add model management (add, remove, scan)
6. Add custom node management (install, enable, disable)
7. Write tests for resource operations

**Files Created**:
- `backend/resource_manager.py`

**Dependencies**: Phase 1

**Estimated Complexity**: High (symlink logic is complex)

---

### Phase 4: Version Manager

**Goal**: Install, manage, and launch ComfyUI versions

**Tasks**:
1. Implement VersionManager class
2. Add version installation (download, extract, setup venv)
3. Add dependency checking (parse requirements.txt, check venv)
4. Add dependency installation (pip install in venv)
5. Add version switching (update active version, rebuild symlinks)
6. Add version removal
7. Add version launching
8. Write tests for version operations

**Files Created**:
- `backend/version_manager.py`

**Dependencies**: Phase 2, Phase 3

**Estimated Complexity**: High (many moving parts)

---

### Phase 5: Backend API Integration

**Goal**: Expose version management to frontend via PyWebView API

**Tasks**:
1. Update `backend/api.py` ComfyUISetupAPI class with new methods:
   - `get_available_versions()` - fetch from GitHub
   - `get_installed_versions()` - list installed
   - `install_version(tag)` - install new version
   - `remove_version(tag)` - remove version
   - `switch_version(tag)` - change active version
   - `check_version_dependencies(tag)` - check if deps installed
   - `install_version_dependencies(tag)` - install missing deps
   - `get_version_status()` - get comprehensive status
   - `get_models()` - list models in shared storage
   - `get_custom_nodes()` - list custom nodes
   - `enable_custom_node(node, version)` - enable node for version
   - `disable_custom_node(node, version)` - disable node
   - `install_custom_node(git_url, versions)` - install new node
2. Update `backend/main.py` JavaScriptAPI class to expose new methods
3. Add progress callback support for long operations
4. Write integration tests

**Files Modified**:
- `backend/api.py`
- `backend/main.py`

**Dependencies**: Phase 4

**Estimated Complexity**: Medium

---

### Phase 6: Frontend UI Components

**Goal**: Build React UI for version management

**Tasks**:

#### 6.1: Version Selector Component
- Dropdown showing installed versions
- Refresh button (icon) next to version display to fetch updates
- Shows currently active version
- Allows switching versions

#### 6.2: Install Dialog Component
- Download icon button to open dialog
- Fetches available releases from GitHub
- Displays list with version, date, and release notes
- Install button with progress indicator
- Filters (show pre-releases, show installed)

#### 6.3: Version Manager View
- Table of installed versions with:
  - Version tag
  - Install date
  - Size on disk
  - Dependency status
  - Actions: Launch, Configure, Remove
- "Check for Updates" status indicator
- Shows if newer version available

#### 6.4: Resource Browser Component
- Tabbed interface:
  - Models tab: Browse models by category
  - Custom Nodes tab: Manage custom nodes
  - Workflows tab: Browse workflows
- Add/remove/import functionality
- Search and filter

#### 6.5: Custom Node Manager Component
- List of all custom nodes in shared storage
- Per-version enable/disable toggles
- Install new node (from git URL)
- Update existing nodes (git pull)
- Show compatibility status per version

#### 6.6: Dependency Check Component
- Shows dependency status for active version
- Lists missing packages
- Install button with progress

**Files Created**:
- `frontend/src/components/VersionSelector.tsx`
- `frontend/src/components/InstallDialog.tsx`
- `frontend/src/components/VersionManager.tsx`
- `frontend/src/components/ResourceBrowser.tsx`
- `frontend/src/components/CustomNodeManager.tsx`
- `frontend/src/components/DependencyCheck.tsx`
- `frontend/src/hooks/useVersions.ts`
- `frontend/src/hooks/useResources.ts`

**Dependencies**: Phase 5

**Estimated Complexity**: High (lots of UI work)

---

### Phase 7: Migration Tool

**Goal**: Detect and migrate existing ComfyUI installations

**Tasks**:
1. Implement detection of existing ComfyUI in common locations
2. Build migration preview UI showing what will be migrated
3. Implement migration process:
   - Detect version
   - Move installation to comfyui-versions/{version}/
   - Scan for models, custom nodes, workflows
   - Move to shared storage
   - Create symlinks
   - Update metadata
4. Handle edge cases (missing files, permission errors)
5. Add rollback capability

**Files Created**:
- `backend/migration_manager.py`
- `frontend/src/components/MigrationDialog.tsx`

**Files Modified**:
- `backend/api.py` (add migration methods)
- `backend/main.py` (expose migration API)

**Dependencies**: Phase 4, Phase 6

**Estimated Complexity**: High (many edge cases)

---

### Phase 8: Testing & Polish

**Goal**: Comprehensive testing and bug fixes

**Tasks**:
1. End-to-end testing
2. Test migration with real ComfyUI installation
3. Test version switching
4. Test custom node enable/disable
5. Test dependency installation
6. Test error handling and edge cases
7. Performance testing (large model libraries)
8. UI/UX polish
9. Documentation

**Dependencies**: All previous phases

**Estimated Complexity**: Medium-High

---

## UI/UX Design

### Main Dashboard Layout

```
┌─────────────────────────────────────────────────────────┐
│  ComfyUI Launcher                               [X]      │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  Active Version: [v0.2.1 ▼]  [🔄]  [⬇]                 │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │ Status: Ready to launch                            │ │
│  │ Dependencies: ✓ All installed                      │ │
│  │ Update Available: v0.3.0                           │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  [ Launch ComfyUI ]        [ Stop ]                     │
│                                                          │
│  ┌─ Installed Versions ──────────────────────────────┐ │
│  │                                                    │ │
│  │  v0.3.0    Jan 25  1.2GB  ✓  [Launch] [⚙] [🗑]   │ │
│  │  v0.2.1    Jan 20  1.2GB  ✓  [Launch] [⚙] [🗑]   │ │
│  │  v0.2.0    Jan 15  1.2GB  ✓  [Launch] [⚙] [🗑]   │ │
│  │                                                    │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌─ Tabs ──────────────────────────────────────────┐   │
│  │ [Models] [Custom Nodes] [Workflows] [Settings]  │   │
│  │                                                  │   │
│  │  [Content based on selected tab]                │   │
│  │                                                  │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

**Legend**:
- `[v0.2.1 ▼]` - Version dropdown selector
- `[🔄]` - Refresh releases from GitHub
- `[⬇]` - Download/install new version
- `[⚙]` - Configure version (custom nodes, launch args)
- `[🗑]` - Delete version

### Install New Version Dialog

```
┌─────────────────────────────────────────────────────────┐
│  Install ComfyUI Version                         [X]     │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  Available Releases:                                     │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │ v0.3.0 - Jan 25, 2025                 [Install]    │ │
│  │ ─────────────────────────────────────────────────  │ │
│  │ ### New Features                                   │ │
│  │ - Added XYZ plot node                              │ │
│  │ - Improved performance                             │ │
│  │                                                    │ │
│  │ Custom Node Compatibility:                         │ │
│  │ ✓ ComfyUI-Manager (compatible)                     │ │
│  │ ✓ WAS-Node-Suite (compatible)                      │ │
│  │ ✗ Advanced-ControlNet (incompatible - torch v2.2)  │ │
│  │                                                    │ │
│  │ v0.2.1 - Jan 18, 2025              [✓ Installed]  │ │
│  │ ─────────────────────────────────────────────────  │ │
│  │ Bug fixes and stability improvements               │ │
│  │                                                    │ │
│  │ v0.2.0 - Jan 10, 2025              [✓ Installed]  │ │
│  │ ─────────────────────────────────────────────────  │ │
│  │ Initial stable release                             │ │
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  [ ] Show pre-releases                                   │
│                                                          │
│  [Cancel]                                                │
└─────────────────────────────────────────────────────────┘
```

### Custom Nodes Manager

```
┌─────────────────────────────────────────────────────────┐
│  Custom Nodes                                            │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  [Install from Git URL]  [Update Cache]                  │
│                                                          │
│  ┌────────────────────────────────────────────────────┐ │
│  │ Node Name           │ v0.2.0  │ v0.2.1  │ v0.3.0  │  │
│  ├────────────────────────────────────────────────────┤ │
│  │ ComfyUI-Manager     │ ✓ abc12 │ ✓ xyz78 │ ✓ xyz78 │🗑│
│  │ Advanced-ControlNet │ ✗ N/A   │ ✓ def45 │ ✓ def45 │🗑│
│  │ WAS-Node-Suite      │ ✓ 9ab3c │ -       │ -       │🗑│
│  └────────────────────────────────────────────────────┘ │
│                                                          │
│  Legend:                                                 │
│  ✓ = Installed & compatible (shows git commit short)    │
│  ✗ = Incompatible with version (not installed)          │
│  - = Not installed for this version                      │
│                                                          │
│  Click a cell to:                                        │
│  • Install node to that version                          │
│  • Update to different commit/tag                        │
│  • View compatibility details                            │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

---

## Technical Specifications

### UV Package Manager Integration

**Why UV Instead of Pip**:
- **10-100x faster** installation and dependency resolution
- Better dependency conflict detection
- More reliable virtual environment management
- Built-in lockfile support for reproducible installs

**UV Installation**:
```bash
# UV will be installed automatically by the launcher if not present
curl -LsSf https://astral.sh/uv/install.sh | sh
```

**Usage**:
```python
# Create venv with UV
subprocess.run(["uv", "venv", str(venv_path)])

# Install dependencies with UV
subprocess.run([
    "uv", "pip", "install",
    "-r", str(requirements_path),
    "--python", str(venv_path / "bin/python")
])

# Check installed packages
subprocess.run([
    "uv", "pip", "list",
    "--python", str(venv_path / "bin/python"),
    "--format", "json"
])
```

### Dependency Conflict Detection System

**Critical Requirement**: Prevent installation of incompatible custom nodes by checking dependencies BEFORE installation.

**Conflict Detection Algorithm**:

1. **Parse ComfyUI requirements**:
   ```python
   def parse_requirements(requirements_path: Path) -> Dict[str, VersionSpec]:
       """Parse requirements.txt into package→version mapping"""
       # Use packaging.requirements.Requirement
       # Return: {"torch": ">=2.0.0,<2.2.0", "numpy": "==1.24.0"}
   ```

2. **Parse custom node requirements**:
   ```python
   def parse_custom_node_requirements(node_path: Path) -> Dict[str, VersionSpec]:
       """Parse custom node requirements.txt"""
       # Same as above
   ```

3. **Check for conflicts**:
   ```python
   def check_compatibility(
       comfyui_reqs: Dict[str, VersionSpec],
       custom_node_reqs: Dict[str, VersionSpec]
   ) -> CompatibilityResult:
       """
       Check if requirements are compatible.
       ComfyUI requirements take precedence.
       """
       conflicts = []
       additional = []

       for package, node_spec in custom_node_reqs.items():
           if package in comfyui_reqs:
               comfyui_spec = comfyui_reqs[package]

               # Check if version ranges overlap
               if not specs_compatible(comfyui_spec, node_spec):
                   conflicts.append({
                       "package": package,
                       "comfyui_requires": comfyui_spec,
                       "node_requires": node_spec
                   })
           else:
               # Package not in ComfyUI - safe to add
               additional.append(package)

       if conflicts:
           return CompatibilityResult(
               status="incompatible",
               conflicts=conflicts
           )
       else:
           return CompatibilityResult(
               status="compatible",
               additional_requirements=additional
           )
   ```

4. **Install strategy**:
   ```python
   if compatibility.status == "compatible":
       # Install ComfyUI requirements first
       uv_pip_install(comfyui_requirements)

       # Then install only ADDITIONAL custom node requirements
       # (packages not already satisfied by ComfyUI)
       if compatibility.additional_requirements:
           uv_pip_install(compatibility.additional_requirements)
   else:
       raise IncompatibleDependenciesError(compatibility.conflicts)
   ```

**Caching**:
- Cache compatibility results in `custom_nodes.json`
- Key: `(node_name, node_requirements_hash, comfyui_version, comfyui_requirements_hash)`
- Invalidate when requirements.txt changes

### Dependency Checking System

**When checking if dependencies are installed for a ComfyUI version**:

1. **Read the version's requirements.txt**:
   ```python
   requirements_path = comfyui_versions / version / "requirements.txt"
   ```

2. **Check against the version's venv using UV**:
   ```python
   uv_path = comfyui_versions / version / "venv/bin/uv"
   venv_python = comfyui_versions / version / "venv/bin/python"

   # Use UV to list installed packages
   result = subprocess.run([
       str(uv_path), "pip", "list",
       "--python", str(venv_python),
       "--format", "json"
   ], capture_output=True, text=True)

   installed = json.loads(result.stdout)
   ```

3. **Parse requirements correctly**:
   - Use `packaging.requirements.Requirement` to parse
   - Handle version specifiers: `torch>=2.0.0`, `numpy==1.24.0`, `pillow>=9.0.0,<11.0.0`
   - Handle extras: `transformers[torch]`
   - Handle comments and blank lines
   - Handle `-f` (find-links) and other pip/uv flags

4. **Report missing/outdated dependencies**:
   ```python
   {
       "status": "incomplete",
       "missing": ["torch>=2.0.0"],
       "outdated": [
           {"package": "numpy", "required": "1.24.0", "installed": "1.23.0"}
       ],
       "satisfied": ["pillow>=9.0.0"]
   }
   ```

5. **Install dependencies using UV**:
   ```python
   uv_path = comfyui_versions / version / "venv/bin/uv"
   venv_python = comfyui_versions / version / "venv/bin/python"

   subprocess.run([
       str(uv_path), "pip", "install",
       "-r", str(requirements_path),
       "--python", str(venv_python)
   ])
   ```

**Implementation Notes**:
- Use `packaging.requirements` to parse requirement strings
- Use UV's pip interface to check installed packages
- Cache dependency check results (invalidate on requirements.txt hash change)
- Provide progress callbacks for installation
- NEVER use system pip - always use UV within the venv

### Symlink Management

**Strategy**: Relative symlinks for portability

**What Gets Symlinked**:
- ✅ Models (shared across all versions)
- ✅ User data (workflows, settings - shared across all versions)
- ❌ Custom nodes (NOT symlinked - per-version snapshots)

```python
# Example: Link shared model to version directory
source = Path("../../shared-resources/models/checkpoints/model.safetensors")
target = Path("comfyui-versions/v0.2.0/models/checkpoints/model.safetensors")

target.parent.mkdir(parents=True, exist_ok=True)
target.symlink_to(source)
```

**Symlink Creation Process**:
1. Ensure shared resource exists (source of truth)
2. Create target directory if needed
3. Remove existing symlink/file at target if needed
4. Create relative symlink
5. Verify symlink is valid

**Symlink Verification**:
```python
def verify_symlink(link: Path, expected_target: Path) -> bool:
    if not link.is_symlink():
        return False
    if not link.exists():  # Broken symlink
        return False
    resolved = link.resolve()
    return resolved == expected_target.resolve()
```

**When to Validate Symlinks**:
- On launcher startup
- Before launching a ComfyUI version
- After switching versions
- On user request (manual "Repair" button in UI)

**Handling Broken Symlinks**:
```python
def validate_and_repair_symlinks(version: str) -> RepairReport:
    broken = []
    repaired = []

    for symlink in find_all_symlinks(version_dir):
        if not symlink.exists():  # Broken
            broken.append(symlink)

            # Attempt repair
            expected_target = get_expected_target(symlink)
            if expected_target.exists():
                symlink.unlink()
                symlink.symlink_to(expected_target)
                repaired.append(symlink)
            else:
                # Target missing - just remove broken link
                symlink.unlink()

    return RepairReport(broken=broken, repaired=repaired)
```

**Dynamic Model Directory Discovery**:
```python
def discover_model_directories(comfyui_path: Path) -> List[str]:
    """
    Parse ComfyUI's folder_paths.py to discover model directories.
    This ensures compatibility with future ComfyUI versions that
    may add new model categories.
    """
    folder_paths_file = comfyui_path / "folder_paths.py"

    if not folder_paths_file.exists():
        return KNOWN_MODEL_DIRS  # Fallback

    # Parse folder_names_and_paths definitions
    discovered = parse_folder_paths(folder_paths_file)

    # Combine with known directories
    return list(set(discovered + KNOWN_MODEL_DIRS))

def sync_shared_model_structure(comfyui_version: str):
    """
    Add new model directories to shared storage when found.
    NEVER remove directories - preserve models for other versions.
    """
    version_model_dirs = discover_model_directories(
        versions_dir / comfyui_version
    )

    for model_dir in version_model_dirs:
        target = shared_dir / "models" / model_dir
        if not target.exists():
            target.mkdir(parents=True)
            logger.info(f"Added new model directory: {model_dir}")
```

### Migration Detection

**Detection Algorithm**:

1. Check common locations:
   - `../ComfyUI` (sibling directory)
   - `~/ComfyUI`
   - User-specified path

2. Verify it's ComfyUI:
   ```python
   def is_comfyui_installation(path: Path) -> bool:
       main_py = path / "main.py"
       pyproject = path / "pyproject.toml"

       if not (main_py.exists() and pyproject.exists()):
           return False

       # Verify pyproject.toml contains ComfyUI
       try:
           with open(pyproject, 'rb') as f:
               data = tomllib.load(f)
               return data.get('project', {}).get('name') == 'ComfyUI'
       except:
           return False
   ```

3. Detect version:
   ```python
   def detect_version(path: Path) -> str:
       # Try git tag
       try:
           tag = subprocess.check_output(
               ['git', '-C', str(path), 'describe', '--tags', '--exact-match'],
               stderr=subprocess.DEVNULL, text=True
           ).strip()
           return tag
       except:
           pass

       # Try pyproject.toml version
       try:
           with open(path / "pyproject.toml", 'rb') as f:
               data = tomllib.load(f)
               version = data.get('project', {}).get('version')
               if version:
                   return f"v{version}"
       except:
           pass

       # Fallback: use "migrated-unknown"
       return "migrated-unknown"
   ```

**Migration Process**:

1. Show preview to user
2. On confirmation:
   - Move (or copy) installation to `comfyui-versions/{version}/`
   - Scan for models and workflows
   - Move models to shared storage and create symlinks
   - Move workflows to shared storage and create symlinks
   - Leave custom nodes in place (per-version snapshots)
   - Create git cache repos for each custom node (for future updates)
   - Create metadata entries
   - Create version config with custom node information

### Custom Node Isolation Strategy

**Design Decision**: Each ComfyUI version has its own isolated custom node installations to prevent updates from breaking compatibility.

**Why Per-Version Snapshots**:
- Custom node updates may introduce breaking changes
- Different ComfyUI versions may need different custom node versions
- Allows safe experimentation without affecting stable installations
- Prevents dependency conflicts between versions

**Implementation**:

```
comfyui-versions/
├── v0.2.0/
│   └── custom_nodes/
│       ├── ComfyUI-Manager/         # Real files, git commit abc123
│       └── Advanced-ControlNet/     # Real files, git commit def456
├── v0.2.1/
│   └── custom_nodes/
│       ├── ComfyUI-Manager/         # Real files, git commit xyz789 (different)
│       └── Advanced-ControlNet/     # Real files, git commit def456 (same)
```

**Git Repository Cache**:

To avoid repeated cloning, maintain bare git repos in shared storage:

```
shared-resources/
└── custom_nodes_cache/
    ├── ComfyUI-Manager.git/         # Bare repo, updated periodically
    └── Advanced-ControlNet.git/     # Bare repo
```

**Installation Flow**:

1. **First installation** of a custom node:
   ```python
   # Clone bare repo to cache
   git clone --bare <url> shared-resources/custom_nodes_cache/<name>.git

   # Clone from cache to version directory
   git clone shared-resources/custom_nodes_cache/<name>.git \
       comfyui-versions/v0.2.0/custom_nodes/<name>

   # Check compatibility
   check_custom_node_compatibility(node, "v0.2.0")

   # Install requirements if compatible
   uv pip install -r requirements.txt
   ```

2. **Installing to another version**:
   ```python
   # Update cache
   git -C shared-resources/custom_nodes_cache/<name>.git fetch

   # Clone from cache (fast - local clone)
   git clone shared-resources/custom_nodes_cache/<name>.git \
       comfyui-versions/v0.2.1/custom_nodes/<name>

   # User can choose specific commit/tag if desired
   git -C comfyui-versions/v0.2.1/custom_nodes/<name> checkout <commit>

   # Check compatibility and install
   ```

3. **Updating a custom node** (per-version):
   ```python
   # Update cache
   git -C shared-resources/custom_nodes_cache/<name>.git fetch

   # Pull in specific version directory
   git -C comfyui-versions/v0.2.0/custom_nodes/<name> pull

   # Recheck compatibility
   # Reinstall requirements if changed
   ```

**Benefits**:
- Fast cloning (local git clones are nearly instant)
- Offline capability (once cached)
- Version pinning (each ComfyUI version can use different node commits)
- Safe updates (update only affects one version at a time)

**Disk Space Considerations**:
- Custom nodes are typically small (1-50 MB each)
- Git compression makes bare repos efficient
- Most disk usage is from models (which remain shared)
- Example: 10 custom nodes × 3 versions = ~1-2 GB max

### Progress Tracking

For long-running operations (download, install, migration), use progress callbacks:

```python
from typing import Callable

ProgressCallback = Callable[[int, int, str], None]
# Parameters: current, total, status_message

def install_version(tag: str, progress_callback: ProgressCallback = None):
    if progress_callback:
        progress_callback(0, 100, "Downloading release...")

    # Download with progress
    download_with_progress(url, dest, lambda cur, tot:
        progress_callback(int(cur/tot * 30), 100, f"Downloaded {cur}/{tot} bytes")
    )

    if progress_callback:
        progress_callback(30, 100, "Extracting archive...")

    # Extract...

    if progress_callback:
        progress_callback(50, 100, "Creating virtual environment...")

    # Create venv...

    if progress_callback:
        progress_callback(70, 100, "Installing dependencies...")

    # Install deps...

    if progress_callback:
        progress_callback(100, 100, "Installation complete")
```

Frontend can display this as a progress bar.

### Error Handling

**Principles**:
- Fail gracefully with clear error messages
- Rollback on partial failures
- Log errors for debugging
- Provide user-friendly error descriptions

**Example**:
```python
try:
    install_version(tag)
except DownloadError as e:
    return {
        "success": False,
        "error": "Failed to download release",
        "details": str(e),
        "suggestion": "Check your internet connection and try again"
    }
except ExtractionError as e:
    return {
        "success": False,
        "error": "Failed to extract archive",
        "details": str(e),
        "suggestion": "The download may be corrupted. Try again."
    }
except DependencyError as e:
    return {
        "success": False,
        "error": "Failed to install dependencies",
        "details": str(e),
        "suggestion": "Check that UV is installed and accessible"
    }
except IncompatibleDependenciesError as e:
    return {
        "success": False,
        "error": "Incompatible custom node dependencies",
        "details": str(e),
        "conflicts": e.conflicts,
        "suggestion": "This custom node cannot be installed for this ComfyUI version"
    }
```

---

## Migration & Compatibility

### Migrating Existing Launcher Installations

**Current launcher** (before this update) doesn't have version management. After implementing this system:

1. On first run, detect if launcher is being upgraded
2. Look for existing ComfyUI installation at `../ComfyUI`
3. Offer migration to user
4. Migrate as described in Phase 7

### Backward Compatibility

**Breaking Changes**:
- Directory structure changes (new dirs: comfyui-versions, shared-resources, launcher-data)
- Metadata format (new JSON files)

**Migration Path**:
- On launcher update, run migration wizard
- Keep old installation intact (copy, don't move)
- User can verify new system works before removing old installation

### Version Compatibility

**ComfyUI versions** may have different:
- Python version requirements
- Dependency versions
- Directory structures
- API changes

**Handling**:
- Each version has its own UV-managed venv (isolates Python dependencies)
- Model symlinks are shared across versions (saves disk space)
- Custom nodes are isolated per-version (prevents breaking changes)
- Version-specific configs allow per-version customization

**Future-Proofing**:
- Dynamic model directory discovery (parse folder_paths.py)
- Add new model directories without removing existing ones
- Version-specific workarounds if needed
- Metadata format should be extensible (allow unknown fields)

---

## Testing Strategy

### Unit Tests

**Module**: Each backend module should have unit tests

**Coverage**:
- MetadataManager: CRUD operations, validation, atomic writes
- GitHubAPI: Fetch, cache, download (use mocks for API calls)
- ResourceManager: Symlink creation, file migration, scanning
- VersionManager: Installation, switching, launching

**Tools**: pytest, unittest.mock

### Integration Tests

**Scenarios**:
1. Install version → verify files, venv, metadata, symlinks
2. Switch version → verify symlinks updated, active version changed
3. Enable custom node → verify symlink created, config updated
4. Add model → verify copied to shared storage, metadata updated
5. Migrate existing installation → verify resources moved, symlinks created

**Tools**: pytest with temporary directories, fixtures

### End-to-End Tests

**Scenarios**:
1. Fresh installation workflow
2. Migration workflow
3. Multi-version workflow (install 3 versions, switch between them)
4. Custom node management across versions
5. Dependency checking and installation

**Tools**: pytest, selenium (for UI testing)

### Manual Testing Checklist

- [ ] Install new version from GitHub
- [ ] Switch between versions
- [ ] Launch each version and verify it works
- [ ] Add model to shared storage, verify visible in all versions
- [ ] Install custom node, enable for specific versions
- [ ] Disable custom node for a version
- [ ] Remove a version
- [ ] Migrate existing ComfyUI installation
- [ ] Check dependency status
- [ ] Install missing dependencies
- [ ] Test with large model library (100+ models)
- [ ] Test with many custom nodes (20+ nodes)
- [ ] Test error cases (network failure, disk full, etc.)

---

## Appendix

### Known ComfyUI Model Categories

Based on ComfyUI source code, these are the model directories:

- `checkpoints/`
- `clip/`
- `clip_vision/`
- `configs/`
- `controlnet/`
- `diffusion_models/`
- `embeddings/`
- `loras/`
- `photomaker/`
- `style_models/`
- `unet/`
- `upscale_models/`
- `vae/`
- `vae_approx/`

The launcher should create all these subdirectories in `shared-resources/models/`.

### Example requirements.txt from ComfyUI

```
torch
torchvision
torchaudio
torchsde
einops
transformers>=4.25.1
tokenizers>=0.13.3
sentencepiece
safetensors>=0.4.2
aiohttp
pyyaml
Pillow
scipy
tqdm
psutil
kornia>=0.7.1
spandrel
soundfile
```

The dependency checker must handle this correctly, including:
- Version specifiers (`>=`, `==`, `<`, `>`, `<=`, `!=`)
- No version specifier (any version)
- Git URLs (if used)
- Editable installs (if used)

### File Sizes & Performance Considerations

**Model files**:
- SDXL checkpoint: ~6-7 GB
- SD 1.5 checkpoint: ~2-4 GB
- LoRA: 50-200 MB
- VAE: 100-500 MB

**Implications**:
- Symlinks are instant (no file copying)
- Model scanning should be efficient (don't hash large files unless necessary)
- Use metadata cache to avoid repeated filesystem scans
- Provide progress indicators for operations on large files

**Disk Space**:
- Multiple ComfyUI versions: ~1.2 GB per version
- Models: Can be 100s of GB
- Shared storage saves disk space compared to duplicating models per version

### Security Considerations

1. **Symlink attacks**: Verify symlink targets are within expected directories
2. **Code execution**: Custom nodes contain Python code - warn users
3. **Git clones**: Verify git URLs before cloning
4. **Downloads**: Verify checksums if available
5. **File permissions**: Ensure launcher doesn't require root privileges for normal operations

### Open Questions (to be resolved during implementation)

1. **How to handle ComfyUI versions with different model directory structures?**
   - Monitor ComfyUI releases for structural changes
   - Add version-specific symlink mappings if needed

2. **Should we support installing from git commits/branches, or only releases?**
   - Start with releases only (simpler)
   - Add git support in future if needed

3. **How to handle custom nodes with conflicting requirements?**
   - ✅ RESOLVED: Dependency conflict detection system
   - Check compatibility before installation
   - Per-version venvs isolate dependencies
   - Per-version custom node snapshots prevent cross-contamination

4. **Should we auto-update custom nodes?**
   - No auto-update (too risky)
   - Provide manual update button
   - Show "update available" indicator

5. **How to handle workflows that use nodes not available in a version?**
   - Metadata tracks requiredNodes per workflow
   - Warn user when opening incompatible workflow

---

## Summary

This plan provides a comprehensive roadmap for implementing multi-version ComfyUI management in the launcher. Key features:

### Core Features

✅ Install multiple ComfyUI versions from GitHub releases
✅ Switch between versions (only one runs at a time)
✅ Shared storage for models and workflows (saves disk space)
✅ **Per-version custom node snapshots** (isolated installations)
✅ **UV-managed Python virtual environments** (10-100x faster than pip)
✅ **Dependency conflict detection** (prevents incompatible installations)
✅ **Pre-installation compatibility checking** (know before you install)
✅ **Dynamic model directory discovery** (future-proof)
✅ Migration tool for existing installations
✅ Metadata tracking and caching for all resources
✅ Clean, modular architecture

### Key Refinements from Original Plan

1. **UV Package Manager** (replacing pip):
   - Faster installation and dependency resolution
   - Better conflict detection
   - More reliable venv management

2. **Per-Version Custom Node Isolation**:
   - Each ComfyUI version has its own custom node installations
   - Updates don't break other versions
   - Git repository caching for fast cloning
   - Support for version pinning (specific commits/tags)

3. **Dependency Conflict Prevention**:
   - Parse and compare requirements.txt files
   - Detect incompatibilities BEFORE installation
   - ComfyUI requirements always take precedence
   - Cache compatibility results for performance

4. **Dynamic Model Directory Management**:
   - Parse folder_paths.py to discover model directories
   - Automatically add new model categories
   - Never remove directories (preserve models for other versions)
   - Forward-compatible with ComfyUI changes

5. **Smart Symlink Validation**:
   - Validate on startup, before launch, and after version switch
   - Automatic repair of broken symlinks
   - No periodic background checking (on-demand only)

### Architecture Highlights

**What Gets Shared**:
- ✅ Models (symlinked to all versions)
- ✅ User data (workflows, settings)
- ✅ Git repository cache (for custom nodes)

**What Stays Isolated**:
- ✅ Custom nodes (per-version real files)
- ✅ Python dependencies (per-version UV venvs)
- ✅ ComfyUI code (per-version installations)

**Disk Space Efficiency**:
- Models shared = massive savings (100+ GB of models not duplicated)
- Custom nodes duplicated = minimal cost (~1-2 GB for 10 nodes × 3 versions)
- Git caching reduces network usage and speeds up installations

The implementation is divided into 8 phases, progressing from infrastructure to UI to migration and testing. Each phase builds on previous phases, ensuring a solid foundation.