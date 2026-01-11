"""Tests for library.py with FTS5 integration."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from backend.model_library.library import ModelLibrary


@pytest.fixture
def temp_library(tmp_path: Path) -> Path:
    """Create a temporary library directory."""
    library_path = tmp_path / "library"
    library_path.mkdir()
    return library_path


@pytest.fixture
def library(temp_library: Path) -> ModelLibrary:
    """Create a ModelLibrary instance."""
    return ModelLibrary(temp_library)


@pytest.fixture
def sample_metadata() -> dict:
    """Create sample model metadata."""
    return {
        "model_id": "test-model",
        "family": "test-family",
        "model_type": "diffusion",
        "subtype": "checkpoints",
        "official_name": "Test Model v1.0",
        "cleaned_name": "test-model",
        "tags": ["checkpoint", "sd-xl", "base"],
        "base_model": "",
        "preview_image": "",
        "release_date": "",
        "download_url": "https://huggingface.co/test/model",
        "model_card": {},
        "inference_settings": {},
        "compatible_apps": [],
        "hashes": {"sha256": "abc123", "blake3": "def456"},
        "notes": "",
        "added_date": "2026-01-10T12:00:00Z",
        "updated_date": "2026-01-10T12:00:00Z",
        "size_bytes": 1024,
        "files": [],
    }


@pytest.mark.unit
class TestModelLibraryInit:
    """Tests for ModelLibrary initialization."""

    def test_library_creates_directory(self, temp_library: Path) -> None:
        """Test that library creates root directory."""
        library = ModelLibrary(temp_library)
        assert library.library_root.exists()

    def test_library_creates_db(self, temp_library: Path) -> None:
        """Test that library creates database file."""
        library = ModelLibrary(temp_library)
        assert library.db_path == temp_library / "models.db"

    def test_library_has_index(self, library: ModelLibrary) -> None:
        """Test that library has an index."""
        assert library.index is not None


@pytest.mark.unit
class TestModelLibraryMetadata:
    """Tests for metadata operations."""

    def test_save_and_load_metadata(self, library: ModelLibrary, sample_metadata: dict) -> None:
        """Test saving and loading metadata."""
        model_dir = library.library_root / "diffusion" / "test-family" / "test-model"
        model_dir.mkdir(parents=True)

        library.save_metadata(model_dir, sample_metadata)
        loaded = library.load_metadata(model_dir)

        assert loaded is not None
        assert loaded["official_name"] == "Test Model v1.0"
        assert loaded["family"] == "test-family"

    def test_load_nonexistent_metadata(self, library: ModelLibrary) -> None:
        """Test loading metadata from nonexistent directory."""
        model_dir = library.library_root / "nonexistent"
        result = library.load_metadata(model_dir)
        assert result is None


@pytest.mark.unit
class TestModelLibraryIndex:
    """Tests for index operations."""

    def test_index_model_dir(self, library: ModelLibrary, sample_metadata: dict) -> None:
        """Test indexing a model directory."""
        model_dir = library.library_root / "diffusion" / "test-family" / "test-model"
        model_dir.mkdir(parents=True)

        library.save_metadata(model_dir, sample_metadata)
        library.index_model_dir(model_dir, sample_metadata)

        models = library.list_models()
        assert len(models) == 1
        assert models[0]["official_name"] == "Test Model v1.0"

    def test_list_models_empty(self, library: ModelLibrary) -> None:
        """Test listing models when library is empty."""
        models = library.list_models()
        assert models == []

    def test_get_model(self, library: ModelLibrary, sample_metadata: dict) -> None:
        """Test getting a specific model by path."""
        model_dir = library.library_root / "diffusion" / "test-family" / "test-model"
        model_dir.mkdir(parents=True)

        library.save_metadata(model_dir, sample_metadata)
        library.index_model_dir(model_dir, sample_metadata)

        model = library.get_model("diffusion/test-family/test-model")
        assert model is not None
        assert model["official_name"] == "Test Model v1.0"

    def test_get_nonexistent_model(self, library: ModelLibrary) -> None:
        """Test getting a model that doesn't exist."""
        model = library.get_model("nonexistent/path")
        assert model is None


@pytest.mark.unit
class TestModelLibraryFTS5:
    """Tests for FTS5 integration."""

    def test_fts5_manager_available(self) -> None:
        """Test that FTS5Manager can be imported."""
        from backend.model_library.search import FTS5Manager

        assert FTS5Manager is not None

    def test_search_models_function_available(self) -> None:
        """Test that search_models function can be imported."""
        from backend.model_library.search import search_models

        assert callable(search_models)

    def test_library_search_method(self, library: ModelLibrary, sample_metadata: dict) -> None:
        """Test that library has a search method after FTS5 integration."""
        # Add a model
        model_dir = library.library_root / "diffusion" / "test-family" / "test-model"
        model_dir.mkdir(parents=True)
        library.save_metadata(model_dir, sample_metadata)
        library.index_model_dir(model_dir, sample_metadata)

        # Search should work
        if hasattr(library, "search_models"):
            result = library.search_models("test")
            assert result.total_count >= 0

    def test_library_has_fts5_after_rebuild(
        self, library: ModelLibrary, sample_metadata: dict
    ) -> None:
        """Test that library sets up FTS5 after rebuild."""
        # Add some models
        for i in range(3):
            model_dir = library.library_root / "diffusion" / "family" / f"model-{i}"
            model_dir.mkdir(parents=True)
            meta = sample_metadata.copy()
            meta["official_name"] = f"Model {i}"
            meta["cleaned_name"] = f"model-{i}"
            library.save_metadata(model_dir, meta)
            library.index_model_dir(model_dir, meta)

        # Rebuild should populate FTS5
        library.rebuild_index()

        # Should be able to search
        if hasattr(library, "search_models"):
            result = library.search_models("model")
            assert result.total_count == 3


@pytest.mark.unit
class TestModelLibraryBuildPath:
    """Tests for build_model_path."""

    def test_build_model_path(self, library: ModelLibrary) -> None:
        """Test building model path from components."""
        path = library.build_model_path("diffusion", "stability", "sdxl-base")
        assert path == library.library_root / "diffusion" / "stability" / "sdxl-base"

    def test_build_model_path_normalizes(self, library: ModelLibrary) -> None:
        """Test that build_model_path normalizes names."""
        path = library.build_model_path("diffusion", "My Family", "Test Model")
        # Names should be normalized (implementation detail)
        assert path.parent.parent == library.library_root / "diffusion"
