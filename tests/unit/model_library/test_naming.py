"""Tests for naming.py with NTFS sanitization."""

from __future__ import annotations

from pathlib import Path

import pytest

from backend.model_library.naming import normalize_filename, normalize_name, unique_path


@pytest.mark.unit
class TestNormalizeName:
    """Tests for normalize_name function."""

    def test_simple_name(self) -> None:
        """Test normalizing a simple name."""
        result = normalize_name("MyModel")
        assert result == "MyModel"

    def test_removes_spaces(self) -> None:
        """Test that spaces are removed."""
        result = normalize_name("My Model Name")
        assert " " not in result

    def test_removes_special_chars(self) -> None:
        """Test that special characters are removed."""
        result = normalize_name("model@v1.0!")
        # Should only contain alphanumeric, underscore, hyphen
        assert all(c.isalnum() or c in "-_" for c in result)

    def test_preserves_allowed_chars(self) -> None:
        """Test that allowed characters are preserved."""
        result = normalize_name("model-name_v1")
        assert result == "model-name_v1"

    def test_max_length(self) -> None:
        """Test that result is truncated to max length."""
        long_name = "a" * 200
        result = normalize_name(long_name, max_length=50)
        assert len(result) <= 50

    def test_fallback_for_empty(self) -> None:
        """Test fallback when all chars removed."""
        result = normalize_name("@#$%")
        assert result == "model"  # Default fallback

    def test_custom_fallback(self) -> None:
        """Test custom fallback value."""
        result = normalize_name("@#$%", fallback="unknown")
        assert result == "unknown"


@pytest.mark.unit
class TestNormalizeFilename:
    """Tests for normalize_filename function."""

    def test_simple_filename(self) -> None:
        """Test normalizing a simple filename."""
        result = normalize_filename("model.safetensors")
        assert result.endswith(".safetensors")

    def test_preserves_extension(self) -> None:
        """Test that extension is preserved."""
        result = normalize_filename("my model.gguf")
        assert result.endswith(".gguf")

    def test_removes_spaces_in_stem(self) -> None:
        """Test that spaces in stem are removed."""
        result = normalize_filename("my model file.txt")
        assert " " not in result
        assert result.endswith(".txt")

    def test_handles_multiple_extensions(self) -> None:
        """Test handling of multiple dots in filename."""
        result = normalize_filename("model.v1.0.safetensors")
        # Should preserve last extension
        assert result.endswith(".safetensors")

    def test_max_length_with_extension(self) -> None:
        """Test max length is respected with extension."""
        long_name = "a" * 200 + ".safetensors"
        result = normalize_filename(long_name, max_length=50)
        assert len(result) <= 50
        assert result.endswith(".safetensors")


@pytest.mark.unit
class TestUniquePath:
    """Tests for unique_path function."""

    def test_returns_same_if_not_exists(self, tmp_path: Path) -> None:
        """Test returns same path if it doesn't exist."""
        path = tmp_path / "newfile.txt"
        result = unique_path(path)
        assert result == path

    def test_adds_suffix_if_exists(self, tmp_path: Path) -> None:
        """Test adds numeric suffix if path exists."""
        path = tmp_path / "existing.txt"
        path.touch()

        result = unique_path(path)
        assert result != path
        assert result.stem.endswith("-2")
        assert result.suffix == ".txt"

    def test_increments_suffix(self, tmp_path: Path) -> None:
        """Test increments suffix for multiple conflicts."""
        base_path = tmp_path / "file.txt"
        base_path.touch()
        (tmp_path / "file-2.txt").touch()
        (tmp_path / "file-3.txt").touch()

        result = unique_path(base_path)
        assert result.name == "file-4.txt"


@pytest.mark.unit
class TestNTFSSanitization:
    """Tests for NTFS-specific sanitization functions."""

    def test_ntfs_reserved_chars_removed(self) -> None:
        """Test that NTFS reserved characters are handled."""
        # These are reserved on NTFS: < > : " / \ | ? *
        from backend.model_library.naming import sanitize_for_ntfs

        result = sanitize_for_ntfs('file<>:"/\\|?*.txt')
        # None of the reserved chars should remain
        for char in '<>:"/\\|?*':
            assert char not in result

    def test_ntfs_reserved_names(self) -> None:
        """Test that NTFS reserved names are handled."""
        from backend.model_library.naming import sanitize_for_ntfs

        # Reserved device names: CON, PRN, AUX, NUL, COM1-9, LPT1-9
        for name in ["CON", "PRN", "AUX", "NUL", "COM1", "LPT1"]:
            result = sanitize_for_ntfs(name)
            # Should be modified to not conflict
            assert result.upper() != name

    def test_ntfs_trailing_dots_spaces(self) -> None:
        """Test that trailing dots and spaces are removed."""
        from backend.model_library.naming import sanitize_for_ntfs

        result = sanitize_for_ntfs("filename...")
        assert not result.endswith(".")
        result = sanitize_for_ntfs("filename   ")
        assert not result.endswith(" ")

    def test_is_ntfs_safe(self) -> None:
        """Test NTFS safety check function."""
        from backend.model_library.naming import is_ntfs_safe

        # Safe names
        assert is_ntfs_safe("normal_file.txt") is True
        assert is_ntfs_safe("model-v1.safetensors") is True

        # Unsafe names
        assert is_ntfs_safe("file:name.txt") is False
        assert is_ntfs_safe("COM1.txt") is False
        assert is_ntfs_safe("file*name.txt") is False


@pytest.mark.unit
class TestCrossplatformNaming:
    """Tests for cross-platform naming utilities."""

    def test_normalize_works_on_all_platforms(self) -> None:
        """Test that normalize_name produces platform-safe names."""
        # Input with various problematic chars
        result = normalize_name("My:Model<v1>")
        # Should be safe on all platforms
        assert all(c.isalnum() or c in "-_" for c in result)

    def test_normalize_filename_cross_platform(self) -> None:
        """Test filename normalization is cross-platform safe."""
        result = normalize_filename("my:file<name>.txt")
        # Should not contain any OS-specific problematic chars
        problematic = '<>:"/\\|?*'
        for char in problematic:
            assert char not in result
