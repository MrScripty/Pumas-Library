# Model Library System - Overview

**Version**: 3.1
**Status**: Planning Phase

---

## Table of Contents

- [Goal](#goal)
- [Key Requirements](#key-requirements)
- [Architecture Summary](#architecture-summary)
- [Technology Stack](#technology-stack)
- [Directory Structure](#directory-structure)
- [Library Structure](#library-structure)
- [ComfyUI Integration](#comfyui-integration)
- [Core Principles](#core-principles)
- [Success Criteria](#success-criteria)
- [Related Documents](#related-documents)

---

## Goal

Enable seamless model management by:

1. **Importing models** via drag-and-drop with automatic HuggingFace metadata lookup
2. **Mapping models** from the centralized library to application-specific directories (ComfyUI, etc.) using configurable translation maps
3. **No file duplication** - all mappings use symlinks (relative when possible, absolute when necessary)

---

## Key Requirements

- Import models by dragging files onto the GUI
- Look up metadata from HuggingFace (hash verification with fuzzy filename fallback)
- **Copy** files into standardized library structure (with optional move for same-filesystem imports)
- Support multi-file models (folders and companion files)
- Create translation configs that map library → app directories
- Use relative symlinks for portability when library and app are on same filesystem
- Fall back to absolute symlinks for cross-filesystem scenarios (with warnings)
- Support arbitrary apps (ComfyUI is the reference implementation)
- All ComfyUI installs share default mapping unless overridden
- Incremental sync when library changes (new models imported/deleted)
- Validate version constraints in override files

---

## Architecture Summary

### Technology Stack

- **Backend**: Python 3.12+ with PyWebView (GTK/WebKit)
- **Frontend**: React 19 + TypeScript, Vite, TailwindCSS, Framer Motion
- **Database**: SQLite with WAL mode for model library indexing (disposable cache)
- **Dependencies**: huggingface_hub, pydantic, blake3, psutil (drive type detection)

### Platform Requirements

- **OS**: Linux only (GTK/WebKit environment)
- **Filesystem**: Library and app installations should be on the same filesystem for optimal performance (relative symlinks)
- **Cross-filesystem support**: Absolute symlinks with warnings if library and apps are on different drives
- **Python**: 3.12+ required
- **Dependencies**: huggingface_hub, blake3, packaging, psutil

---

## Directory Structure

```
/media/jeremy/OrangeCream/Linux Software/Pumas-Library/
├── backend/
│   ├── model_library/
│   │   ├── library.py          # Core library management
│   │   ├── downloader.py       # HuggingFace downloads (997 lines)
│   │   ├── importer.py         # Local model import (154 lines)
│   │   ├── mapper.py           # Model mapping engine (205 lines)
│   │   ├── naming.py           # Filename normalization
│   │   ├── io_manager.py       # Drive-aware I/O queue (NEW)
│   │   └── fs_validator.py     # Filesystem validation (NEW)
│   └── api/core.py             # Main PyWebView API
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   │   ├── ModelManager.tsx
│   │   │   ├── LocalModelsList.tsx
│   │   │   ├── RemoteModelsList.tsx
│   │   │   ├── ModelImportDropZone.tsx      (NEW)
│   │   │   └── ModelImportDialog.tsx        (NEW)
│   │   └── hooks/
│   │       ├── useModels.ts
│   │       └── useModelDownloads.ts
├── shared-resources/
│   └── models/
│       ├── models.db           # SQLite index (disposable cache)
│       └── {model_type}/{family}/{cleaned_name}/
│           ├── metadata.json   # Single Source of Truth
│           ├── overrides.json  # Version constraints (optional)
│           └── *.safetensors
├── comfyui-versions/
│   └── {version}/
│       └── models/             # 22+ subdirectories (dynamic)
└── launcher-data/
    ├── db/
    │   └── registry.db         # Link registry (NEW)
    └── config/
        └── model-library-translation/  # Mapping configs
```

---

## Library Structure

### Terminology

This system uses standardized industry terminology for model types:

- **Single-File Model**: .safetensors, .ckpt, .gguf files (most common)
- **Diffusion Folder**: HuggingFace Diffusers format with unet/, vae/, etc. subdirectories
- **Sharded Set**: Large models split into multiple files (e.g., model-00001-of-00005.safetensors)

### Model Directory Layout

```
shared-resources/models/
├── models.db                    # SQLite index (disposable)
├── .downloads/                  # Temp staging
├── diffusion/
│   ├── stable-diffusion/
│   │   └── sd-v1-5/
│   │       ├── metadata.json         # Single Source of Truth
│   │       ├── overrides.json        # Version constraints (optional)
│   │       ├── sd-v1-5.safetensors   # Single-file model
│   │       └── preview.png
│   │   └── flux-dev/                 # Diffusion folder (Diffusers format)
│   │       ├── metadata.json
│   │       ├── unet/
│   │       ├── vae/
│   │       └── text_encoder/
│   └── llama-3-70b/                  # Sharded set
│       ├── metadata.json
│       ├── model-00001-of-00005.safetensors
│       ├── model-00002-of-00005.safetensors
│       └── ...
└── llm/
    └── llama/
        └── llama-3.1-8b/
            └── ...
```

### Metadata Schema (metadata.json)

**Single Source of Truth**: This file is the authoritative record for each model. SQLite is a disposable cache that can be perfectly reconstructed via "Deep Scan".

**Model Weight Size Field**: The `approx_weight_size_gb` field provides an approximate model weight size based on file size and precision. This is **informational only** - users are not blocked from using models that exceed their VRAM.

> **Important**: This field represents the model weight size on disk, NOT the actual VRAM required
> to run the model. Actual VRAM usage depends on:
> - KV cache size (scales with context length)
> - Activation memory during inference
> - Dequantization overhead for quantized models (GGUF, EXL2)
> - Batch size and image resolution
>
> A 7B model at FP16 (~14GB weights) may require 18-24GB VRAM during inference.

The estimate is calculated as:
- FP32: `size_bytes / (1024^3)` (full precision)
- FP16/BF16: `size_bytes / (1024^3) / 2` (half precision)
- INT8: `size_bytes / (1024^3) / 4` (quantized)
- GGUF: Uses embedded metadata if available

```json
{
  "model_id": "diffusion/stable-diffusion/sd-v1-5",
  "family": "stable-diffusion",
  "model_type": "diffusion",
  "subtype": "checkpoints",
  "official_name": "Stable Diffusion v1.5",
  "cleaned_name": "sd-v1-5",
  "variant": "pruned",
  "precision": "fp16",
  "tags": ["sd1.5", "realistic"],
  "base_model": "stable-diffusion-v1",
  "preview_image": "preview.png",
  "release_date": "2023-10-01T00:00:00Z",
  "download_url": "https://huggingface.co/runwayml/stable-diffusion-v1-5",
  "model_card": {},
  "inference_settings": {},
  "compatible_apps": ["comfyui"],
  "hashes": {
    "sha256": "...",
    "blake3": "..."
  },
  "notes": "",
  "added_date": "2026-01-07T12:00:00Z",
  "updated_date": "2026-01-07T12:00:00Z",
  "size_bytes": 4265380864,
  "approx_weight_size_gb": 4.0,
  "files": [
    {
      "name": "sd-v1-5.safetensors",
      "original_name": "sd-v1-5.safetensors",
      "size": 4265380864
    }
  ]
}
```

### Version Overrides (overrides.json)

Optional file allowing users to restrict which app versions can link to a model using PEP 440 version specifiers.

```json
{
  "version_ranges": {
    "comfyui": ">=0.5.0,<0.7.0",
    "automatic1111": "*"
  }
}
```

**Behavior**:
- **Missing file**: Model links to all versions of all apps (default)
- **Valid file**: Model only links to versions matching the constraints
- **Invalid file**: WARNING logged, model excluded from all mappings until fixed

---

## ComfyUI Integration

### Model Directory Structure

ComfyUI v0.6.0 has 22+ subdirectories under `models/`. Custom nodes may add more (e.g., `ipadapter/`).

**Dynamic Directory Discovery**: The mapper scans the actual ComfyUI installation to discover all subdirectories, not relying on a hardcoded list.

```
comfyui-versions/v0.6.0/models/
├── checkpoints/              # Main SD models
├── loras/                    # LoRA adapters
├── vae/                      # VAE models
├── controlnet/               # ControlNet models
├── embeddings/               # Textual inversions
├── upscale_models/           # ESRGAN, etc.
├── clip/                     # CLIP text encoders
├── clip_vision/              # CLIP vision models
├── diffusers/                # Diffusers format
├── unet/                     # U-Net models
├── ... (22+ total, dynamic)
```

### Linking Strategy

**Same Filesystem** (library and ComfyUI on same drive):
- **ext4/btrfs/xfs**: Relative symlinks (portable, recommended)
- **NTFS**: Hard links (NTFS symlinks are unreliable on Linux)

**Cross-Filesystem** (library and ComfyUI on different drives):
- **Absolute symlinks** with UI warnings about drive unmounting

---

## Link Registry (registry.db)

A persistent SQLite database (`launcher-data/db/registry.db`) tracks every symlink/hardlink created by the mapping system.

### Purpose

- **Hybrid Path Storage**: Relative paths for internal links (portable), absolute paths for external drives
- **Clean Deletion**: When a model is deleted, query the registry to find all links and cascade-delete them
- **Health Checks**: Detect broken links, orphaned links, and missing sources on startup
- **Relocation Helper**: Bulk-update absolute paths when external drives change mount points

### Schema

```sql
CREATE TABLE links (
    link_id INTEGER PRIMARY KEY,
    model_id TEXT NOT NULL,
    target_app_path TEXT NOT NULL,      -- Relative if internal, Absolute if external
    source_model_path TEXT NOT NULL,    -- Relative if internal, Absolute if external
    is_external BOOLEAN DEFAULT 0,
    link_type TEXT CHECK(link_type IN ('symlink', 'hardlink')) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (model_id) REFERENCES models(model_id) ON DELETE CASCADE
);

CREATE INDEX idx_model_id ON links(model_id);
CREATE INDEX idx_target_path ON links(target_app_path);
```

### Operations

**Create Link**:
```python
registry.register_link(
    model_id="diffusion/stable-diffusion/sd-v1-5",
    target_path="/app/comfyui/models/checkpoints/sd-v1-5.safetensors",
    source_path="/library/models/diffusion/stable-diffusion/sd-v1-5/sd-v1-5.safetensors",
    is_external=False,
    link_type="symlink"
)
```

**Cascade Delete**:
```python
# Query all links for model
links = registry.get_links_for_model(model_id)
# Unlink all symlinks
for link in links:
    Path(link['target_app_path']).unlink(missing_ok=True)
# Purge registry entries
registry.delete_links_for_model(model_id)
# Delete physical model files
delete_model_directory(model_dir)
```

**Health Check**:
```python
# Find broken links (source deleted)
broken_links = registry.find_broken_links()
# Find orphaned links (not in registry but exist on disk)
orphaned_links = find_orphaned_links_on_disk()
```

---

## Core Principles

### 1. Metadata as Single Source of Truth (SSoT)

The `metadata.json` file inside each model directory is the authoritative record. SQLite is treated as a **disposable cache** that can be perfectly reconstructed via a "Deep Scan" of the library folders.

### 2. Link Registry for Clean Operations

The `registry.db` tracks every link created, enabling:
- **Cascade deletion** (delete model → find all links → unlink → purge registry → delete files)
- **Health validation** (detect broken links, missing sources, orphaned symlinks)
- **Portable vs absolute path management** (relative for same filesystem, absolute for external drives)

### 3. Atomic Operations

All file operations use temporary extensions (`.tmp`) during processing to prevent partial indexing by watchers or crashes mid-operation.

### 4. Offline-First Design

Network failures or API rate limits never block local operations. Models can be imported without HuggingFace metadata and enriched later.

### 5. Platform-Aware I/O

Optimize disk access patterns based on drive type (SSD vs HDD) to prevent thrashing:
- **SSD**: Allow 2 concurrent operations
- **HDD**: Force strictly sequential (1 operation at a time)

### 6. Incremental Sync

When models are imported, only the new models are synced to installed apps, not the entire library. This provides ~2200× performance improvement over full tree scans.

### 7. Cross-Platform Compatibility

All filenames are normalized to be NTFS-compatible at import time, ensuring the library can be copied to Windows drives without errors.

---

## Success Criteria

### Phase 1: Model Import Complete When

- Users can drag model files onto GUI
- Files are looked up on HuggingFace (hash verification + fuzzy fallback)
- Metadata is displayed and editable with trust badges
- Related files are shown with download option
- Files are copied into library with atomic operations
- Stream hashing computes BLAKE3/SHA256 during copy (no double-read)
- SQLite index is updated
- Incremental sync applies new models to all installed apps
- All import scenarios pass tests

### Phase 2: Mapping System Complete When

- Default mapping config is created on ComfyUI install with dynamic directory discovery
- Mappings are auto-applied (symlinks created)
- All ComfyUI model directories have correct mappings
- Symlinks are relative when possible, absolute with warnings when necessary
- Version constraints work (overrides.json)
- Manual sync API works
- Clean uninstall removes symlinks
- Sandbox detection warns users (Flatpak/Snap/Docker)
- All mapping scenarios pass tests

### Phase 3: Mapping UI Complete When

- Visual mapping editor is functional
- Drag-and-drop between panels works
- Filter rules auto-generate from selections
- Preview shows accurate results
- Custom variants can be created and saved

---

## Related Documents

- [Performance & Data Integrity](01-performance-and-integrity.md) - I/O optimization, hashing, SQLite tuning, filesystem validation
- [Model Import System](02-model-import.md) - Drag-and-drop import with HuggingFace lookup
- [Mapping System](03-mapping-system.md) - Configuration-based model mapping
- [Implementation Phases](04-implementation-phases.md) - Concrete implementation steps and file checklist

---

**End of Overview**
