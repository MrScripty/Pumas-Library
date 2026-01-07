"""Tests for the model library mapper."""

from pathlib import Path
from typing import Optional

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
