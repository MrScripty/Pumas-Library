"""Tests for platform abstraction for link creation."""

from __future__ import annotations

import os
import sys
from pathlib import Path
from unittest import mock

import pytest

from backend.model_library.io.platform import (
    LinkResult,
    LinkStrategy,
    create_link,
    get_available_strategies,
    get_default_strategy,
    is_cross_filesystem,
    remove_link,
    verify_link,
)


@pytest.mark.unit
class TestLinkStrategy:
    """Tests for LinkStrategy enum."""

    def test_strategy_values(self):
        """Test that link strategies have expected values."""
        assert LinkStrategy.SYMLINK.value == "symlink"
        assert LinkStrategy.HARDLINK.value == "hardlink"
        assert LinkStrategy.COPY.value == "copy"
        assert LinkStrategy.REFLINK.value == "reflink"

    def test_strategy_from_string(self):
        """Test creating strategy from string."""
        assert LinkStrategy("symlink") == LinkStrategy.SYMLINK
        assert LinkStrategy("hardlink") == LinkStrategy.HARDLINK
        assert LinkStrategy("copy") == LinkStrategy.COPY


@pytest.mark.unit
class TestLinkResult:
    """Tests for LinkResult dataclass."""

    def test_create_success(self, tmp_path: Path):
        """Test creating successful result."""
        result = LinkResult(
            success=True,
            source=tmp_path / "src",
            target=tmp_path / "dst",
            strategy=LinkStrategy.SYMLINK,
        )
        assert result.success is True
        assert result.error is None

    def test_create_failure(self, tmp_path: Path):
        """Test creating failure result."""
        result = LinkResult(
            success=False,
            source=tmp_path / "src",
            target=tmp_path / "dst",
            strategy=LinkStrategy.SYMLINK,
            error="Permission denied",
        )
        assert result.success is False
        assert result.error == "Permission denied"


@pytest.mark.unit
class TestGetDefaultStrategy:
    """Tests for get_default_strategy function."""

    def test_linux_default_is_symlink(self):
        """Test that Linux defaults to symlink."""
        with mock.patch.object(sys, "platform", "linux"):
            strategy = get_default_strategy()
            assert strategy == LinkStrategy.SYMLINK

    def test_darwin_default_is_symlink(self):
        """Test that macOS defaults to symlink."""
        with mock.patch.object(sys, "platform", "darwin"):
            strategy = get_default_strategy()
            assert strategy == LinkStrategy.SYMLINK

    def test_windows_default_is_copy(self):
        """Test that Windows defaults to copy (symlinks require admin)."""
        with mock.patch.object(sys, "platform", "win32"):
            strategy = get_default_strategy()
            assert strategy == LinkStrategy.COPY


@pytest.mark.unit
class TestGetAvailableStrategies:
    """Tests for get_available_strategies function."""

    def test_linux_has_symlink(self):
        """Test that symlink is available on Linux."""
        with mock.patch.object(sys, "platform", "linux"):
            strategies = get_available_strategies()
            assert LinkStrategy.SYMLINK in strategies

    def test_all_platforms_have_copy(self):
        """Test that copy is always available."""
        for platform in ["linux", "darwin", "win32"]:
            with mock.patch.object(sys, "platform", platform):
                strategies = get_available_strategies()
                assert LinkStrategy.COPY in strategies


@pytest.mark.unit
class TestIsCrossFilesystem:
    """Tests for is_cross_filesystem function."""

    def test_same_directory(self, tmp_path: Path):
        """Test that paths in same directory are same filesystem."""
        src = tmp_path / "file1.txt"
        dst = tmp_path / "file2.txt"
        src.touch()
        dst.touch()

        assert is_cross_filesystem(src, dst) is False

    def test_nonexistent_paths(self, tmp_path: Path):
        """Test handling of nonexistent paths."""
        src = tmp_path / "nonexistent1.txt"
        dst = tmp_path / "nonexistent2.txt"

        # Should not raise, returns False as default
        result = is_cross_filesystem(src, dst)
        assert isinstance(result, bool)


@pytest.mark.unit
class TestCreateLink:
    """Tests for create_link function."""

    def test_create_symlink(self, tmp_path: Path):
        """Test creating a symlink."""
        src = tmp_path / "source.txt"
        src.write_text("test content")
        dst = tmp_path / "link.txt"

        result = create_link(src, dst, LinkStrategy.SYMLINK)
        assert result.success is True
        assert dst.is_symlink()
        assert dst.read_text() == "test content"

    def test_create_hardlink(self, tmp_path: Path):
        """Test creating a hardlink."""
        src = tmp_path / "source.txt"
        src.write_text("test content")
        dst = tmp_path / "link.txt"

        result = create_link(src, dst, LinkStrategy.HARDLINK)
        assert result.success is True
        assert dst.exists()
        assert not dst.is_symlink()
        assert dst.read_text() == "test content"
        # Verify it's a hardlink (same inode)
        assert src.stat().st_ino == dst.stat().st_ino

    def test_create_copy(self, tmp_path: Path):
        """Test creating a copy."""
        src = tmp_path / "source.txt"
        src.write_text("test content")
        dst = tmp_path / "copy.txt"

        result = create_link(src, dst, LinkStrategy.COPY)
        assert result.success is True
        assert dst.exists()
        assert not dst.is_symlink()
        assert dst.read_text() == "test content"
        # Verify it's not a hardlink
        assert src.stat().st_ino != dst.stat().st_ino

    def test_create_relative_symlink(self, tmp_path: Path):
        """Test creating a relative symlink."""
        subdir = tmp_path / "subdir"
        subdir.mkdir()
        src = tmp_path / "source.txt"
        src.write_text("relative content")
        dst = subdir / "link.txt"

        result = create_link(src, dst, LinkStrategy.SYMLINK, relative=True)
        assert result.success is True
        assert dst.is_symlink()
        # Check that the symlink target is relative
        link_target = os.readlink(dst)
        assert not os.path.isabs(link_target)

    def test_create_absolute_symlink(self, tmp_path: Path):
        """Test creating an absolute symlink."""
        src = tmp_path / "source.txt"
        src.write_text("absolute content")
        dst = tmp_path / "link.txt"

        result = create_link(src, dst, LinkStrategy.SYMLINK, relative=False)
        assert result.success is True
        assert dst.is_symlink()
        # Check that the symlink target is absolute
        link_target = os.readlink(dst)
        assert os.path.isabs(link_target)

    def test_create_link_creates_parent_dirs(self, tmp_path: Path):
        """Test that create_link creates parent directories."""
        src = tmp_path / "source.txt"
        src.write_text("nested content")
        dst = tmp_path / "nested" / "deep" / "link.txt"

        result = create_link(src, dst, LinkStrategy.SYMLINK)
        assert result.success is True
        assert dst.is_symlink()

    def test_create_link_source_not_found(self, tmp_path: Path):
        """Test error when source doesn't exist."""
        src = tmp_path / "nonexistent.txt"
        dst = tmp_path / "link.txt"

        result = create_link(src, dst, LinkStrategy.SYMLINK)
        assert result.success is False
        assert "not found" in result.error.lower() or "exist" in result.error.lower()

    def test_create_link_target_exists_no_overwrite(self, tmp_path: Path):
        """Test error when target exists and overwrite=False."""
        src = tmp_path / "source.txt"
        src.write_text("source")
        dst = tmp_path / "existing.txt"
        dst.write_text("existing")

        result = create_link(src, dst, LinkStrategy.SYMLINK, overwrite=False)
        assert result.success is False
        assert "exists" in result.error.lower()

    def test_create_link_target_exists_with_overwrite(self, tmp_path: Path):
        """Test replacing existing file when overwrite=True."""
        src = tmp_path / "source.txt"
        src.write_text("source")
        dst = tmp_path / "existing.txt"
        dst.write_text("existing")

        result = create_link(src, dst, LinkStrategy.SYMLINK, overwrite=True)
        assert result.success is True
        assert dst.is_symlink()
        assert dst.read_text() == "source"


@pytest.mark.unit
class TestVerifyLink:
    """Tests for verify_link function."""

    def test_verify_valid_symlink(self, tmp_path: Path):
        """Test verifying a valid symlink."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src)

        is_valid, error = verify_link(dst)
        assert is_valid is True
        assert error is None

    def test_verify_broken_symlink(self, tmp_path: Path):
        """Test verifying a broken symlink."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src)
        src.unlink()  # Break the link

        is_valid, error = verify_link(dst)
        assert is_valid is False
        assert error is not None
        assert "broken" in error.lower() or "target" in error.lower()

    def test_verify_hardlink(self, tmp_path: Path):
        """Test verifying a hardlink."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.hardlink_to(src)

        is_valid, error = verify_link(dst)
        assert is_valid is True
        assert error is None

    def test_verify_nonexistent(self, tmp_path: Path):
        """Test verifying nonexistent path."""
        dst = tmp_path / "nonexistent.txt"

        is_valid, error = verify_link(dst)
        assert is_valid is False
        assert "not found" in error.lower() or "exist" in error.lower()


@pytest.mark.unit
class TestRemoveLink:
    """Tests for remove_link function."""

    def test_remove_symlink(self, tmp_path: Path):
        """Test removing a symlink."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src)

        success = remove_link(dst)
        assert success is True
        assert not dst.exists()
        assert src.exists()  # Source should not be affected

    def test_remove_broken_symlink(self, tmp_path: Path):
        """Test removing a broken symlink."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src)
        src.unlink()  # Break the link

        success = remove_link(dst)
        assert success is True
        assert not dst.is_symlink()

    def test_remove_nonexistent(self, tmp_path: Path):
        """Test removing nonexistent path."""
        dst = tmp_path / "nonexistent.txt"

        success = remove_link(dst)
        # Should succeed (nothing to remove)
        assert success is True

    def test_remove_regular_file(self, tmp_path: Path):
        """Test that regular files are not removed by default."""
        dst = tmp_path / "regular.txt"
        dst.write_text("content")

        success = remove_link(dst, force=False)
        assert success is False
        assert dst.exists()

    def test_remove_regular_file_with_force(self, tmp_path: Path):
        """Test removing regular files with force=True."""
        dst = tmp_path / "regular.txt"
        dst.write_text("content")

        success = remove_link(dst, force=True)
        assert success is True
        assert not dst.exists()

    def test_remove_symlink_error(self, tmp_path: Path):
        """Test error handling when symlink removal fails."""
        src = tmp_path / "source.txt"
        src.write_text("content")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src)

        with mock.patch.object(Path, "unlink", side_effect=OSError("Permission denied")):
            success = remove_link(dst)
            assert success is False

    def test_remove_force_file_error(self, tmp_path: Path):
        """Test error handling when file removal fails with force."""
        dst = tmp_path / "regular.txt"
        dst.write_text("content")

        with mock.patch.object(Path, "unlink", side_effect=OSError("Permission denied")):
            success = remove_link(dst, force=True)
            assert success is False


@pytest.mark.unit
class TestEdgeCases:
    """Additional edge case tests for better coverage."""

    def test_cross_filesystem_source_not_exist_path_walk_up(self, tmp_path: Path):
        """Test walking up path when source doesn't exist."""
        src = tmp_path / "nested" / "deep" / "source.txt"
        dst = tmp_path / "dest.txt"
        dst.touch()

        result = is_cross_filesystem(src, dst)
        assert result is False

    def test_cross_filesystem_both_not_exist(self):
        """Test when both paths are deeply nonexistent."""
        src = Path("/nonexistent/path/a/b/c/source.txt")
        dst = Path("/nonexistent/path/x/y/z/dest.txt")

        result = is_cross_filesystem(src, dst)
        assert result is False

    def test_create_link_reflink_strategy(self, tmp_path: Path):
        """Test REFLINK strategy (falls back to copy)."""
        src = tmp_path / "source.txt"
        src.write_text("reflink test content")
        dst = tmp_path / "dest.txt"

        result = create_link(src, dst, LinkStrategy.REFLINK)
        assert result.success is True
        assert dst.exists()
        assert dst.read_text() == "reflink test content"

    def test_create_link_unknown_strategy_returns_error(self, tmp_path: Path):
        """Test that unknown strategy values return error."""
        src = tmp_path / "source.txt"
        src.write_text("source")
        dst = tmp_path / "dest.txt"

        # Create mock strategy with unknown value
        mock_strategy = mock.MagicMock()
        mock_strategy.value = "unknown_strategy"

        result = create_link(src, dst, mock_strategy)
        assert result.success is False
        assert "unknown" in result.error.lower()

    def test_create_link_symlink_to_failure(self, tmp_path: Path):
        """Test handling symlink_to failure."""
        src = tmp_path / "source.txt"
        src.write_text("source")
        dst = tmp_path / "link.txt"

        with mock.patch.object(Path, "symlink_to", side_effect=OSError("Cannot create symlink")):
            result = create_link(src, dst, LinkStrategy.SYMLINK)
            assert result.success is False

    def test_create_link_existing_symlink_overwrite(self, tmp_path: Path):
        """Test overwriting an existing symlink."""
        src1 = tmp_path / "source1.txt"
        src1.write_text("source1")
        src2 = tmp_path / "source2.txt"
        src2.write_text("source2")
        dst = tmp_path / "link.txt"
        dst.symlink_to(src1)

        result = create_link(src2, dst, LinkStrategy.SYMLINK, overwrite=True)
        assert result.success is True
        assert dst.read_text() == "source2"

    def test_create_link_parent_dir_creation_failure(self, tmp_path: Path):
        """Test handling mkdir failure."""
        src = tmp_path / "source.txt"
        src.write_text("source")
        dst = tmp_path / "nested" / "deep" / "link.txt"

        with mock.patch.object(Path, "mkdir", side_effect=OSError("Permission denied")):
            result = create_link(src, dst, LinkStrategy.SYMLINK)
            assert result.success is False
            assert "parent" in result.error.lower() or "directory" in result.error.lower()
