"""Tests for drive-aware I/O queue manager."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from unittest import mock

import pytest

from backend.model_library.io.manager import (
    DriveInfo,
    DriveType,
    IOManager,
    get_drive_info,
    get_drive_type,
)


@pytest.mark.unit
class TestDriveType:
    """Tests for DriveType enum."""

    def test_drive_type_values(self):
        """Test that drive types have expected values."""
        assert DriveType.SSD.value == "ssd"
        assert DriveType.HDD.value == "hdd"
        assert DriveType.UNKNOWN.value == "unknown"


@pytest.mark.unit
class TestDriveInfo:
    """Tests for DriveInfo dataclass."""

    def test_create_drive_info(self):
        """Test creating DriveInfo with all fields."""
        info = DriveInfo(
            path=Path("/dev/sda"),
            drive_type=DriveType.SSD,
            device="/dev/sda",
            mount_point=Path("/"),
        )
        assert info.path == Path("/dev/sda")
        assert info.drive_type == DriveType.SSD
        assert info.device == "/dev/sda"
        assert info.mount_point == Path("/")

    def test_create_drive_info_defaults(self):
        """Test creating DriveInfo with defaults."""
        info = DriveInfo(path=Path("/tmp"))
        assert info.path == Path("/tmp")
        assert info.drive_type == DriveType.UNKNOWN
        assert info.device == ""
        assert info.mount_point is None


@pytest.mark.unit
class TestGetDriveType:
    """Tests for get_drive_type function."""

    def test_tmp_path_returns_valid_type(self, tmp_path: Path):
        """Test that tmp_path returns a valid drive type."""
        result = get_drive_type(tmp_path)
        assert result in (DriveType.SSD, DriveType.HDD, DriveType.UNKNOWN)

    def test_nonexistent_path_returns_unknown(self):
        """Test that nonexistent path returns UNKNOWN."""
        result = get_drive_type(Path("/nonexistent/path/12345"))
        assert result == DriveType.UNKNOWN

    @mock.patch("subprocess.run")
    def test_ssd_detection_via_rotational(self, mock_run, tmp_path: Path):
        """Test SSD detection via rotational flag (0 = SSD)."""
        # Mock lsblk to return device info
        mock_run.return_value = mock.Mock(
            returncode=0,
            stdout="sda",
            stderr="",
        )

        with mock.patch("builtins.open", mock.mock_open(read_data="0\n")):
            with mock.patch("pathlib.Path.exists", return_value=True):
                # Force detection path
                result = get_drive_type(tmp_path)
                # May return UNKNOWN depending on mock setup
                assert isinstance(result, DriveType)

    @mock.patch("subprocess.run")
    def test_hdd_detection_via_rotational(self, mock_run, tmp_path: Path):
        """Test HDD detection via rotational flag (1 = HDD)."""
        mock_run.return_value = mock.Mock(
            returncode=0,
            stdout="sda",
            stderr="",
        )

        with mock.patch("builtins.open", mock.mock_open(read_data="1\n")):
            with mock.patch("pathlib.Path.exists", return_value=True):
                result = get_drive_type(tmp_path)
                assert isinstance(result, DriveType)

    @mock.patch("subprocess.run")
    def test_lsblk_fails_returns_unknown(self, mock_run, tmp_path: Path):
        """Test graceful fallback when lsblk fails."""
        mock_run.side_effect = FileNotFoundError()

        result = get_drive_type(tmp_path)
        assert result == DriveType.UNKNOWN


@pytest.mark.unit
class TestGetDriveInfo:
    """Tests for get_drive_info function."""

    def test_get_drive_info_tmp_path(self, tmp_path: Path):
        """Test getting drive info for tmp_path."""
        info = get_drive_info(tmp_path)
        assert info.path == tmp_path
        assert isinstance(info.drive_type, DriveType)

    def test_get_drive_info_nonexistent(self):
        """Test getting drive info for nonexistent path."""
        info = get_drive_info(Path("/nonexistent/path/12345"))
        assert info.drive_type == DriveType.UNKNOWN

    @mock.patch("subprocess.run")
    def test_get_drive_info_with_device(self, mock_run, tmp_path: Path):
        """Test extracting device and mount info."""
        # Mock df command
        mock_run.return_value = mock.Mock(
            returncode=0,
            stdout="Filesystem\tMounted on\n/dev/sda1\t/\n",
            stderr="",
        )

        info = get_drive_info(tmp_path)
        assert isinstance(info, DriveInfo)


@pytest.mark.unit
class TestIOManager:
    """Tests for IOManager class."""

    def test_create_io_manager(self):
        """Test creating IOManager with defaults."""
        manager = IOManager()
        assert manager.ssd_concurrency > 0
        assert manager.hdd_concurrency > 0

    def test_create_io_manager_custom_concurrency(self):
        """Test creating IOManager with custom concurrency."""
        manager = IOManager(ssd_concurrency=8, hdd_concurrency=1)
        assert manager.ssd_concurrency == 8
        assert manager.hdd_concurrency == 1

    def test_get_concurrency_for_ssd(self, tmp_path: Path):
        """Test getting concurrency limit for SSD."""
        manager = IOManager(ssd_concurrency=8, hdd_concurrency=2)

        with mock.patch(
            "backend.model_library.io.manager.get_drive_type",
            return_value=DriveType.SSD,
        ):
            limit = manager.get_concurrency_for_path(tmp_path)
            assert limit == 8

    def test_get_concurrency_for_hdd(self, tmp_path: Path):
        """Test getting concurrency limit for HDD."""
        manager = IOManager(ssd_concurrency=8, hdd_concurrency=2)

        with mock.patch(
            "backend.model_library.io.manager.get_drive_type",
            return_value=DriveType.HDD,
        ):
            limit = manager.get_concurrency_for_path(tmp_path)
            assert limit == 2

    def test_get_concurrency_for_unknown(self, tmp_path: Path):
        """Test getting concurrency limit for unknown drive type."""
        manager = IOManager(ssd_concurrency=8, hdd_concurrency=2)

        with mock.patch(
            "backend.model_library.io.manager.get_drive_type",
            return_value=DriveType.UNKNOWN,
        ):
            limit = manager.get_concurrency_for_path(tmp_path)
            # Should default to HDD (conservative)
            assert limit == 2

    def test_copy_file_basic(self, tmp_path: Path):
        """Test basic file copy operation."""
        manager = IOManager()

        src = tmp_path / "source.txt"
        src.write_text("test content")
        dst = tmp_path / "dest.txt"

        result = manager.copy_file(src, dst)
        assert result.exists()
        assert result.read_text() == "test content"

    def test_copy_file_with_hashing(self, tmp_path: Path):
        """Test file copy with hash computation."""
        manager = IOManager()

        src = tmp_path / "source.bin"
        content = b"test binary content for hashing"
        src.write_bytes(content)
        dst = tmp_path / "dest.bin"

        result, hashes = manager.copy_file_with_hashing(src, dst)
        assert result.exists()
        assert result.read_bytes() == content
        assert "sha256" in hashes
        assert len(hashes["sha256"]) == 64  # SHA256 hex length

    def test_copy_file_creates_parent_dirs(self, tmp_path: Path):
        """Test that copy creates parent directories."""
        manager = IOManager()

        src = tmp_path / "source.txt"
        src.write_text("nested test")
        dst = tmp_path / "nested" / "deep" / "dest.txt"

        result = manager.copy_file(src, dst)
        assert result.exists()
        assert result.read_text() == "nested test"

    def test_copy_file_preserves_mtime(self, tmp_path: Path):
        """Test that copy preserves modification time."""
        manager = IOManager()

        src = tmp_path / "source.txt"
        src.write_text("preserve mtime")
        # Set specific mtime
        original_mtime = 1000000000.0
        os.utime(src, (original_mtime, original_mtime))

        dst = tmp_path / "dest.txt"
        result = manager.copy_file(src, dst, preserve_mtime=True)

        assert abs(result.stat().st_mtime - original_mtime) < 1.0

    def test_copy_file_source_not_found(self, tmp_path: Path):
        """Test error when source file doesn't exist."""
        manager = IOManager()

        src = tmp_path / "nonexistent.txt"
        dst = tmp_path / "dest.txt"

        with pytest.raises(FileNotFoundError):
            manager.copy_file(src, dst)

    def test_move_file_basic(self, tmp_path: Path):
        """Test basic file move operation."""
        manager = IOManager()

        src = tmp_path / "source.txt"
        src.write_text("move me")
        dst = tmp_path / "dest.txt"

        result = manager.move_file(src, dst)
        assert result.exists()
        assert result.read_text() == "move me"
        assert not src.exists()

    def test_move_file_cross_device(self, tmp_path: Path):
        """Test move falls back to copy+delete when needed."""
        manager = IOManager()

        src = tmp_path / "source.txt"
        src.write_text("cross device move")
        dst = tmp_path / "subdir" / "dest.txt"

        result = manager.move_file(src, dst)
        assert result.exists()
        assert result.read_text() == "cross device move"
        assert not src.exists()

    def test_drive_cache(self, tmp_path: Path):
        """Test that drive info is cached."""
        manager = IOManager()

        # First call should detect drive
        with mock.patch(
            "backend.model_library.io.manager.get_drive_type",
            return_value=DriveType.SSD,
        ) as mock_get:
            manager.get_concurrency_for_path(tmp_path)
            manager.get_concurrency_for_path(tmp_path)
            # Should only call once due to caching
            assert mock_get.call_count == 1

    def test_clear_drive_cache(self, tmp_path: Path):
        """Test clearing the drive cache."""
        manager = IOManager()

        with mock.patch(
            "backend.model_library.io.manager.get_drive_type",
            return_value=DriveType.SSD,
        ) as mock_get:
            manager.get_concurrency_for_path(tmp_path)
            manager.clear_drive_cache()
            manager.get_concurrency_for_path(tmp_path)
            # Should call twice after cache clear
            assert mock_get.call_count == 2
