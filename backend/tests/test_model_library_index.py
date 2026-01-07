"""Tests for the model library SQLite index."""

from pathlib import Path

import pytest

from backend.model_library.index import ModelIndex
from backend.models import ModelMetadata


def _sample_metadata() -> ModelMetadata:
    return {
        "model_id": "model-a",
        "family": "family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Model A",
        "cleaned_name": "model-a",
        "tags": ["tag"],
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


def test_index_upsert_and_get(tmp_path: Path):
    db_path = tmp_path / "models.db"
    index = ModelIndex(db_path)

    metadata = _sample_metadata()
    index.upsert("diffusion/family/model-a", "diffusion/family/model-a", metadata)

    result = index.get_metadata("diffusion/family/model-a")
    assert result is not None
    assert result["official_name"] == "Model A"


def test_index_list_metadata_skips_bad_json(tmp_path: Path):
    db_path = tmp_path / "models.db"
    index = ModelIndex(db_path)

    metadata = _sample_metadata()
    index.upsert("diffusion/family/model-a", "diffusion/family/model-a", metadata)

    with index._connect() as conn:
        conn.execute(
            "INSERT INTO models (id, path, cleaned_name, official_name, model_type, tags_json, hashes_json, metadata_json, updated_at) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                "bad",
                "bad",
                "bad",
                "bad",
                "bad",
                "[]",
                "{}",
                "{not-json}",
                "2024-01-01T00:00:00Z",
            ),
        )
        conn.commit()

    results = index.list_metadata()
    assert len(results) == 1


def test_index_clear(tmp_path: Path):
    db_path = tmp_path / "models.db"
    index = ModelIndex(db_path)

    metadata = _sample_metadata()
    index.upsert("diffusion/family/model-a", "diffusion/family/model-a", metadata)
    index.clear()

    assert index.list_metadata() == []
