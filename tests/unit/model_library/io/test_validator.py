"""Tests for filesystem validation utilities."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from unittest import mock

import pytest

from backend.model_library.io.validator import (
    SandboxInfo,
    ValidationIssue,
    ValidationResult,
    ValidationSeverity,
    check_symlink_capability,
    detect_sandbox_environment,
    is_filesystem_writable,
    is_ntfs_dirty,
    is_path_on_readonly_mount,
    is_sandboxed,
    validate_import_source,
    validate_mapping_target,
    validate_symlink_support,
)


@pytest.mark.unit
class TestValidationSeverity:
    """Tests for ValidationSeverity enum."""

    def test_severity_values(self):
        """Test that severity levels have correct ordering."""
        assert ValidationSeverity.INFO.value < ValidationSeverity.WARNING.value
        assert ValidationSeverity.WARNING.value < ValidationSeverity.ERROR.value


@pytest.mark.unit
class TestValidationIssue:
    """Tests for ValidationIssue dataclass."""

    def test_create_info_issue(self):
        """Test creating an INFO severity issue."""
        issue = ValidationIssue(
            severity=ValidationSeverity.INFO, message="Test info", path=Path("/test")
        )
        assert issue.severity == ValidationSeverity.INFO
        assert issue.message == "Test info"
        assert issue.path == Path("/test")

    def test_create_warning_issue(self):
        """Test creating a WARNING severity issue."""
        issue = ValidationIssue(
            severity=ValidationSeverity.WARNING,
            message="Test warning",
            path=Path("/test"),
        )
        assert issue.severity == ValidationSeverity.WARNING

    def test_create_error_issue(self):
        """Test creating an ERROR severity issue."""
        issue = ValidationIssue(
            severity=ValidationSeverity.ERROR,
            message="Test error",
            path=Path("/test"),
        )
        assert issue.severity == ValidationSeverity.ERROR


@pytest.mark.unit
class TestValidationResult:
    """Tests for ValidationResult class."""

    def test_is_valid_with_no_issues(self):
        """Test that result with no issues is valid."""
        result = ValidationResult(path=Path("/test"), issues=[])
        assert result.is_valid() is True

    def test_is_valid_with_info_issue(self):
        """Test that result with INFO issues is still valid."""
        issue = ValidationIssue(
            severity=ValidationSeverity.INFO, message="Info", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.is_valid() is True

    def test_is_valid_with_warning_issue(self):
        """Test that result with WARNING issues is still valid."""
        issue = ValidationIssue(
            severity=ValidationSeverity.WARNING, message="Warning", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.is_valid() is True

    def test_is_valid_with_error_issue(self):
        """Test that result with ERROR issues is invalid."""
        issue = ValidationIssue(
            severity=ValidationSeverity.ERROR, message="Error", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.is_valid() is False

    def test_is_valid_with_multiple_issues(self):
        """Test validation with mixed severity issues."""
        issues = [
            ValidationIssue(severity=ValidationSeverity.INFO, message="Info", path=Path("/test")),
            ValidationIssue(
                severity=ValidationSeverity.WARNING,
                message="Warning",
                path=Path("/test"),
            ),
            ValidationIssue(severity=ValidationSeverity.ERROR, message="Error", path=Path("/test")),
        ]
        result = ValidationResult(path=Path("/test"), issues=issues)
        assert result.is_valid() is False

    def test_has_warnings(self):
        """Test detecting warnings in validation result."""
        issue = ValidationIssue(
            severity=ValidationSeverity.WARNING, message="Warning", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.has_warnings() is True

    def test_has_no_warnings(self):
        """Test result without warnings."""
        issue = ValidationIssue(
            severity=ValidationSeverity.INFO, message="Info", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.has_warnings() is False

    def test_has_errors(self):
        """Test detecting errors in validation result."""
        issue = ValidationIssue(
            severity=ValidationSeverity.ERROR, message="Error", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.has_errors() is True

    def test_has_no_errors(self):
        """Test result without errors."""
        issue = ValidationIssue(
            severity=ValidationSeverity.WARNING, message="Warning", path=Path("/test")
        )
        result = ValidationResult(path=Path("/test"), issues=[issue])
        assert result.has_errors() is False


@pytest.mark.unit
class TestIsNtfsDirty:
    """Tests for is_ntfs_dirty function."""

    def test_not_ntfs_filesystem(self, tmp_path: Path):
        """Test that non-NTFS filesystems return False."""
        # Most Linux test environments use ext4 or similar
        result = is_ntfs_dirty(tmp_path)
        assert result is False

    @mock.patch("subprocess.run")
    def test_ntfs_clean_filesystem(self, mock_run, tmp_path: Path):
        """Test NTFS filesystem without dirty bit."""
        mock_run.return_value = mock.Mock(returncode=0, stdout="State: OK", stderr="")

        with mock.patch(
            "backend.model_library.io.validator._get_filesystem_type",
            return_value="ntfs",
        ):
            result = is_ntfs_dirty(tmp_path)
            assert result is False

    @mock.patch("subprocess.run")
    def test_ntfs_dirty_filesystem(self, mock_run, tmp_path: Path):
        """Test NTFS filesystem with dirty bit set."""
        # Mock df command to return device
        df_result = mock.Mock(returncode=0, stdout="Filesystem\n/dev/sda1", stderr="")
        # Mock ntfsinfo command to show dirty state
        ntfsinfo_result = mock.Mock(
            returncode=0, stdout="State: Dirty\nVolume Flags: 0x01", stderr=""
        )
        mock_run.side_effect = [df_result, ntfsinfo_result]

        with mock.patch(
            "backend.model_library.io.validator._get_filesystem_type",
            return_value="ntfs",
        ):
            result = is_ntfs_dirty(tmp_path)
            assert result is True

    @mock.patch("subprocess.run")
    def test_ntfsinfo_command_fails(self, mock_run, tmp_path: Path):
        """Test graceful handling when ntfsinfo fails."""
        mock_run.side_effect = FileNotFoundError()

        with mock.patch(
            "backend.model_library.io.validator._get_filesystem_type",
            return_value="ntfs",
        ):
            result = is_ntfs_dirty(tmp_path)
            # Should return False when check fails (conservative)
            assert result is False


@pytest.mark.unit
class TestIsPathOnReadonlyMount:
    """Tests for is_path_on_readonly_mount function."""

    def test_writable_path(self, tmp_path: Path):
        """Test that writable tmp_path is not on readonly mount."""
        result = is_path_on_readonly_mount(tmp_path)
        assert result is False

    def test_nonexistent_path(self):
        """Test handling of nonexistent paths."""
        result = is_path_on_readonly_mount(Path("/nonexistent/path/12345"))
        # Should return False for nonexistent paths (conservative)
        assert result is False

    @mock.patch("os.statvfs")
    def test_readonly_mount(self, mock_statvfs, tmp_path: Path):
        """Test detection of readonly mount."""
        # Mock statvfs to return readonly flag (ST_RDONLY = 1)
        mock_statvfs.return_value = mock.Mock(f_flag=1)  # ST_RDONLY

        result = is_path_on_readonly_mount(tmp_path)
        assert result is True

    @mock.patch("os.statvfs")
    def test_writable_mount(self, mock_statvfs, tmp_path: Path):
        """Test detection of writable mount."""
        # Mock statvfs to return without readonly flag
        mock_statvfs.return_value = mock.Mock(f_flag=0)

        result = is_path_on_readonly_mount(tmp_path)
        assert result is False


@pytest.mark.unit
class TestIsFilesystemWritable:
    """Tests for is_filesystem_writable function."""

    def test_writable_directory(self, tmp_path: Path):
        """Test that writable directory is detected correctly."""
        result = is_filesystem_writable(tmp_path)
        assert result is True

    def test_writable_file_parent(self, tmp_path: Path):
        """Test checking writability of file's parent directory."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("test")

        result = is_filesystem_writable(test_file)
        assert result is True

    def test_nonexistent_path_parent_writable(self, tmp_path: Path):
        """Test checking writability for nonexistent path with writable parent."""
        nonexistent = tmp_path / "subdir" / "file.txt"

        result = is_filesystem_writable(nonexistent)
        # Parent (tmp_path) is writable
        assert result is True

    @mock.patch("os.access")
    def test_readonly_directory(self, mock_access, tmp_path: Path):
        """Test detection of readonly directory."""
        mock_access.return_value = False

        result = is_filesystem_writable(tmp_path)
        assert result is False

    def test_nonexistent_path_no_parent(self):
        """Test handling when path and all parents don't exist."""
        # Root should always exist
        result = is_filesystem_writable(Path("/"))
        # Root might not be writable, but function should not crash
        assert isinstance(result, bool)


@pytest.mark.unit
class TestValidateImportSource:
    """Tests for validate_import_source function."""

    def test_valid_file(self, tmp_path: Path):
        """Test validation of valid file."""
        test_file = tmp_path / "model.safetensors"
        test_file.write_bytes(b"test model data")

        result = validate_import_source(test_file)
        assert result.is_valid() is True
        assert result.path == test_file

    def test_valid_directory(self, tmp_path: Path):
        """Test validation of valid directory."""
        test_dir = tmp_path / "model_dir"
        test_dir.mkdir()
        (test_dir / "file.txt").write_text("test")

        result = validate_import_source(test_dir)
        assert result.is_valid() is True

    def test_nonexistent_path(self, tmp_path: Path):
        """Test that nonexistent path fails validation."""
        nonexistent = tmp_path / "nonexistent.safetensors"

        result = validate_import_source(nonexistent)
        assert result.is_valid() is False
        assert result.has_errors() is True

    def test_empty_directory(self, tmp_path: Path):
        """Test that empty directory generates warning."""
        empty_dir = tmp_path / "empty"
        empty_dir.mkdir()

        result = validate_import_source(empty_dir)
        # Should warn about empty directory but not fail
        assert result.has_warnings() is True

    @mock.patch("backend.model_library.io.validator.is_path_on_readonly_mount")
    def test_readonly_mount_warning(self, mock_readonly, tmp_path: Path):
        """Test warning for paths on readonly mounts."""
        test_file = tmp_path / "model.safetensors"
        test_file.write_bytes(b"test")

        mock_readonly.return_value = True

        result = validate_import_source(test_file)
        assert result.has_warnings() is True

    @mock.patch("backend.model_library.io.validator.is_ntfs_dirty")
    def test_ntfs_dirty_error(self, mock_dirty, tmp_path: Path):
        """Test error for NTFS dirty bit."""
        test_file = tmp_path / "model.safetensors"
        test_file.write_bytes(b"test")

        mock_dirty.return_value = True

        result = validate_import_source(test_file)
        assert result.is_valid() is False
        assert result.has_errors() is True


@pytest.mark.unit
class TestValidateMappingTarget:
    """Tests for validate_mapping_target function."""

    def test_valid_writable_directory(self, tmp_path: Path):
        """Test validation of valid writable target directory."""
        target_dir = tmp_path / "target"
        target_dir.mkdir()

        result = validate_mapping_target(target_dir)
        assert result.is_valid() is True

    def test_nonexistent_target_writable_parent(self, tmp_path: Path):
        """Test nonexistent target with writable parent."""
        target = tmp_path / "new_target"

        result = validate_mapping_target(target)
        # Parent is writable, so target can be created
        assert result.is_valid() is True

    @mock.patch("backend.model_library.io.validator.is_filesystem_writable")
    def test_readonly_target(self, mock_writable, tmp_path: Path):
        """Test error for readonly target."""
        mock_writable.return_value = False

        result = validate_mapping_target(tmp_path)
        assert result.is_valid() is False
        assert result.has_errors() is True

    def test_target_is_file(self, tmp_path: Path):
        """Test error when target is a file instead of directory."""
        target_file = tmp_path / "file.txt"
        target_file.write_text("test")

        result = validate_mapping_target(target_file)
        assert result.is_valid() is False
        assert result.has_errors() is True

    @mock.patch("backend.model_library.io.validator.is_ntfs_dirty")
    def test_ntfs_dirty_error(self, mock_dirty, tmp_path: Path):
        """Test error for NTFS dirty bit on target."""
        mock_dirty.return_value = True

        result = validate_mapping_target(tmp_path)
        assert result.is_valid() is False
        assert result.has_errors() is True

    @mock.patch("backend.model_library.io.validator.is_path_on_readonly_mount")
    def test_readonly_mount_error(self, mock_readonly, tmp_path: Path):
        """Test error for target on readonly mount."""
        mock_readonly.return_value = True

        result = validate_mapping_target(tmp_path)
        assert result.is_valid() is False
        assert result.has_errors() is True


@pytest.mark.unit
class TestSandboxDetection:
    """Tests for sandbox environment detection."""

    def test_sandbox_info_dataclass(self):
        """Test SandboxInfo dataclass creation."""
        info = SandboxInfo(
            is_sandboxed=True, sandbox_type="flatpak", limitations=["test limitation"]
        )
        assert info.is_sandboxed is True
        assert info.sandbox_type == "flatpak"
        assert len(info.limitations) == 1

    def test_detect_no_sandbox(self, monkeypatch):
        """Test detection when not in sandbox."""
        # Ensure no sandbox env vars are set
        monkeypatch.delenv("FLATPAK_ID", raising=False)
        monkeypatch.delenv("SNAP", raising=False)
        monkeypatch.delenv("APPIMAGE", raising=False)

        # Mock Path.exists to return False for sandbox indicators
        with mock.patch.object(Path, "exists", return_value=False):
            with mock.patch.object(Path, "read_text", return_value=""):
                result = detect_sandbox_environment()
                # May still be detected if in container
                assert isinstance(result.is_sandboxed, bool)
                assert isinstance(result.sandbox_type, str)

    def test_detect_flatpak_by_env(self, monkeypatch):
        """Test Flatpak detection via environment variable."""
        monkeypatch.setenv("FLATPAK_ID", "com.example.app")
        monkeypatch.delenv("SNAP", raising=False)
        monkeypatch.delenv("APPIMAGE", raising=False)

        with mock.patch.object(Path, "exists", return_value=False):
            result = detect_sandbox_environment()
            assert result.is_sandboxed is True
            assert result.sandbox_type == "flatpak"
            assert len(result.limitations) > 0

    def test_detect_snap(self, monkeypatch):
        """Test Snap detection via environment variable."""
        monkeypatch.delenv("FLATPAK_ID", raising=False)
        monkeypatch.setenv("SNAP", "/snap/example/123")
        monkeypatch.delenv("APPIMAGE", raising=False)

        with mock.patch.object(Path, "exists", return_value=False):
            result = detect_sandbox_environment()
            assert result.is_sandboxed is True
            assert result.sandbox_type == "snap"
            assert len(result.limitations) > 0

    def test_detect_appimage(self, monkeypatch):
        """Test AppImage detection via environment variable."""
        monkeypatch.delenv("FLATPAK_ID", raising=False)
        monkeypatch.delenv("SNAP", raising=False)
        monkeypatch.setenv("APPIMAGE", "/path/to/app.AppImage")

        with mock.patch.object(Path, "exists", return_value=False):
            result = detect_sandbox_environment()
            assert result.is_sandboxed is True
            assert result.sandbox_type == "appimage"

    def test_is_sandboxed_convenience(self, monkeypatch):
        """Test is_sandboxed convenience function."""
        monkeypatch.delenv("FLATPAK_ID", raising=False)
        monkeypatch.delenv("SNAP", raising=False)
        monkeypatch.delenv("APPIMAGE", raising=False)

        with mock.patch.object(Path, "exists", return_value=False):
            with mock.patch.object(Path, "read_text", return_value=""):
                result = is_sandboxed()
                assert isinstance(result, bool)


@pytest.mark.unit
class TestSymlinkCapability:
    """Tests for symlink capability testing."""

    def check_symlink_capability_success(self, tmp_path: Path):
        """Test symlink capability detection in writable directory."""
        result = check_symlink_capability(tmp_path)
        # Most test environments support symlinks
        assert result is True

    def check_symlink_capability_nonexistent_dir(self):
        """Test symlink capability returns False for nonexistent directory."""
        result = check_symlink_capability(Path("/nonexistent/path/12345"))
        assert result is False

    @mock.patch.object(Path, "symlink_to")
    def check_symlink_capability_failure(self, mock_symlink, tmp_path: Path):
        """Test symlink capability detection when symlinks fail."""
        mock_symlink.side_effect = OSError("Symlinks not supported")

        result = check_symlink_capability(tmp_path)
        assert result is False


@pytest.mark.unit
class TestValidateSymlinkSupport:
    """Tests for validate_symlink_support function."""

    def test_valid_symlink_support(self, tmp_path: Path):
        """Test validation in directory with symlink support."""
        result = validate_symlink_support(tmp_path)
        # Most test environments support symlinks
        assert result.is_valid() is True

    @mock.patch("backend.model_library.io.validator.check_symlink_capability")
    def test_nonexistent_directory_error(self, mock_capability, tmp_path: Path):
        """Test error for completely nonexistent directory tree."""
        # Create a path that doesn't exist and has no writable parent
        nonexistent = tmp_path / "subdir" / "fake" / "path"
        # Don't create the parent directories
        # Mock symlink test to fail
        mock_capability.return_value = False

        result = validate_symlink_support(nonexistent)
        # Should have error about symlink creation failing
        assert result.has_errors() is True

    @mock.patch("backend.model_library.io.validator.detect_sandbox_environment")
    def test_sandbox_warning(self, mock_sandbox, tmp_path: Path):
        """Test warning when in sandbox environment."""
        mock_sandbox.return_value = SandboxInfo(
            is_sandboxed=True, sandbox_type="flatpak", limitations=["Test limitation"]
        )

        result = validate_symlink_support(tmp_path)
        # Should have warning about sandbox
        assert result.has_warnings() is True

    @mock.patch("backend.model_library.io.validator.check_symlink_capability")
    def test_symlink_failure_error(self, mock_capability, tmp_path: Path):
        """Test error when symlink creation fails."""
        mock_capability.return_value = False

        result = validate_symlink_support(tmp_path)
        assert result.is_valid() is False
        assert result.has_errors() is True
