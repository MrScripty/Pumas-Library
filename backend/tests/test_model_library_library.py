"""Tests for the model library manager."""

from pathlib import Path

from backend.model_library.library import ModelLibrary
from backend.models import ModelMetadata, ModelOverrides


def _sample_metadata() -> ModelMetadata:
    return {
        "model_id": "model-a",
        "family": "family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Model A",
        "cleaned_name": "model-a",
        "tags": [],
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
        "size_bytes": 10,
        "files": [],
    }


def test_save_and_load_metadata(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    model_dir = library.build_model_path("diffusion", "family", "model-a")
    model_dir.mkdir(parents=True, exist_ok=True)

    metadata = _sample_metadata()
    library.save_metadata(model_dir, metadata)

    loaded = library.load_metadata(model_dir)
    assert loaded is not None
    assert loaded["official_name"] == "Model A"


def test_save_and_load_overrides(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    model_dir = library.build_model_path("diffusion", "family", "model-a")
    model_dir.mkdir(parents=True, exist_ok=True)

    overrides: ModelOverrides = {"version_ranges": {"comfyui": ">=0.1.0"}}
    library.save_overrides(model_dir, overrides)

    loaded = library.load_overrides(model_dir)
    assert loaded["version_ranges"]["comfyui"] == ">=0.1.0"


def test_rebuild_index_and_list_models(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    model_dir = library.build_model_path("diffusion", "family", "model-a")
    model_dir.mkdir(parents=True, exist_ok=True)

    metadata = _sample_metadata()
    library.save_metadata(model_dir, metadata)
    library.index_model_dir(model_dir, metadata)

    library.rebuild_index()
    results = library.list_models()

    assert len(results) == 1
    assert results[0]["official_name"] == "Model A"


def test_get_model_by_path(tmp_path: Path):
    library = ModelLibrary(tmp_path / "models")
    model_dir = library.build_model_path("diffusion", "family", "model-a")
    model_dir.mkdir(parents=True, exist_ok=True)

    metadata = _sample_metadata()
    library.save_metadata(model_dir, metadata)
    library.index_model_dir(model_dir, metadata)

    rel_path = str(model_dir.relative_to(library.library_root))
    result = library.get_model(rel_path)
    assert result is not None
    assert result["official_name"] == "Model A"
