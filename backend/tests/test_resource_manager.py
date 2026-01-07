#!/usr/bin/env python3
"""Unit tests for ResourceManager (model library flow)."""

from pathlib import Path

import pytest

from backend.metadata_manager import MetadataManager
from backend.models import ModelMetadata
from backend.resources.resource_manager import ResourceManager


@pytest.fixture
def launcher_root(tmp_path):
    return tmp_path / "launcher"


@pytest.fixture
def metadata_manager(launcher_root):
    launcher_root.mkdir(parents=True, exist_ok=True)
    return MetadataManager(launcher_root / "launcher-data")


@pytest.fixture
def resource_manager(launcher_root, metadata_manager):
    return ResourceManager(launcher_root, metadata_manager)


def _create_model(resource_manager: ResourceManager) -> Path:
    model_dir = resource_manager.shared_models_dir / "diffusion" / "test-family" / "test-model"
    model_dir.mkdir(parents=True, exist_ok=True)
    model_file = model_dir / "model.safetensors"
    model_file.write_text("data")

    metadata: ModelMetadata = {
        "model_id": "test-model",
        "family": "test-family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Test Model",
        "cleaned_name": "test-model",
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
                "original_name": "Model.safetensors",
                "size": model_file.stat().st_size,
            }
        ],
    }

    resource_manager.model_library.save_metadata(model_dir, metadata)
    resource_manager.model_library.save_overrides(model_dir, {})
    resource_manager.model_library.index_model_dir(model_dir, metadata)
    return model_dir


def _write_mapping_config(resource_manager: ResourceManager, version: str) -> None:
    config_path = resource_manager.translation_config_dir / f"comfyui_{version}_default.json"
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


def test_initialize_shared_storage(resource_manager):
    assert resource_manager.initialize_shared_storage() is True
    assert resource_manager.shared_models_dir.exists()
    assert resource_manager.shared_user_dir.exists()
    assert resource_manager.shared_workflows_dir.exists()


def test_get_models_lists_library(resource_manager):
    _create_model(resource_manager)
    models = resource_manager.get_models()
    assert "diffusion/test-family/test-model" in models
    entry = models["diffusion/test-family/test-model"]
    assert entry["modelType"] == "checkpoints"


def test_scan_shared_storage_counts_models(resource_manager):
    _create_model(resource_manager)
    result = resource_manager.scan_shared_storage()
    assert result["modelsFound"] == 1
    assert result["totalSize"] > 0


def test_setup_version_symlinks_maps_models(resource_manager):
    resource_manager.initialize_shared_storage()
    _create_model(resource_manager)

    version_tag = "v0.1.0"
    version_dir = resource_manager.versions_dir / version_tag
    version_dir.mkdir(parents=True, exist_ok=True)

    _write_mapping_config(resource_manager, "0.1.0")

    assert resource_manager.setup_version_symlinks(version_tag) is True

    linked_file = version_dir / "models" / "checkpoints" / "model.safetensors"
    assert linked_file.exists()
    assert linked_file.is_symlink()

    user_link = version_dir / "user"
    assert user_link.exists()
    assert user_link.is_symlink()
