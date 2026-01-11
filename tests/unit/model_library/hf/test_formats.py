"""Tests for model format detection utilities."""

from __future__ import annotations

from unittest.mock import MagicMock

import pytest

from backend.model_library.hf.formats import (
    KNOWN_FORMATS,
    extract_formats,
    extract_formats_from_paths,
)


@pytest.mark.unit
class TestKnownFormats:
    """Tests for KNOWN_FORMATS constant."""

    def test_known_formats_is_frozenset(self):
        """Test that KNOWN_FORMATS is immutable."""
        assert isinstance(KNOWN_FORMATS, frozenset)

    def test_known_formats_contains_common_formats(self):
        """Test that common model formats are present."""
        common = ["safetensors", "gguf", "ckpt", "pt", "bin", "onnx"]
        for fmt in common:
            assert fmt in KNOWN_FORMATS

    def test_known_formats_contains_mobile_formats(self):
        """Test that mobile formats are present."""
        mobile = ["tflite", "mlmodel"]
        for fmt in mobile:
            assert fmt in KNOWN_FORMATS


@pytest.mark.unit
class TestExtractFormatsFromPaths:
    """Tests for extract_formats_from_paths function."""

    def test_extracts_from_file_extension(self):
        """Test extraction from file extension."""
        paths = ["model.safetensors"]
        result = extract_formats_from_paths(paths, [])
        assert "safetensors" in result

    def test_extracts_from_path_with_format_extension(self):
        """Test extraction when format appears as .ext in path."""
        paths = ["models/model.safetensors.backup", "model.bin"]
        result = extract_formats_from_paths(paths, [])
        assert "safetensors" in result
        assert "bin" in result

    def test_extracts_from_tags(self):
        """Test extraction from tags."""
        result = extract_formats_from_paths([], ["gguf", "pytorch"])
        assert "gguf" in result

    def test_extracts_multiple_formats(self):
        """Test extraction of multiple formats."""
        paths = ["model.safetensors", "model.gguf", "weights.bin"]
        result = extract_formats_from_paths(paths, [])
        assert "safetensors" in result
        assert "gguf" in result
        assert "bin" in result

    def test_case_insensitive(self):
        """Test case insensitive matching."""
        paths = ["MODEL.SAFETENSORS", "model.GGUF"]
        result = extract_formats_from_paths(paths, [])
        assert "safetensors" in result
        assert "gguf" in result

    def test_returns_sorted_list(self):
        """Test that results are sorted alphabetically."""
        paths = ["z.gguf", "a.safetensors", "m.bin"]
        result = extract_formats_from_paths(paths, [])
        assert result == sorted(result)

    def test_empty_inputs(self):
        """Test empty inputs."""
        assert extract_formats_from_paths([], []) == []

    def test_no_formats_found(self):
        """Test when no formats are found."""
        paths = ["readme.md", "config.json"]
        result = extract_formats_from_paths(paths, ["text-generation"])
        assert result == []

    def test_deduplicates_formats(self):
        """Test that duplicate formats are removed."""
        paths = ["model1.safetensors", "model2.safetensors"]
        result = extract_formats_from_paths(paths, ["safetensors"])
        assert result.count("safetensors") == 1


@pytest.mark.unit
class TestExtractFormats:
    """Tests for extract_formats function."""

    def test_extracts_from_siblings_rfilename(self):
        """Test extraction from sibling rfilename attributes."""
        sibling1 = MagicMock()
        sibling1.rfilename = "model.safetensors"
        sibling2 = MagicMock()
        sibling2.rfilename = "weights.bin"

        result = extract_formats([sibling1, sibling2], [])
        assert "safetensors" in result
        assert "bin" in result

    def test_handles_missing_rfilename(self):
        """Test handling of siblings without rfilename."""
        sibling = MagicMock(spec=[])  # No rfilename attribute
        result = extract_formats([sibling], ["gguf"])
        assert "gguf" in result

    def test_handles_none_rfilename(self):
        """Test handling of None rfilename."""
        sibling = MagicMock()
        sibling.rfilename = None

        result = extract_formats([sibling], ["safetensors"])
        assert "safetensors" in result

    def test_combines_siblings_and_tags(self):
        """Test combining formats from siblings and tags."""
        sibling = MagicMock()
        sibling.rfilename = "model.safetensors"

        result = extract_formats([sibling], ["gguf"])
        assert "safetensors" in result
        assert "gguf" in result

    def test_empty_siblings(self):
        """Test with empty siblings list."""
        result = extract_formats([], ["gguf"])
        assert "gguf" in result

    def test_empty_tags(self):
        """Test with empty tags list."""
        sibling = MagicMock()
        sibling.rfilename = "model.onnx"

        result = extract_formats([sibling], [])
        assert "onnx" in result
