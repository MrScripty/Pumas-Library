"""Tests for the model library mapper."""

from pathlib import Path
from typing import Any, Dict, Optional

from backend.model_library.library import ModelLibrary
from backend.model_library.mapper import ModelMapper
from backend.models import ModelMetadata, ModelOverrides


def _create_model(library: ModelLibrary, overrides: Optional[ModelOverrides] = None) -> Path:
    model_dir = library.library_root / "diffusion" / "family" / "model-a"
    model_dir.mkdir(parents=True, exist_ok=True)
    model_file = model_dir / "model.safetensors"
    model_file.write_text("data")

    metadata: ModelMetadata = {
        "model_id": "model-a",
        "family": "family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Model A",
        "cleaned_name": "model-a",
        "tags": ["stable-diffusion"],
        "base_model": "",
        "preview_image": "",
        "release_date": "",
        "download_url": "",
        "model_card": {},
        "inference_settings": {},
        "compatible_apps": [],
        "hashes": {"sha256": "", "blake3": ""},
        "notes": "",
        "added_date": "2024-01-01T00:00:00Z",
        "updated_date": "2024-01-01T00:00:00Z",
        "size_bytes": model_file.stat().st_size,
        "files": [
            {
                "name": model_file.name,
                "original_name": "model.safetensors",
                "size": model_file.stat().st_size,
            }
        ],
    }

    library.save_metadata(model_dir, metadata)
    library.save_overrides(model_dir, overrides or {})
    library.index_model_dir(model_dir, metadata)
    return model_dir


def _write_mapping_config(config_root: Path, version: str) -> None:
    config_path = config_root / f"comfyui_{version}_default.json"
    config_path.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "filters": {
        "model_type": ["diffusion"],
        "subtypes": ["checkpoints"],
        "tags": ["stable-diffusion"]
      },
      "method": "symlink"
    }
  ]
}
""".strip()
    )


def test_apply_for_app_creates_symlink(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "0.1.0")

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)

    assert links == 1
    linked_file = app_root / "checkpoints" / "model.safetensors"
    assert linked_file.exists()
    assert linked_file.is_symlink()


def test_apply_for_app_respects_version_range(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library, {"version_ranges": {"comfyui": ">=0.2.0"}})

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "0.1.0")

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)

    assert links == 0


def test_apply_for_app_handles_collisions(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "0.1.0")

    app_root = tmp_path / "app" / "models"
    collision_path = app_root / "checkpoints" / "model.safetensors"
    collision_path.parent.mkdir(parents=True, exist_ok=True)
    collision_path.write_text("existing")

    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)

    assert links == 1
    suffix_path = app_root / "checkpoints" / "model-2.safetensors"
    assert suffix_path.exists()
    assert suffix_path.is_symlink()


def test_load_configs_ignores_invalid_json(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)

    bad_config = config_root / "comfyui_0.1.0_default.json"
    bad_config.write_text("{not-json}")
    other_config = config_root / "other_0.1.0_default.json"
    other_config.write_text("{}")

    configs = mapper._load_configs("comfyui", "0.1.0")
    assert configs == []


def test_apply_for_app_skips_non_symlink_method(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    config_path = config_root / "comfyui_0.1.0_default.json"
    config_path.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "method": "config"
    }
  ]
}
""".strip()
    )

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)
    assert links == 0


def test_apply_for_app_missing_target_subdir(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    config_path = config_root / "comfyui_0.1.0_default.json"
    config_path.write_text(
        """
{
  "mappings": [
    {
      "patterns": ["*.safetensors"],
      "method": "symlink"
    }
  ]
}
""".strip()
    )

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)
    assert links == 0


def test_apply_for_app_handles_invalid_range(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library, {"version_ranges": {"comfyui": "not-a-spec"}})

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "0.1.0")

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)

    assert links == 1


def test_apply_for_app_handles_string_filters(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    config_path = config_root / "comfyui_0.1.0_default.json"
    config_path.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "checkpoints",
      "patterns": "*.safetensors",
      "filters": {
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "families": "family",
        "tags": "stable-diffusion"
      },
      "method": "symlink"
    }
  ]
}
""".strip()
    )

    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)

    assert links == 1


def test_iter_matching_files_skips_metadata(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    mapper = ModelMapper(library, tmp_path / "config")

    model_dir = tmp_path / "model"
    model_dir.mkdir()
    (model_dir / "metadata.json").write_text("{}")
    (model_dir / "overrides.json").write_text("{}")
    data_file = model_dir / "weights.bin"
    data_file.write_text("data")

    results = list(mapper._iter_matching_files(model_dir, ["*"]))
    assert results == [data_file]


def test_create_link_skips_existing_file(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    mapper = ModelMapper(library, tmp_path / "config")

    source = tmp_path / "source.bin"
    source.write_text("data")
    target = tmp_path / "target.bin"
    target.write_text("existing")

    assert mapper._create_link(source, target) is False


# ============================================================================
# Phase 1C Tests: Config Loading, Merging, and Preview
# ============================================================================


def test_wildcard_config_matches_any_version(tmp_path: Path):
    """Test that wildcard configs (comfyui_*_default.json) match any version."""
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)

    # Create wildcard config
    config_path = config_root / "comfyui_*_default.json"
    config_path.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "method": "symlink"
    }
  ]
}
""".strip()
    )

    # Should match any version
    app_root = tmp_path / "app" / "models"
    links = mapper.apply_for_app("comfyui", "0.1.0", app_root)
    assert links == 1

    # Also test a different version
    app_root2 = tmp_path / "app2" / "models"
    links2 = mapper.apply_for_app("comfyui", "1.5.3", app_root2)
    assert links2 == 1


def test_specific_config_overrides_wildcard(tmp_path: Path):
    """Test that specific version configs take precedence over wildcards."""
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)

    # Create wildcard config
    wildcard_config = config_root / "comfyui_*_default.json"
    wildcard_config.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "checkpoints",
      "patterns": ["*.safetensors"],
      "method": "symlink",
      "priority": 10
    }
  ]
}
""".strip()
    )

    # Create specific version config with different target
    specific_config = config_root / "comfyui_0.2.0_default.json"
    specific_config.write_text(
        """
{
  "mappings": [
    {
      "target_subdir": "stable-diffusion",
      "patterns": ["*.safetensors"],
      "method": "symlink",
      "priority": 5
    }
  ]
}
""".strip()
    )

    # Version 0.2.0 should get both mappings (merged), sorted by priority
    merged = mapper._load_and_merge_configs("comfyui", "0.2.0")
    assert merged is not None
    assert len(merged["mappings"]) == 2
    # Sorted by priority, priority 5 comes first
    assert merged["mappings"][0]["target_subdir"] == "stable-diffusion"
    assert merged["mappings"][1]["target_subdir"] == "checkpoints"


def test_calculate_specificity(tmp_path: Path):
    """Test config specificity calculation."""
    library = ModelLibrary(tmp_path / "models")
    mapper = ModelMapper(library, tmp_path / "config")

    # Wildcard + default = lowest
    assert mapper._calculate_specificity("*", "default") == 0

    # Wildcard + custom variant
    assert mapper._calculate_specificity("*", "sdxl-only") == 10

    # Exact version + default
    assert mapper._calculate_specificity("0.6.0", "default") == 100

    # Exact version + custom = highest
    assert mapper._calculate_specificity("0.6.0", "custom") == 110


def test_exclude_tags_filter(tmp_path: Path):
    """Test that exclude_tags filter works correctly."""
    library = ModelLibrary(tmp_path / "models")

    # Create model with "deprecated" tag
    model_dir = library.library_root / "diffusion" / "family" / "model-deprecated"
    model_dir.mkdir(parents=True, exist_ok=True)
    model_file = model_dir / "model.safetensors"
    model_file.write_text("data")

    metadata: ModelMetadata = {
        "model_id": "model-deprecated",
        "family": "family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Deprecated Model",
        "cleaned_name": "model-deprecated",
        "tags": ["stable-diffusion", "deprecated"],  # Has deprecated tag
        "base_model": "",
        "preview_image": "",
        "release_date": "",
        "download_url": "",
        "model_card": {},
        "inference_settings": {},
        "compatible_apps": [],
        "hashes": {"sha256": "", "blake3": ""},
        "notes": "",
        "added_date": "2024-01-01T00:00:00Z",
        "updated_date": "2024-01-01T00:00:00Z",
        "size_bytes": model_file.stat().st_size,
        "files": [],
    }
    library.save_metadata(model_dir, metadata)
    library.index_model_dir(model_dir, metadata)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)

    # Test filter with exclude_tags
    filters = {"model_type": "diffusion", "exclude_tags": ["deprecated"]}
    metadata_dict: Dict[str, Any] = dict(metadata)
    assert mapper._matches_filters(metadata_dict, filters) is False

    # Same model without exclude should match
    filters_no_exclude = {"model_type": "diffusion"}
    assert mapper._matches_filters(metadata_dict, filters_no_exclude) is True


def test_discover_model_directories(tmp_path: Path):
    """Test dynamic directory discovery."""
    library = ModelLibrary(tmp_path / "models")
    mapper = ModelMapper(library, tmp_path / "config")

    # Create some directories
    models_root = tmp_path / "comfyui" / "models"
    (models_root / "checkpoints").mkdir(parents=True)
    (models_root / "loras").mkdir()
    (models_root / "ipadapter").mkdir()
    (models_root / ".hidden").mkdir()

    subdirs = mapper.discover_model_directories(models_root)

    assert "checkpoints" in subdirs
    assert "loras" in subdirs
    assert "ipadapter" in subdirs
    assert ".hidden" not in subdirs  # Hidden dirs excluded


def test_preview_mapping_shows_actions(tmp_path: Path):
    """Test that preview_mapping returns correct action preview."""
    from backend.model_library.mapper import MappingActionType

    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "*")  # Wildcard config

    app_root = tmp_path / "app" / "models"

    # Get preview before any links exist
    preview = mapper.preview_mapping("comfyui", "0.1.0", app_root)

    assert len(preview.to_create) == 1
    assert len(preview.to_skip_exists) == 0
    assert len(preview.conflicts) == 0
    assert preview.to_create[0].action_type == MappingActionType.CREATE


def test_preview_mapping_detects_existing_links(tmp_path: Path):
    """Test that preview detects already-linked files."""
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "*")

    app_root = tmp_path / "app" / "models"

    # Apply mapping first
    mapper.apply_for_app("comfyui", "0.1.0", app_root)

    # Now preview should show as "skip_exists"
    preview = mapper.preview_mapping("comfyui", "0.1.0", app_root)

    assert len(preview.to_create) == 0
    assert len(preview.to_skip_exists) == 1


def test_preview_mapping_detects_conflicts(tmp_path: Path):
    """Test that preview detects conflicts with non-symlink files."""
    library = ModelLibrary(tmp_path / "models")
    _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "*")

    app_root = tmp_path / "app" / "models"
    conflict_path = app_root / "checkpoints" / "model.safetensors"
    conflict_path.parent.mkdir(parents=True, exist_ok=True)
    conflict_path.write_text("existing file")

    preview = mapper.preview_mapping("comfyui", "0.1.0", app_root)

    assert len(preview.conflicts) == 1
    assert "Non-symlink file exists" in preview.conflicts[0].reason


def test_sync_models_incremental(tmp_path: Path):
    """Test incremental sync for specific models."""
    library = ModelLibrary(tmp_path / "models")
    model_dir = _create_model(library)

    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)
    _write_mapping_config(config_root, "*")

    app_root = tmp_path / "app" / "models"

    # Get the model_id (library_path)
    model_id = str(model_dir.relative_to(library.library_root))

    # Incremental sync for just this model
    result = mapper.sync_models_incremental("comfyui", "0.1.0", app_root, [model_id])

    assert result["links_created"] == 1
    assert result["links_updated"] == 0
    assert result["links_skipped"] == 0

    # Sync again - should skip
    result2 = mapper.sync_models_incremental("comfyui", "0.1.0", app_root, [model_id])

    assert result2["links_created"] == 0
    assert result2["links_skipped"] == 1


def test_check_mapping_config_exists(tmp_path: Path):
    """Test checking if mapping config exists."""
    library = ModelLibrary(tmp_path / "models")
    config_root = tmp_path / "config"
    mapper = ModelMapper(library, config_root)

    # No config yet
    assert mapper.check_mapping_config_exists("comfyui", "0.1.0") is False

    # Create wildcard config
    wildcard = config_root / "comfyui_*_default.json"
    wildcard.write_text('{"mappings": []}')

    # Should find via wildcard
    assert mapper.check_mapping_config_exists("comfyui", "0.1.0") is True
    assert mapper.check_mapping_config_exists("comfyui", "2.0.0") is True

    # Create specific config
    specific = config_root / "comfyui_0.5.0_default.json"
    specific.write_text('{"mappings": []}')

    assert mapper.check_mapping_config_exists("comfyui", "0.5.0") is True


# ============================================================================
# Phase 1C Tests: Sandbox Detection
# ============================================================================


def test_sandbox_detection_returns_info():
    """Test that sandbox detection returns proper SandboxInfo."""
    from backend.model_library.io import detect_sandbox_environment

    info = detect_sandbox_environment()

    # Should return a SandboxInfo dataclass
    assert hasattr(info, "is_sandboxed")
    assert hasattr(info, "sandbox_type")
    assert hasattr(info, "limitations")
    assert isinstance(info.is_sandboxed, bool)
    assert isinstance(info.sandbox_type, str)
    assert isinstance(info.limitations, list)


def test_get_cross_filesystem_warning_same_fs(tmp_path: Path):
    """Test that cross-filesystem warning returns None for same filesystem."""
    library = ModelLibrary(tmp_path / "models")
    mapper = ModelMapper(library, tmp_path / "config")

    # Both on same filesystem (tmp_path)
    app_models = tmp_path / "app" / "models"
    app_models.mkdir(parents=True, exist_ok=True)

    warning = mapper.get_cross_filesystem_warning(app_models)

    # Should not warn for same filesystem
    assert warning is None or warning.get("cross_filesystem") is False
