# Model Mapping System

**Version**: 3.1

---

## Table of Contents

- [Overview](#overview)
- [Configuration Schema](#configuration-schema)
- [Default ComfyUI Configuration](#default-comfyui-configuration)
- [Mapping Engine](#mapping-engine)
- [Sync Strategies](#sync-strategies)
- [Version Constraints](#version-constraints)
- [Link Types](#link-types)
- [Backend Implementation](#backend-implementation)
- [Frontend Integration](#frontend-integration)
- [Testing Strategy](#testing-strategy)

---

## Overview

Create a flexible translation system that maps models from the standardized library structure to application-specific directories using JSON configuration files.

### Key Features

- JSON-based mapping configurations
- Support for wildcard versions (`comfyui_*_default.json`)
- Dynamic directory discovery (scans actual ComfyUI installation)
- Multiple link types (relative symlink, absolute symlink, hard link)
- **Mapping preview (dry run)** - Preview all changes before applying with conflict detection
- **Conflict resolution** - Skip+warn strategy with detailed reporting
- Version constraints via `overrides.json` (PEP 440 specifiers)
- Incremental sync (only process changed models)
- Automatic sync on model import and ComfyUI installation
- Manual sync API for user-triggered updates
- Clean uninstall (removes all symlinks)
- Sandbox detection (Flatpak/Snap/Docker warnings)

---

## Configuration Schema

### File Naming Convention

**Pattern**: `{app_id}_{version}_{variant}.json`

**Examples**:
- `comfyui_*_default.json` - Applies to all ComfyUI versions
- `comfyui_0.6.0_default.json` - Specific to v0.6.0
- `comfyui_0.6.0_sdxl-only.json` - Custom variant

**Location**: `launcher-data/config/model-library-translation/`

### Top-Level Schema

```typescript
interface MappingConfig {
  app: string;                    // "comfyui", "automatic1111", etc.
  version: string;                // Semantic version or "*" for all
  variant?: string;               // "default", "sdxl-only", etc.
  description?: string;           // Human-readable description
  created_at?: string;            // ISO 8601 timestamp
  updated_at?: string;            // ISO 8601 timestamp
  mappings: MappingRule[];        // Array of mapping rules
}
```

### Mapping Rule Schema

```typescript
interface MappingRule {
  name: string;                   // Human-readable name
  description?: string;           // Purpose of this mapping
  method: "symlink" | "copy";     // How to link (always symlink for now)
  target_subdir: string;          // App subdirectory (e.g., "checkpoints")
  patterns: string[];             // File glob patterns (e.g., ["*.safetensors"])
  link_type?: "file" | "directory"; // Default: "file"
  filters: FilterCriteria;        // Model selection criteria
  enabled: boolean;               // Can be toggled on/off
  priority?: number;              // Order of application (default: 0)
}
```

### Filter Criteria

```typescript
interface FilterCriteria {
  // All filters are AND-ed together
  model_type?: string | string[];      // "diffusion", "llm"
  subtype?: string | string[];         // "checkpoints", "loras", etc.
  families?: string[];                 // ["stable-diffusion", "flux"]
  tags?: string[];                     // Match ANY tag (OR logic)
  exclude_tags?: string[];             // Exclude models with these tags
  base_model?: string;                 // For fine-tunes
  min_size_mb?: number;                // Minimum file size
  max_size_mb?: number;                // Maximum file size
}
```

**Filter Logic**:
- `model_type`, `subtype`, `families`: AND (must match all)
- `tags`: OR (match ANY tag)
- `exclude_tags`: OR (exclude if has ANY excluded tag)
- **Exclusion happens AFTER inclusion** (exclusion wins)

**Example**:
```json
{
  "filters": {
    "tags": ["sdxl", "sd1.5"],        // Include if has sdxl OR sd1.5
    "exclude_tags": ["nsfw", "beta"]  // BUT exclude if has nsfw OR beta
  }
}
```
Result: Model with `["sdxl", "beta"]` is **excluded** (exclusion wins)

---

## Default ComfyUI Configuration

**File**: `launcher-data/config/model-library-translation/comfyui_*_default.json`

This configuration is created automatically on first ComfyUI installation and applies to all versions unless overridden.

**Key Features**:
- Dynamic directory discovery (scans actual `models/` folder)
- Baseline mappings for core directories (checkpoints, loras, vae, etc.)
- Auto-discovered custom node directories (e.g., `ipadapter/`)
- Separate file vs directory handling (Diffusers format uses `link_type: "directory"`)

```json
{
  "app": "comfyui",
  "version": "*",
  "variant": "default",
  "description": "Default model mappings for all ComfyUI versions",
  "created_at": "2026-01-07T00:00:00Z",
  "updated_at": "2026-01-07T00:00:00Z",
  "mappings": [
    {
      "name": "Main Checkpoints",
      "description": "Stable Diffusion checkpoints (SD1.5, SDXL, etc.)",
      "method": "symlink",
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors", "*.ckpt"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "checkpoints"
      },
      "enabled": true,
      "priority": 10
    },
    {
      "name": "LoRA Adapters",
      "description": "LoRA fine-tuning adapters",
      "method": "symlink",
      "target_subdir": "loras",
      "patterns": ["*.safetensors", "*.pt"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "loras"
      },
      "enabled": true,
      "priority": 20
    },
    {
      "name": "VAE Models",
      "description": "Variational Autoencoders",
      "method": "symlink",
      "target_subdir": "vae",
      "patterns": ["*.safetensors", "*.pt"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "vae"
      },
      "enabled": true,
      "priority": 30
    },
    {
      "name": "ControlNet Models",
      "description": "ControlNet conditioning models",
      "method": "symlink",
      "target_subdir": "controlnet",
      "patterns": ["*.safetensors", "*.pt", "*.gguf"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "controlnet"
      },
      "enabled": true,
      "priority": 40
    },
    {
      "name": "Embeddings",
      "description": "Textual inversion embeddings",
      "method": "symlink",
      "target_subdir": "embeddings",
      "patterns": ["*.pt", "*.safetensors"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "embeddings"
      },
      "enabled": true,
      "priority": 50
    },
    {
      "name": "Upscale Models",
      "description": "Image upscaling models (ESRGAN, RealESRGAN)",
      "method": "symlink",
      "target_subdir": "upscale_models",
      "patterns": ["*.pth", "*.safetensors"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "upscale"
      },
      "enabled": true,
      "priority": 60
    },
    {
      "name": "CLIP Models",
      "description": "CLIP text encoder models",
      "method": "symlink",
      "target_subdir": "clip",
      "patterns": ["*.safetensors", "*.pt"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "clip"
      },
      "enabled": true,
      "priority": 70
    },
    {
      "name": "CLIP Vision Models",
      "description": "CLIP vision encoder models",
      "method": "symlink",
      "target_subdir": "clip_vision",
      "patterns": ["*.safetensors", "*.pt"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "clip_vision"
      },
      "enabled": true,
      "priority": 80
    },
    {
      "name": "Diffusers Format",
      "description": "HuggingFace Diffusers format models (entire directories)",
      "method": "symlink",
      "target_subdir": "diffusers",
      "patterns": ["*"],
      "link_type": "directory",
      "filters": {
        "model_type": "diffusion",
        "subtype": "diffusers"
      },
      "enabled": true,
      "priority": 140
    }
  ]
}
```

### Custom Variants

#### SDXL-Only Variant

`comfyui_0.6.0_sdxl-only.json`:
```json
{
  "app": "comfyui",
  "version": "0.6.0",
  "variant": "sdxl-only",
  "description": "Only map SDXL models",
  "mappings": [
    {
      "name": "SDXL Checkpoints Only",
      "method": "symlink",
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "tags": ["sdxl"]
      },
      "enabled": true
    },
    {
      "name": "SDXL LoRAs Only",
      "method": "symlink",
      "target_subdir": "loras",
      "patterns": ["*.safetensors"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "loras",
        "base_model": "sdxl"
      },
      "enabled": true
    }
  ]
}
```

#### Mobile/Low-Spec Variant

`comfyui_0.6.0_mobile.json`:
```json
{
  "app": "comfyui",
  "version": "0.6.0",
  "variant": "mobile",
  "description": "Only small models for mobile/low-spec systems",
  "mappings": [
    {
      "name": "Small Checkpoints",
      "method": "symlink",
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "link_type": "file",
      "filters": {
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "max_size_mb": 4000
      },
      "enabled": true
    }
  ]
}
```

---

## Mapping Engine

### Config Loading & Precedence

**Precedence** (highest to lowest):
1. Exact version + variant: `comfyui_0.6.0_custom.json`
2. Exact version + default: `comfyui_0.6.0_default.json`
3. Wildcard + variant: `comfyui_*_custom.json`
4. Wildcard + default: `comfyui_*_default.json`

**Implementation**:

```python
def _load_and_merge_configs(self, app_id: str, app_version: str) -> dict:
    """
    Load and merge all matching configs with deterministic precedence.

    Returns:
        Merged config with all mappings, sorted by priority
    """
    configs = []
    config_root = Path("launcher-data/config/model-library-translation")

    # Collect all matching configs
    for config_path in sorted(config_root.glob("*.json")):
        parts = config_path.stem.split("_", 2)
        if len(parts) < 3:
            continue

        config_app, config_version, config_variant = parts

        if config_app.lower() != app_id.lower():
            continue

        # Check version match (exact or wildcard)
        if config_version != "*" and config_version != app_version:
            continue

        try:
            with open(config_path, 'r', encoding='utf-8') as f:
                config_data = json.load(f)
                config_data['_source_file'] = config_path.name
                config_data['_specificity'] = self._calculate_specificity(
                    config_version, config_variant
                )
                configs.append(config_data)
        except Exception as e:
            logger.error(f"Failed to load config {config_path}: {e}")

    if not configs:
        return None

    # Sort by specificity (highest first)
    configs.sort(key=lambda c: c['_specificity'], reverse=True)

    # Merge all mappings
    merged = {
        'app': app_id,
        'version': app_version,
        'variant': 'merged',
        'description': f'Merged from {len(configs)} configs',
        'mappings': []
    }

    for config in configs:
        for mapping in config.get('mappings', []):
            merged['mappings'].append({
                **mapping,
                '_source': config['_source_file']
            })

    # Sort mappings by priority (higher = applied later)
    merged['mappings'].sort(key=lambda m: m.get('priority', 0))

    return merged

def _calculate_specificity(self, version: str, variant: str) -> int:
    """
    Calculate config specificity score.
    Higher score = more specific = higher precedence.
    """
    score = 0

    # Version specificity
    if version != "*":
        score += 100  # Exact version

    # Variant specificity
    if variant != "default":
        score += 10  # Custom variant

    return score
```

### Dynamic Directory Discovery

**Problem**: Hardcoding ComfyUI directories breaks when custom nodes add new ones.

**Solution**: Scan actual `models/` folder at runtime.

**Implementation**:

```python
def create_default_comfyui_config(
    self,
    version: str = "*",
    comfyui_models_path: Optional[Path] = None
) -> Path:
    """
    Create default ComfyUI mapping config with dynamic directory discovery.

    Args:
        version: ComfyUI version (e.g., "0.6.0" or "*")
        comfyui_models_path: Path to ComfyUI models/ dir (for scanning)

    If comfyui_models_path is provided, scans actual directories instead
    of using hardcoded list.
    """
    # Static baseline mappings (always include these)
    baseline_mappings = [
        # ... core mappings from default config above
    ]

    mappings = baseline_mappings.copy()

    # If path provided, discover additional directories
    if comfyui_models_path and comfyui_models_path.exists():
        discovered_dirs = self._discover_model_directories(comfyui_models_path)

        # Find directories not in baseline
        baseline_subdirs = {m['target_subdir'] for m in baseline_mappings}
        new_subdirs = [d for d in discovered_dirs if d not in baseline_subdirs]

        logger.info(f"Discovered {len(new_subdirs)} additional model directories: {new_subdirs}")

        # Create generic mappings for new directories
        for subdir in new_subdirs:
            mappings.append({
                "name": f"{subdir.replace('_', ' ').title()} (Auto-discovered)",
                "description": f"Auto-discovered directory from ComfyUI installation",
                "target_subdir": subdir,
                "patterns": ["*.safetensors", "*.pt", "*.ckpt"],
                "link_type": "file",
                "filters": {"model_type": "diffusion"},
                "enabled": True,
                "priority": 200 + len(mappings)
            })

    config = {
        "app": "comfyui",
        "version": version,
        "variant": "default",
        "description": f"Default model mappings for ComfyUI {version}",
        "created_at": get_iso_timestamp(),
        "updated_at": get_iso_timestamp(),
        "mappings": mappings
    }

    # Save config
    filename = f"comfyui_{version}_default.json"
    config_path = self.config_root / filename

    with open(config_path, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=2, ensure_ascii=False)

    logger.info(f"Created config with {len(mappings)} mappings: {config_path}")
    return config_path


def _discover_model_directories(self, models_root: Path) -> list[str]:
    """
    Scan ComfyUI models/ directory for subdirectories.

    Returns:
        List of subdirectory names (e.g., ['checkpoints', 'loras', 'ipadapter'])
    """
    if not models_root.is_dir():
        return []

    subdirs = []
    for item in models_root.iterdir():
        if item.is_dir() and not item.name.startswith('.'):
            subdirs.append(item.name)

    return sorted(subdirs)
```

---

## Sync Strategies

### Auto-Application on Installation

When a ComfyUI version is installed, automatically create and apply default mappings.

**Implementation**: Update `backend/version_manager.py`

```python
def _finalize_installation(self, version_tag: str):
    """Called after ComfyUI version is installed."""
    # ... existing setup code ...

    # Create/apply model mappings
    self._setup_model_mappings(version_tag)

    logger.info(f"Installation complete for {version_tag}")


def _setup_model_mappings(self, version_tag: str):
    """Setup model library mappings for this version."""
    try:
        config_exists = self._check_mapping_config_exists(version_tag)

        if not config_exists:
            logger.info(f"Creating default mapping config for {version_tag}")
            # Pass models_root for directory scanning
            models_root = self._get_version_path(version_tag) / "models"
            self.resource_manager.model_mapper.create_default_comfyui_config(
                version_tag,
                comfyui_models_path=models_root
            )

        logger.info(f"Applying model mappings for {version_tag}")
        models_root = self._get_version_path(version_tag) / "models"
        links_created = self.resource_manager.model_mapper.apply_for_app(
            "comfyui",
            version_tag,
            models_root
        )

        logger.info(f"Created {links_created} model symlinks for {version_tag}")

    except Exception as e:
        logger.error(f"Failed to setup model mappings: {e}", exc_info=True)


def _check_mapping_config_exists(self, version_tag: str) -> bool:
    """Check if a mapping config exists for this version."""
    config_root = Path("launcher-data/config/model-library-translation")

    specific_config = config_root / f"comfyui_{version_tag}_default.json"
    if specific_config.exists():
        return True

    wildcard_config = config_root / "comfyui_*_default.json"
    if wildcard_config.exists():
        return True

    return False
```

### Incremental Sync After Import

After models are imported, sync only the newly imported models to all installed apps.

**Implementation**: See [Performance & Data Integrity](01-performance-and-integrity.md#3-incremental-sync-strategy)

**Key Points**:
- Batch imports trigger a single sync at the end, not per-file
- Only newly imported model IDs are processed
- ~2200× faster than full tree scan

### Manual Sync API

User-triggered sync for a specific app version.

**Implementation**: `backend/api/core.py`

```python
def sync_app_models(self, app_id: str, version: str) -> dict:
    """
    Manually sync models for a specific app version.

    This function:
    1. Removes broken symlinks (targets no longer exist)
    2. Re-applies all mapping configurations
    3. Recreates missing symlinks (e.g., after moving Pumas-Library folder)
    4. Creates new symlinks for newly imported models

    Returns:
        {
            'success': bool,
            'links_created': int,
            'links_removed': int,
            'total_links': int
        }
    """
    try:
        if app_id == "comfyui":
            models_root = Path(f"comfyui-versions/{version}/models")
        else:
            return {'success': False, 'error': f'Unsupported app: {app_id}'}

        if not models_root.exists():
            return {'success': False, 'error': f'App not installed: {version}'}

        # Step 1: Clean broken symlinks (targets deleted or moved)
        links_removed = self._clean_broken_symlinks(models_root)

        # Step 2: Re-apply all mappings (recreates missing links, adds new ones)
        # This handles:
        # - Broken relative symlinks after folder move
        # - Newly imported models
        # - Models added to library externally
        links_created = self.mapper.apply_for_app(app_id, version, models_root)

        return {
            'success': True,
            'links_created': links_created,
            'links_removed': links_removed,
            'total_links': links_created
        }
    except Exception as e:
        logger.error(f"Error syncing models: {e}")
        return {'success': False, 'error': str(e)}


def _clean_broken_symlinks(self, root: Path) -> int:
    """Remove broken symlinks in app model directories."""
    count = 0
    for subdir in root.iterdir():
        if not subdir.is_dir():
            continue
        for item in subdir.iterdir():
            if item.is_symlink() and not item.exists():
                item.unlink()
                count += 1
                logger.debug(f"Removed broken symlink: {item}")
    return count


def garbage_collect_orphaned_links(self, app_id: str, version: str) -> dict:
    """
    Remove symlinks that no longer match any active mapping rules.

    This handles the case where a user changes mapping filters
    (e.g., from "all checkpoints" to "only SDXL"), leaving old
    symlinks that are no longer valid.

    Returns:
        {
            'success': bool,
            'orphaned_links_removed': int,
            'broken_links_removed': int
        }
    """
    if app_id == "comfyui":
        models_root = Path(f"comfyui-versions/{version}/models")
    else:
        return {'success': False, 'error': f'Unsupported app: {app_id}'}

    if not models_root.exists():
        return {'success': False, 'error': f'App not installed: {version}'}

    # Load current mapping config
    config = self.mapper._load_and_merge_configs(app_id, version)
    if not config:
        return {'success': False, 'error': 'No mapping config found'}

    orphaned = 0
    broken = 0

    # Scan all existing symlinks
    for subdir in models_root.iterdir():
        if not subdir.is_dir():
            continue

        for link in subdir.iterdir():
            if not link.is_symlink():
                continue

            # Check if link target exists
            if not link.exists():
                link.unlink()
                broken += 1
                logger.debug(f"Removed broken link: {link}")
                continue

            # Check if link still matches any mapping rule
            link_target = link.resolve()
            matches_rule = False

            for mapping in config.get('mappings', []):
                if not mapping.get('enabled', True):
                    continue

                # Get models that match this rule's filters
                models = self.mapper._get_matching_models(mapping.get('filters', {}))

                for metadata in models:
                    model_dir = self.library_root / metadata['library_path']

                    # Check if link points to a file in this model directory
                    if link_target.is_relative_to(model_dir):
                        matches_rule = True
                        break

                if matches_rule:
                    break

            if not matches_rule:
                # Orphaned: Was created by old rule, no longer matches
                link.unlink()
                orphaned += 1
                logger.info(f"Removed orphaned link: {link} (no longer matches filters)")

    return {
        'success': True,
        'orphaned_links_removed': orphaned,
        'broken_links_removed': broken
    }
```

### Clean Uninstall

When a ComfyUI version is deleted, remove all symlinks pointing to the library.

**Implementation**: Update `backend/version_manager.py`

```python
def delete_version(self, version_tag: str):
    """Delete a ComfyUI version and clean up symlinks."""
    version_path = self._get_version_path(version_tag)

    if not version_path.exists():
        raise ValueError(f"Version {version_tag} not found")

    try:
        # Clean up model symlinks first
        models_root = version_path / "models"
        if models_root.exists():
            self._clean_model_symlinks(models_root)

        # Delete the version directory
        shutil.rmtree(version_path)

        logger.info(f"Deleted version {version_tag}")

    except Exception as e:
        logger.error(f"Failed to delete version {version_tag}: {e}")
        raise


def _clean_model_symlinks(self, models_root: Path):
    """Remove all symlinks in model directories."""
    count = 0
    for subdir in models_root.rglob('*'):
        if subdir.is_symlink():
            subdir.unlink()
            count += 1

    logger.info(f"Removed {count} model symlinks from {models_root}")
```

---

## Version Constraints

### Overview

Models can restrict which app versions they link to using `overrides.json` with PEP 440 version specifiers.

### Behavior

- **Missing file**: Model links to all versions (default)
- **Valid file**: Model only links to versions matching constraints
- **Invalid file**: WARNING logged, model excluded from all mappings until fixed

### Example

`shared-resources/models/diffusion/stable-diffusion/sd-v1-5/overrides.json`:

```json
{
  "version_ranges": {
    "comfyui": ">=0.5.0,<0.7.0",
    "automatic1111": "*"
  }
}
```

### Implementation

```python
def _version_allowed(self, model_dir: Path, app_id: str, app_version: str) -> bool:
    """
    Check if model is allowed for this app version based on overrides.json.

    Behavior:
    - No overrides.json: Allow all (return True)
    - Valid constraint: Check version match
    - Invalid format/constraint: WARNING + exclude (return False)
    """
    overrides = self.library.load_overrides(model_dir)

    # No override file = no constraints
    if not overrides:
        return True

    ranges = overrides.get("version_ranges", {})

    # Invalid format
    if not isinstance(ranges, dict):
        logger.warning(
            f"Invalid overrides.json format in {model_dir}: "
            f"expected dict, got {type(ranges).__name__}. "
            f"Excluding model from all mappings."
        )
        return False

    # Check for this app's constraint
    target_range = ranges.get(app_id.lower())
    if not target_range:
        return True  # No constraint for this app

    # Validate and check constraint
    try:
        from packaging.specifiers import SpecifierSet
        from packaging.version import Version

        spec = SpecifierSet(str(target_range))
        version = Version(app_version)
        return version in spec
    except Exception as exc:
        logger.warning(
            f"Invalid version constraint '{target_range}' for {model_dir}/{app_id}: {exc}. "
            f"Excluding model from mapping. Fix overrides.json to resolve."
        )
        return False
```

---

## Link Types

### Strategy

The mapper determines the optimal link type based on filesystem compatibility:

**Same Filesystem**:
- **ext4/btrfs/xfs**: Relative symlinks (portable, recommended)
- **NTFS**: Hard links (NTFS symlinks are unreliable on Linux)

**Cross-Filesystem** (library and app on different drives):
- **Absolute symlinks** with UI warnings about drive unmounting

### Implementation

See [Performance & Data Integrity - Link Type Strategy](01-performance-and-integrity.md#link-type-strategy)

### Mapping Preview (Dry Run)

**Purpose**: Preview all changes before applying mappings to avoid surprises and conflicts.

**Implementation**: Add dry-run mode to mapping engine

```python
from typing import List, Dict, Literal
from dataclasses import dataclass
from pathlib import Path

@dataclass
class MappingAction:
    """Represents a single mapping operation to be performed."""
    action_type: Literal['create', 'skip_exists', 'skip_conflict', 'remove_broken']
    model_id: str
    model_name: str
    source_path: Path
    target_path: Path
    link_type: str
    reason: str = ""
    existing_target: str = ""  # For conflicts, what currently exists

@dataclass
class MappingPreview:
    """Complete preview of all mapping operations."""
    to_create: List[MappingAction]
    to_skip_exists: List[MappingAction]
    conflicts: List[MappingAction]
    broken_to_remove: List[MappingAction]
    total_actions: int
    warnings: List[str]
    errors: List[str]


def preview_mapping(
    self,
    app_id: str,
    version: str,
    app_models_root: Path
) -> MappingPreview:
    """
    Preview all mapping operations without making changes.

    Args:
        app_id: Application ID ("comfyui")
        version: Version string ("0.6.0")
        app_models_root: Path to app's models/ directory

    Returns:
        MappingPreview with all planned operations
    """
    config = self._load_and_merge_configs(app_id, version)
    if not config:
        return MappingPreview(
            to_create=[],
            to_skip_exists=[],
            conflicts=[],
            broken_to_remove=[],
            total_actions=0,
            warnings=[],
            errors=[f"No mapping config found for {app_id} {version}"]
        )

    to_create = []
    to_skip_exists = []
    conflicts = []
    broken_to_remove = []
    warnings = []
    errors = []

    # Validate filesystem
    validation = fs_validator.validate_mapping_target(
        library_path=self.library_root,
        app_path=app_models_root,
        link_type='symlink'
    )

    if not validation['valid']:
        errors.extend(validation['errors'])
        return MappingPreview(
            to_create=[],
            to_skip_exists=[],
            conflicts=[],
            broken_to_remove=[],
            total_actions=0,
            warnings=warnings,
            errors=errors
        )

    if validation.get('warnings'):
        warnings.extend(validation['warnings'])

    link_type = self._determine_link_type(
        source=self.library_root,
        target_dir=app_models_root,
        validation_result=validation
    )

    # Phase 1: Find broken symlinks to remove
    for subdir in app_models_root.iterdir():
        if not subdir.is_dir():
            continue

        for item in subdir.iterdir():
            if item.is_symlink() and not item.exists():
                # Broken symlink
                try:
                    old_target = item.readlink()
                except:
                    old_target = "[unreadable]"

                broken_to_remove.append(MappingAction(
                    action_type='remove_broken',
                    model_id='',
                    model_name='',
                    source_path=Path(),
                    target_path=item,
                    link_type='symlink',
                    reason=f'Broken link (target missing)',
                    existing_target=str(old_target)
                ))

    # Phase 2: Preview all mapping operations
    for mapping in config.get('mappings', []):
        if not mapping.get('enabled', True):
            continue

        target_subdir = mapping['target_subdir']
        target_dir = app_models_root / target_subdir

        # Get models matching filters
        models = self._get_matching_models(mapping.get('filters', {}))

        for metadata in models:
            model_dir = self.library_root / metadata['library_path']

            # For single-file models
            if not metadata.get('is_sharded_set', False):
                for pattern in mapping.get('patterns', ['*.safetensors']):
                    for model_file in model_dir.glob(pattern):
                        if model_file.name == 'metadata.json':
                            continue

                        target_path = target_dir / model_file.name
                        action = self._preview_single_link(
                            model_id=metadata['model_id'],
                            model_name=metadata['official_name'],
                            source_path=model_file,
                            target_path=target_path,
                            link_type=link_type
                        )

                        if action.action_type == 'create':
                            to_create.append(action)
                        elif action.action_type == 'skip_exists':
                            to_skip_exists.append(action)
                        elif action.action_type == 'skip_conflict':
                            conflicts.append(action)

            # For sharded sets
            else:
                for file_entry in metadata.get('files', []):
                    source_path = model_dir / file_entry['name']
                    target_path = target_dir / file_entry['name']

                    action = self._preview_single_link(
                        model_id=metadata['model_id'],
                        model_name=metadata['official_name'],
                        source_path=source_path,
                        target_path=target_path,
                        link_type=link_type
                    )

                    if action.action_type == 'create':
                        to_create.append(action)
                    elif action.action_type == 'skip_exists':
                        to_skip_exists.append(action)
                    elif action.action_type == 'skip_conflict':
                        conflicts.append(action)

    total_actions = (
        len(to_create) +
        len(to_skip_exists) +
        len(conflicts) +
        len(broken_to_remove)
    )

    return MappingPreview(
        to_create=to_create,
        to_skip_exists=to_skip_exists,
        conflicts=conflicts,
        broken_to_remove=broken_to_remove,
        total_actions=total_actions,
        warnings=warnings,
        errors=errors
    )


def _preview_single_link(
    self,
    model_id: str,
    model_name: str,
    source_path: Path,
    target_path: Path,
    link_type: str
) -> MappingAction:
    """
    Preview a single link operation.

    Returns:
        MappingAction describing what would happen
    """
    # Check if target exists
    if target_path.exists() or target_path.is_symlink():
        if target_path.is_symlink():
            current_target = target_path.resolve()

            # Check if it already points to correct source
            if current_target == source_path.resolve():
                return MappingAction(
                    action_type='skip_exists',
                    model_id=model_id,
                    model_name=model_name,
                    source_path=source_path,
                    target_path=target_path,
                    link_type=link_type,
                    reason='Already linked to correct source',
                    existing_target=str(current_target)
                )

            # Conflict: points to different source
            return MappingAction(
                action_type='skip_conflict',
                model_id=model_id,
                model_name=model_name,
                source_path=source_path,
                target_path=target_path,
                link_type=link_type,
                reason='Symlink points to different source',
                existing_target=str(current_target)
            )

        # Conflict: non-symlink file exists
        return MappingAction(
            action_type='skip_conflict',
            model_id=model_id,
            model_name=model_name,
            source_path=source_path,
            target_path=target_path,
            link_type=link_type,
            reason='Non-symlink file exists at target',
            existing_target='[regular file]'
        )

    # Target doesn't exist - will create
    return MappingAction(
        action_type='create',
        model_id=model_id,
        model_name=model_name,
        source_path=source_path,
        target_path=target_path,
        link_type=link_type,
        reason='New symlink to be created'
    )
```

**API Endpoint**: Add preview endpoint

```python
# In backend/api/core.py

def preview_model_mapping(
    self,
    app_id: str,
    version: str
) -> Dict:
    """
    Preview mapping operations before applying.

    Returns:
        {
            'to_create': List[dict],      # New links to create
            'to_skip_exists': List[dict], # Already correct
            'conflicts': List[dict],       # Need resolution
            'broken_to_remove': List[dict], # Broken links
            'total_actions': int,
            'warnings': List[str],
            'errors': List[str]
        }
    """
    app_models_root = Path(f"comfyui-versions/{version}/models")

    if not app_models_root.exists():
        return {
            'to_create': [],
            'to_skip_exists': [],
            'conflicts': [],
            'broken_to_remove': [],
            'total_actions': 0,
            'warnings': [],
            'errors': [f'App not installed: {version}']
        }

    preview = self.mapper.preview_mapping(app_id, version, app_models_root)

    # Convert to serializable dict
    return {
        'to_create': [self._action_to_dict(a) for a in preview.to_create],
        'to_skip_exists': [self._action_to_dict(a) for a in preview.to_skip_exists],
        'conflicts': [self._action_to_dict(a) for a in preview.conflicts],
        'broken_to_remove': [self._action_to_dict(a) for a in preview.broken_to_remove],
        'total_actions': preview.total_actions,
        'warnings': preview.warnings,
        'errors': preview.errors
    }

def _action_to_dict(self, action: MappingAction) -> dict:
    """Convert MappingAction to JSON-serializable dict."""
    return {
        'action_type': action.action_type,
        'model_id': action.model_id,
        'model_name': action.model_name,
        'source_path': str(action.source_path),
        'target_path': str(action.target_path),
        'link_type': action.link_type,
        'reason': action.reason,
        'existing_target': action.existing_target
    }
```

**UI Integration**: Preview dialog before sync

```tsx
interface MappingPreviewDialogProps {
  appId: string;
  version: string;
  onConfirm: () => void;
  onCancel: () => void;
}

function MappingPreviewDialog({ appId, version, onConfirm, onCancel }: MappingPreviewDialogProps) {
  const [preview, setPreview] = useState<MappingPreview | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.previewModelMapping(appId, version).then(result => {
      setPreview(result);
      setLoading(false);
    });
  }, [appId, version]);

  if (loading) {
    return <div>Loading preview...</div>;
  }

  if (!preview) {
    return <div>Failed to load preview</div>;
  }

  const hasConflicts = preview.conflicts.length > 0;
  const hasBrokenLinks = preview.broken_to_remove.length > 0;
  const hasChanges = preview.to_create.length > 0 || hasBrokenLinks;

  return (
    <Dialog>
      <DialogTitle>
        Mapping Preview: {appId} v{version}
      </DialogTitle>

      <DialogContent className="space-y-4">
        {/* Summary */}
        <div className="grid grid-cols-4 gap-4 p-4 bg-gray-50 rounded">
          <div>
            <div className="text-2xl font-bold text-green-600">{preview.to_create.length}</div>
            <div className="text-sm text-gray-600">New Links</div>
          </div>
          <div>
            <div className="text-2xl font-bold text-blue-600">{preview.to_skip_exists.length}</div>
            <div className="text-sm text-gray-600">Already Correct</div>
          </div>
          <div>
            <div className="text-2xl font-bold text-yellow-600">{preview.conflicts.length}</div>
            <div className="text-sm text-gray-600">Conflicts</div>
          </div>
          <div>
            <div className="text-2xl font-bold text-red-600">{preview.broken_to_remove.length}</div>
            <div className="text-sm text-gray-600">Broken Links</div>
          </div>
        </div>

        {/* Errors */}
        {preview.errors.length > 0 && (
          <div className="p-3 bg-red-50 border border-red-200 rounded">
            <h4 className="font-semibold text-red-800">Errors</h4>
            <ul className="mt-2 space-y-1">
              {preview.errors.map((error, i) => (
                <li key={i} className="text-sm text-red-700">{error}</li>
              ))}
            </ul>
          </div>
        )}

        {/* Warnings */}
        {preview.warnings.length > 0 && (
          <div className="p-3 bg-yellow-50 border border-yellow-200 rounded">
            <h4 className="font-semibold text-yellow-800">Warnings</h4>
            <ul className="mt-2 space-y-1">
              {preview.warnings.map((warning, i) => (
                <li key={i} className="text-sm text-yellow-700">{warning}</li>
              ))}
            </ul>
          </div>
        )}

        {/* Broken Links to Remove */}
        {hasBrokenLinks && (
          <details className="border rounded p-3">
            <summary className="cursor-pointer font-semibold text-red-700">
              {preview.broken_to_remove.length} Broken Links to Remove
            </summary>
            <ul className="mt-2 space-y-2 pl-4">
              {preview.broken_to_remove.map((action, i) => (
                <li key={i} className="text-sm">
                  <div className="font-mono text-red-600">{action.target_path}</div>
                  <div className="text-gray-600">→ {action.existing_target}</div>
                  <div className="text-xs text-gray-500">{action.reason}</div>
                </li>
              ))}
            </ul>
          </details>
        )}

        {/* New Links to Create */}
        {preview.to_create.length > 0 && (
          <details className="border rounded p-3">
            <summary className="cursor-pointer font-semibold text-green-700">
              {preview.to_create.length} New Links to Create
            </summary>
            <ul className="mt-2 space-y-2 pl-4">
              {preview.to_create.slice(0, 10).map((action, i) => (
                <li key={i} className="text-sm">
                  <div className="font-medium">{action.model_name}</div>
                  <div className="font-mono text-xs text-gray-600">
                    {action.target_path}
                  </div>
                  <div className="text-xs text-gray-500">
                    {action.link_type} → {action.source_path}
                  </div>
                </li>
              ))}
              {preview.to_create.length > 10 && (
                <li className="text-sm text-gray-500 italic">
                  ... and {preview.to_create.length - 10} more
                </li>
              )}
            </ul>
          </details>
        )}

        {/* Conflicts */}
        {hasConflicts && (
          <div className="border border-yellow-300 rounded p-3 bg-yellow-50">
            <h4 className="font-semibold text-yellow-800 flex items-center gap-2">
              <AlertTriangle className="w-5 h-5" />
              {preview.conflicts.length} Conflicts Detected
            </h4>
            <p className="text-sm text-yellow-700 mt-1">
              These files already exist and will be skipped. Delete them manually if you want to replace them.
            </p>
            <ul className="mt-2 space-y-2 pl-4">
              {preview.conflicts.map((action, i) => (
                <li key={i} className="text-sm">
                  <div className="font-mono text-yellow-800">{action.target_path}</div>
                  <div className="text-xs text-gray-600">{action.reason}</div>
                  <div className="text-xs text-gray-500">Existing: {action.existing_target}</div>
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* No Changes */}
        {!hasChanges && !hasConflicts && (
          <div className="p-4 bg-blue-50 border border-blue-200 rounded text-center">
            <CheckCircle className="w-12 h-12 mx-auto text-blue-600 mb-2" />
            <p className="text-blue-800 font-medium">All models are already synced</p>
            <p className="text-sm text-blue-600 mt-1">No changes needed</p>
          </div>
        )}
      </DialogContent>

      <DialogActions>
        <button onClick={onCancel} className="px-4 py-2 text-gray-700 hover:bg-gray-100 rounded">
          Cancel
        </button>
        <button
          onClick={onConfirm}
          disabled={!hasChanges}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-300"
        >
          {hasChanges ? 'Apply Changes' : 'Close'}
        </button>
      </DialogActions>
    </Dialog>
  );
}
```

**Usage Flow**:

1. User clicks "Sync Library Models" in Settings
2. Frontend calls `/api/mapping/preview` to get dry-run results
3. Preview dialog shows summary and detailed breakdown
4. User reviews and clicks "Apply Changes"
5. Frontend calls `/api/mapping/sync` to execute changes
6. Success/failure notification shown

### Conflict Resolution

When applying mappings, if a symlink already exists:

```python
def _create_link(self, source: Path, target: Path) -> bool:
    """Create symlink with conflict handling."""
    if target.exists() or target.is_symlink():
        # Check if it points to the same source
        if target.is_symlink():
            current_source = target.resolve()
            if current_source == source.resolve():
                logger.debug(f"Symlink already correct: {target}")
                return False  # No change needed (idempotent)

        # Conflict: different source
        logger.warning(
            f"Symlink conflict at {target}: "
            f"existing -> {target.resolve() if target.is_symlink() else 'non-symlink'}, "
            f"new -> {source}. Skipping."
        )
        return False  # Skip (safe default)

    # Create new symlink
    ensure_directory(target.parent)
    return make_relative_symlink(source, target)
```

**Conflict Strategy**: SKIP + WARN (safe, predictable, logged)

**Phase 2 Behavior**: Conflicts are logged as warnings. Users can view conflict summary via "Sync Library Models" button in Settings, then manually resolve via file manager if needed.

**Phase 3 Enhancement**: Interactive UI for conflict resolution with options:
- Show conflicting file details (size, date, symlink target)
- Options: [Rename Existing] [Overwrite with Link] [Skip] [Skip All]
- Remember choice for batch operations
- Display conflicts in dedicated dialog before sync

---

## Backend Implementation

### Mapping Manager

**File**: `backend/model_library/mapper.py`

```python
def apply_for_app(self, app_id: str, version: str, app_models_root: Path) -> int:
    """
    Apply mapping configuration to create symlinks.

    Args:
        app_id: Application ID ("comfyui")
        version: Version string ("0.6.0")
        app_models_root: Path to app's models/ directory

    Returns:
        Number of symlinks created
    """
    config = self._load_and_merge_configs(app_id, version)
    if not config:
        logger.warning(f"No mapping config found for {app_id} {version}")
        return 0

    # Validate filesystem and get link type recommendation
    validation = fs_validator.validate_mapping_target(
        library_path=self.library_root,
        app_path=app_models_root,
        link_type='symlink'
    )

    if not validation['valid']:
        raise ValueError(f"Mapping validation failed: {validation['errors']}")

    link_type = self._determine_link_type(
        source=self.library_root,
        target_dir=app_models_root,
        validation_result=validation
    )

    logger.info(f"Using {link_type} for mapping (filesystem compatibility)")

    links_created = 0

    # Apply each mapping rule
    for mapping in config.get('mappings', []):
        if not mapping.get('enabled', True):
            continue

        target_subdir = mapping['target_subdir']
        target_dir = app_models_root / target_subdir

        # Get models matching filters
        models = self._get_matching_models(mapping.get('filters', {}))

        for metadata in models:
            model_dir = self.library_root / metadata['library_path']

            # Check version constraints
            if not self._version_allowed(model_dir, app_id, version):
                continue

            # Find matching files
            patterns = mapping.get('patterns', ['*'])
            rule_link_type = mapping.get('link_type', 'file')

            for source_file in self._iter_matching_files(model_dir, patterns, rule_link_type):
                target_path = target_dir / source_file.name

                # Create link
                if self._create_link_with_type(source_file, target_path, link_type):
                    links_created += 1

    logger.info(f"Created {links_created} symlinks for {app_id} {version}")
    return links_created


def _iter_matching_files(
    self,
    model_dir: Path,
    patterns: Iterable[str],
    link_type: str = "file"
) -> Iterable[Path]:
    """
    Iterate matching files or directories based on link_type.

    Args:
        model_dir: Model directory to search
        patterns: Glob patterns to match
        link_type: "file" or "directory"
    """
    seen = set()
    for pattern in patterns:
        for candidate in model_dir.glob(pattern):
            if candidate in seen:
                continue

            # Skip metadata files
            if candidate.name in ("metadata.json", "overrides.json", "preview.png"):
                continue

            # Type filtering
            if link_type == "file" and not candidate.is_file():
                continue
            if link_type == "directory" and not candidate.is_dir():
                continue

            seen.add(candidate)
            yield candidate
```

### Incremental Sync

**File**: `backend/model_library/mapper.py`

```python
def sync_models_incremental(
    self,
    app_id: str,
    version: str,
    models_root: Path,
    model_ids: List[str]
) -> dict:
    """
    Incrementally sync specific models only.

    Args:
        app_id: Application ID
        version: Version string
        models_root: Path to app's models/ directory
        model_ids: List of model IDs to process

    Returns:
        {
            'links_created': int,
            'links_updated': int,
            'links_skipped': int
        }
    """
    config = self._load_and_merge_configs(app_id, version)
    if not config:
        logger.warning(f"No mapping config found for {app_id} {version}")
        return {'links_created': 0, 'links_updated': 0, 'links_skipped': 0}

    links_created = 0
    links_updated = 0
    links_skipped = 0

    # Get metadata for specified models only
    models_metadata = []
    for model_id in model_ids:
        metadata = self.library.get_model_by_id(model_id)
        if metadata:
            models_metadata.append(metadata)

    # Apply mappings only for these models
    for mapping in config.get('mappings', []):
        if not mapping.get('enabled', True):
            continue

        target_subdir = mapping['target_subdir']
        target_dir = models_root / target_subdir

        for metadata in models_metadata:
            # Check if model matches this mapping's filters
            if not self._matches_filters(metadata, mapping.get('filters', {})):
                continue

            model_dir = self.library_root / metadata['library_path']

            # Check version constraints
            if not self._version_allowed(model_dir, app_id, version):
                continue

            # Find matching files
            patterns = mapping.get('patterns', ['*'])
            rule_link_type = mapping.get('link_type', 'file')

            for source_file in self._iter_matching_files(model_dir, patterns, rule_link_type):
                target_path = target_dir / source_file.name

                # Check if link already exists and is correct
                if target_path.exists() or target_path.is_symlink():
                    if target_path.is_symlink() and target_path.resolve() == source_file.resolve():
                        links_skipped += 1
                        continue
                    else:
                        # Update existing link
                        target_path.unlink()
                        links_updated += 1

                # Create link
                if self._create_link(source_file, target_path):
                    links_created += 1

    return {
        'links_created': links_created,
        'links_updated': links_updated,
        'links_skipped': links_skipped
    }
```

### Sandbox Detection

**File**: `backend/model_library/mapper.py`

```python
def detect_sandbox_environment() -> dict:
    """
    Detect if running in a sandboxed environment.

    Returns:
        {
            'sandboxed': bool,
            'type': 'flatpak' | 'snap' | 'docker' | None,
            'permissions_needed': list[str]
        }
    """
    sandboxed = False
    sandbox_type = None
    permissions = []

    # Check for Flatpak
    if Path("/.flatpak-info").exists():
        sandboxed = True
        sandbox_type = "flatpak"
        permissions = [
            "Filesystem access to library directory",
            "Filesystem access to ComfyUI directory"
        ]

    # Check for Snap
    elif "SNAP" in os.environ:
        sandboxed = True
        sandbox_type = "snap"
        permissions = ["Connect 'removable-media' interface"]

    # Check for Docker
    elif Path("/.dockerenv").exists():
        sandboxed = True
        sandbox_type = "docker"
        permissions = ["Mount library directory as volume"]

    return {
        'sandboxed': sandboxed,
        'type': sandbox_type,
        'permissions_needed': permissions
    }
```

---

## Frontend Integration

### Settings UI

Add manual sync button and sandbox warning to Settings or Version Manager.

**Sandbox Warning**:

```tsx
{sandboxDetected && (
  <div className="bg-[hsl(var(--accent-warning)/0.1)] border-l-4 border-[hsl(var(--accent-warning))] p-4 mb-4">
    <div className="flex items-start gap-3">
      <HardDrive className="w-5 h-5 text-[hsl(var(--accent-warning))] mt-0.5" />
      <div className="flex-1">
        <h4 className="font-semibold text-sm mb-1">Sandbox Environment Detected</h4>
        <p className="text-xs text-[hsl(var(--text-secondary))] mb-2">
          Pumas Library is running in a {sandboxType} sandbox. Model symlinks may fail without proper filesystem permissions.
        </p>
        <ul className="text-xs text-[hsl(var(--text-secondary))] list-disc list-inside space-y-1">
          {permissionsNeeded.map(permission => (
            <li key={permission}>{permission}</li>
          ))}
        </ul>
        <p className="text-xs mt-2 text-[hsl(var(--text-secondary))]">
          Use <strong>Flatseal</strong> or your sandbox manager to grant these permissions.
        </p>
      </div>
    </div>
  </div>
)}
```

**Cross-Filesystem Warning**:

```tsx
{crossFilesystemDetected && (
  <div className="bg-[hsl(var(--accent-warning)/0.1)] border-l-4 border-[hsl(var(--accent-warning))] p-4 mb-4">
    <div className="flex items-start gap-3">
      <HardDrive className="w-5 h-5 text-[hsl(var(--accent-warning))] mt-0.5" />
      <div className="flex-1">
        <h4 className="font-semibold text-sm mb-1">External Drive Links Active</h4>
        <p className="text-xs text-[hsl(var(--text-secondary))] mb-2">
          Your model library is on a different drive than ComfyUI.
          Links use absolute paths and will break if:
        </p>
        <ul className="text-xs text-[hsl(var(--text-secondary))] list-disc list-inside space-y-1">
          <li>Library drive is unplugged</li>
          <li>Drive mount point changes (e.g., /media/usb → /media/usb2)</li>
          <li>System is moved to a different computer</li>
        </ul>
        <p className="text-xs mt-2">
          <strong>Recommended:</strong> Move library to same drive as ComfyUI for portable relative symlinks.
        </p>
      </div>
    </div>
  </div>
)}
```

**Manual Sync Button**:

```tsx
<button
  onClick={() => handleSyncModels(activeVersion)}
  className="flex items-center gap-2 px-3 py-1.5 rounded bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))]"
>
  <RefreshCw className="w-4 h-4" />
  <span className="text-sm">Sync Library Models</span>
</button>
```

---

## Testing Strategy

### Unit Tests

- [ ] Config loading (version-specific and wildcard)
- [ ] Filter matching (model_type, subtype, tags, family)
- [ ] Tag filter logic (AND/OR, exclusion wins)
- [ ] Version constraint checking (overrides.json)
- [ ] Pattern matching (globs)
- [ ] Relative symlink creation
- [ ] Absolute symlink creation
- [ ] Hard link creation
- [ ] Config precedence calculation
- [ ] Config merging
- [ ] Mapping preview: Detect new links to create
- [ ] Mapping preview: Detect existing correct links
- [ ] Mapping preview: Detect conflicts (different source)
- [ ] Mapping preview: Detect conflicts (non-symlink file)
- [ ] Mapping preview: Detect broken links to remove
- [ ] Mapping preview: Sharded set handling (all files previewed)

### Integration Tests

- [ ] Default mapping auto-applies on install
- [ ] Manual "Sync Models" button works
- [ ] Symlinks are relative on same filesystem
- [ ] Absolute symlinks used for cross-filesystem
- [ ] Broken symlink cleanup works
- [ ] Multiple ComfyUI versions don't conflict
- [ ] Model added to library → appears in ComfyUI after incremental sync
- [ ] Version constraints filter models correctly
- [ ] Tag filtering works (AND/OR logic, exclusion)
- [ ] Dynamic directory scanning: Custom nodes detected
- [ ] Sandbox detection: Flatpak/Snap/Docker warnings shown
- [ ] Cross-filesystem detection: Absolute symlinks with warnings
- [ ] Clean uninstall: Version deletion removes symlinks
- [ ] Incremental sync: Only new models processed
- [ ] File vs directory link types work correctly
- [ ] Mapping preview workflow: Click sync → Preview dialog → Apply → Success
- [ ] Mapping preview: Empty library shows "No changes needed"
- [ ] Mapping preview: Conflicts block sync with warnings
- [ ] Mapping preview API returns correct counts

### System Tests

- [ ] End-to-end: Import model → Auto-maps to ComfyUI → Appears in app
- [ ] Multiple variants can coexist
- [ ] Config changes apply immediately
- [ ] Preview shows accurate model list
- [ ] **The "Move Test"**: Move entire Pumas-Library folder to new mount point - relative symlinks remain valid
- [ ] Deep Scan rebuild: Delete SQLite DB → Rebuild from metadata.json → All models restored
- [ ] Retry pending lookups: Models imported offline → Connect to internet → Retry enriches metadata

---

**End of Mapping System Document**
