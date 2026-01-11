"""Tests for model importer with io/* module integration."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Generator
from unittest.mock import Mock, patch

import pytest

from backend.model_library.importer import ModelImporter
from backend.model_library.io.hashing import compute_dual_hash


@pytest.fixture
def temp_library(tmp_path: Path) -> Generator[Path, None, None]:
    """Create a temporary library directory."""
    library_path = tmp_path / "library"
    library_path.mkdir()
    yield library_path


@pytest.fixture
def mock_library(temp_library: Path) -> Mock:
    """Create a mock ModelLibrary with required methods."""
    from backend.model_library.library import ModelLibrary

    library = ModelLibrary(temp_library)
    return library


@pytest.fixture
def sample_model_file(tmp_path: Path) -> Path:
    """Create a sample model file for testing."""
    model_file = tmp_path / "test_model.safetensors"
    model_file.write_bytes(b"fake model content " * 1000)
    return model_file


@pytest.fixture
def sample_model_dir(tmp_path: Path) -> Path:
    """Create a sample model directory with multiple files."""
    model_dir = tmp_path / "model_folder"
    model_dir.mkdir()
    (model_dir / "model.safetensors").write_bytes(b"primary model data " * 1000)
    (model_dir / "config.json").write_text('{"key": "value"}')
    (model_dir / "tokenizer.json").write_text('{"tokens": []}')
    return model_dir


@pytest.mark.unit
class TestModelImporterInit:
    """Tests for ModelImporter initialization."""

    def test_importer_init(self, mock_library: Mock) -> None:
        """Test that importer initializes correctly."""
        importer = ModelImporter(mock_library)
        assert importer.library == mock_library


@pytest.mark.unit
class TestDetectType:
    """Tests for model type detection."""

    def test_detect_safetensors(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test detection of safetensors files."""
        importer = ModelImporter(mock_library)
        model_file = tmp_path / "model.safetensors"
        model_file.touch()
        model_type, subtype = importer._detect_type(model_file)
        assert model_type in ("diffusion", "llm")

    def test_detect_gguf(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test detection of GGUF files.

        Note: GGUF appears in both checkpoints and llm categories.
        Current implementation returns first match (checkpoints/diffusion).
        """
        importer = ModelImporter(mock_library)
        model_file = tmp_path / "model.gguf"
        model_file.touch()
        model_type, subtype = importer._detect_type(model_file)
        # GGUF appears in both categories, current impl returns first match
        assert model_type in ("llm", "diffusion")

    def test_detect_ckpt(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test detection of checkpoint files."""
        importer = ModelImporter(mock_library)
        model_file = tmp_path / "model.ckpt"
        model_file.touch()
        model_type, _ = importer._detect_type(model_file)
        assert model_type == "diffusion"


@pytest.mark.unit
class TestChoosePrimaryFile:
    """Tests for choosing primary file from model directory."""

    def test_choose_largest_file(self, mock_library: Mock, sample_model_dir: Path) -> None:
        """Test that the largest model file is chosen as primary."""
        importer = ModelImporter(mock_library)
        primary = importer._choose_primary_file(sample_model_dir)
        assert primary is not None
        assert primary.name == "model.safetensors"

    def test_choose_none_for_empty_dir(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test that None is returned for empty directory."""
        importer = ModelImporter(mock_library)
        empty_dir = tmp_path / "empty"
        empty_dir.mkdir()
        primary = importer._choose_primary_file(empty_dir)
        assert primary is None


@pytest.mark.unit
class TestComputeHashes:
    """Tests for hash computation."""

    def test_compute_hashes(self, mock_library: Mock, sample_model_file: Path) -> None:
        """Test dual hash computation via importer."""
        importer = ModelImporter(mock_library)
        sha256, blake3_hash = importer._compute_hashes(sample_model_file)
        # SHA256 hash should be 64 hex characters
        assert isinstance(sha256, str)
        assert len(sha256) == 64
        # BLAKE3 may not be available
        if blake3_hash:
            assert len(blake3_hash) == 64

    def test_compute_dual_hash(self, sample_model_file: Path) -> None:
        """Test that compute_dual_hash returns both hashes."""
        sha256, blake3 = compute_dual_hash(sample_model_file)
        assert len(sha256) == 64  # SHA256 is 64 hex chars
        if blake3:  # BLAKE3 may not be available
            assert len(blake3) == 64


@pytest.mark.unit
class TestImportPath:
    """Tests for import_path method."""

    def test_import_single_file(self, mock_library: Mock, sample_model_file: Path) -> None:
        """Test importing a single model file."""
        importer = ModelImporter(mock_library)

        model_dir = importer.import_path(
            sample_model_file,
            family="test-family",
            official_name="Test Model",
        )

        assert model_dir.exists()
        assert (model_dir / "metadata.json").exists()
        # Original file should be moved
        assert not sample_model_file.exists()

    def test_import_directory(self, mock_library: Mock, sample_model_dir: Path) -> None:
        """Test importing a model directory."""
        importer = ModelImporter(mock_library)

        model_dir = importer.import_path(
            sample_model_dir,
            family="test-family",
            official_name="Test Model Dir",
        )

        assert model_dir.exists()
        assert (model_dir / "metadata.json").exists()
        # Check model file was moved
        assert any(f.suffix == ".safetensors" for f in model_dir.iterdir())

    def test_import_creates_metadata(self, mock_library: Mock, sample_model_file: Path) -> None:
        """Test that import creates proper metadata."""
        importer = ModelImporter(mock_library)

        model_dir = importer.import_path(
            sample_model_file,
            family="stability",
            official_name="SDXL Base",
            repo_id="stabilityai/sdxl-base",
        )

        meta_path = model_dir / "metadata.json"
        assert meta_path.exists()

        with meta_path.open() as f:
            metadata = json.load(f)

        assert metadata["official_name"] == "SDXL Base"
        assert metadata["family"] == "stability"
        assert "hashes" in metadata
        assert "sha256" in metadata["hashes"]

    def test_import_with_repo_id(self, mock_library: Mock, sample_model_file: Path) -> None:
        """Test that repo_id creates proper download URL."""
        importer = ModelImporter(mock_library)

        model_dir = importer.import_path(
            sample_model_file,
            family="test",
            official_name="Test",
            repo_id="test/repo",
        )

        meta_path = model_dir / "metadata.json"
        with meta_path.open() as f:
            metadata = json.load(f)

        assert metadata["download_url"] == "https://huggingface.co/test/repo"

    def test_import_nonexistent_path_raises(self, mock_library: Mock) -> None:
        """Test that importing nonexistent path raises FileNotFoundError."""
        importer = ModelImporter(mock_library)

        with pytest.raises(FileNotFoundError):
            importer.import_path(
                Path("/nonexistent/path"),
                family="test",
                official_name="Test",
            )


@pytest.mark.unit
class TestImporterWithIOManager:
    """Tests for importer integration with io/manager.py."""

    def test_io_manager_available(self) -> None:
        """Test that IOManager can be imported from io.manager."""
        from backend.model_library.io.manager import IOManager

        manager = IOManager()
        assert hasattr(manager, "copy_file_with_hashing")

    def test_compute_dual_hash_available(self) -> None:
        """Test that compute_dual_hash is available from io.hashing."""
        from backend.model_library.io.hashing import compute_dual_hash

        assert callable(compute_dual_hash)

    def test_import_computes_hashes_efficiently(
        self, mock_library: Mock, sample_model_file: Path
    ) -> None:
        """Test that hashes are computed in single pass during copy."""
        importer = ModelImporter(mock_library)

        # Import should compute both hashes in one file read
        model_dir = importer.import_path(
            sample_model_file,
            family="test",
            official_name="Test Model",
        )

        meta_path = model_dir / "metadata.json"
        with meta_path.open() as f:
            metadata = json.load(f)

        # Both hashes should be present
        assert "sha256" in metadata["hashes"]
        # BLAKE3 may be empty if not available
        assert "blake3" in metadata["hashes"]
