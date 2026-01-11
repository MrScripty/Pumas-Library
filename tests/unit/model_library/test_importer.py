"""Tests for model importer with io/* module integration."""

from __future__ import annotations

import json
import struct
from pathlib import Path
from typing import Generator
from unittest.mock import Mock, patch

import pytest

from backend.model_library.importer import (
    ModelImporter,
    detect_sharded_sets,
    validate_file_type,
    validate_shard_completeness,
)
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
        model_type, subtype, detected_family = importer._detect_type(model_file)
        assert model_type in ("diffusion", "llm")

    def test_detect_gguf(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test detection of GGUF files.

        GGUF files are identified as LLM by default since GGUF was created
        specifically for llama.cpp (LLMs).
        """
        importer = ModelImporter(mock_library)
        model_file = tmp_path / "model.gguf"
        model_file.touch()
        model_type, subtype, detected_family = importer._detect_type(model_file)
        # GGUF defaults to LLM (extension-based fallback, content detection fails on empty file)
        assert model_type == "llm"

    def test_detect_ckpt(self, mock_library: Mock, tmp_path: Path) -> None:
        """Test detection of checkpoint files."""
        importer = ModelImporter(mock_library)
        model_file = tmp_path / "model.ckpt"
        model_file.touch()
        model_type, _, detected_family = importer._detect_type(model_file)
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


@pytest.mark.unit
class TestDetectShardedSets:
    """Tests for sharded model detection."""

    def test_detect_standard_shards(self, tmp_path: Path) -> None:
        """Test detection of standard HF shard pattern (model-00001-of-00005.ext)."""
        files = [
            tmp_path / "model-00001-of-00003.safetensors",
            tmp_path / "model-00002-of-00003.safetensors",
            tmp_path / "model-00003-of-00003.safetensors",
        ]
        for f in files:
            f.touch()

        groups = detect_sharded_sets(files)

        assert "model.safetensors" in groups
        assert len(groups["model.safetensors"]) == 3

    def test_detect_part_shards(self, tmp_path: Path) -> None:
        """Test detection of .part1, .part2 pattern."""
        files = [
            tmp_path / "model.gguf.part1",
            tmp_path / "model.gguf.part2",
            tmp_path / "model.gguf.part3",
        ]
        for f in files:
            f.touch()

        groups = detect_sharded_sets(files)

        assert "model.gguf" in groups
        assert len(groups["model.gguf"]) == 3

    def test_detect_underscore_shards(self, tmp_path: Path) -> None:
        """Test detection of model_00001.ext pattern."""
        files = [
            tmp_path / "llama_00001.safetensors",
            tmp_path / "llama_00002.safetensors",
        ]
        for f in files:
            f.touch()

        groups = detect_sharded_sets(files)

        assert "llama.safetensors" in groups
        assert len(groups["llama.safetensors"]) == 2

    def test_standalone_files_separate(self, tmp_path: Path) -> None:
        """Test that standalone files are kept separate."""
        files = [
            tmp_path / "model-00001-of-00002.safetensors",
            tmp_path / "model-00002-of-00002.safetensors",
            tmp_path / "other_model.safetensors",
        ]
        for f in files:
            f.touch()

        groups = detect_sharded_sets(files)

        assert "model.safetensors" in groups
        assert "other_model.safetensors" in groups
        assert len(groups["model.safetensors"]) == 2
        assert len(groups["other_model.safetensors"]) == 1

    def test_single_file_not_grouped(self, tmp_path: Path) -> None:
        """Test that single files matching pattern aren't incorrectly grouped."""
        files = [tmp_path / "model-00001-of-00002.safetensors"]
        files[0].touch()

        groups = detect_sharded_sets(files)

        # Single file should be treated as standalone
        assert len(groups) == 1
        # Key should be the file name itself, not the base pattern
        assert "model-00001-of-00002.safetensors" in groups


@pytest.mark.unit
class TestValidateShardCompleteness:
    """Tests for shard completeness validation."""

    def test_complete_set(self, tmp_path: Path) -> None:
        """Test validation of complete shard set."""
        files = [
            tmp_path / "model-00001-of-00003.safetensors",
            tmp_path / "model-00002-of-00003.safetensors",
            tmp_path / "model-00003-of-00003.safetensors",
        ]

        result = validate_shard_completeness(files)

        assert result["complete"] is True
        assert result["missing_shards"] == []
        assert result["total_expected"] == 3
        assert result["total_found"] == 3

    def test_incomplete_set(self, tmp_path: Path) -> None:
        """Test validation of incomplete shard set."""
        files = [
            tmp_path / "model-00001-of-00005.safetensors",
            tmp_path / "model-00003-of-00005.safetensors",
            tmp_path / "model-00005-of-00005.safetensors",
        ]

        result = validate_shard_completeness(files)

        assert result["complete"] is False
        assert 2 in result["missing_shards"]
        assert 4 in result["missing_shards"]
        assert result["total_expected"] == 5
        assert result["total_found"] == 3

    def test_empty_set(self) -> None:
        """Test validation of empty file list."""
        result = validate_shard_completeness([])

        assert result["complete"] is False
        assert result["total_found"] == 0


@pytest.mark.unit
class TestValidateFileType:
    """Tests for file type validation using magic bytes."""

    def test_valid_gguf(self, tmp_path: Path) -> None:
        """Test validation of GGUF file format."""
        model_file = tmp_path / "model.gguf"
        model_file.write_bytes(b"GGUF" + b"\x00" * 12)

        result = validate_file_type(model_file)

        assert result["valid"] is True
        assert result["detected_type"] == "gguf"

    def test_valid_ggml(self, tmp_path: Path) -> None:
        """Test validation of GGML file format."""
        model_file = tmp_path / "model.ggml"
        model_file.write_bytes(b"GGML" + b"\x00" * 12)

        result = validate_file_type(model_file)

        assert result["valid"] is True
        assert result["detected_type"] == "ggml"

    def test_valid_safetensors(self, tmp_path: Path) -> None:
        """Test validation of safetensors file format."""
        model_file = tmp_path / "model.safetensors"
        # Safetensors: 8-byte header length + JSON header starting with '{'
        header_json = b'{"test": "value"}'
        header_len = struct.pack("<Q", len(header_json))
        model_file.write_bytes(header_len + header_json + b"data")

        result = validate_file_type(model_file)

        assert result["valid"] is True
        assert result["detected_type"] == "safetensors"

    def test_valid_pickle(self, tmp_path: Path) -> None:
        """Test validation of pickle file format."""
        model_file = tmp_path / "model.pt"
        # Pickle protocol 4 marker
        model_file.write_bytes(b"\x80\x04" + b"\x00" * 14)

        result = validate_file_type(model_file)

        assert result["valid"] is True
        assert result["detected_type"] == "pickle"

    def test_valid_zip_pytorch(self, tmp_path: Path) -> None:
        """Test validation of ZIP-based PyTorch file format."""
        model_file = tmp_path / "model.pt"
        # ZIP magic number (PyTorch .pt files are often ZIP archives)
        model_file.write_bytes(b"PK\x03\x04" + b"\x00" * 12)

        result = validate_file_type(model_file)

        assert result["valid"] is True
        assert result["detected_type"] == "pickle"

    def test_invalid_text_file(self, tmp_path: Path) -> None:
        """Test rejection of text file masquerading as model."""
        model_file = tmp_path / "model.safetensors"
        model_file.write_text("This is not a model file!")

        result = validate_file_type(model_file)

        assert result["valid"] is False
        assert result["detected_type"] == "unknown"

    def test_invalid_html_file(self, tmp_path: Path) -> None:
        """Test rejection of HTML file masquerading as model."""
        model_file = tmp_path / "model.gguf"
        model_file.write_text("<!DOCTYPE html><html>Error page</html>")

        result = validate_file_type(model_file)

        assert result["valid"] is False
        assert result["detected_type"] == "unknown"

    def test_nonexistent_file(self, tmp_path: Path) -> None:
        """Test handling of nonexistent file."""
        model_file = tmp_path / "nonexistent.safetensors"

        result = validate_file_type(model_file)

        assert result["valid"] is False
        assert result["detected_type"] == "error"
