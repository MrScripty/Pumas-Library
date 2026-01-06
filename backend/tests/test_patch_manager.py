"""
Unit tests for PatchManager.

Tests cover:
- Patch detection and application
- Version-specific patching
- Patch reversal
- Backup and restoration
"""

import subprocess
import urllib.error
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.api.patch_manager import PatchManager

# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


def test_init_sets_paths(tmp_path):
    """Test that PatchManager initializes with correct paths."""
    main_py = tmp_path / "main.py"
    main_py.touch()

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    assert manager.comfyui_dir == tmp_path
    assert manager.main_py == main_py


# ============================================================================
# TITLE BUILDING TESTS
# ============================================================================


def test_build_server_title_no_tag():
    """Test server title without version tag."""
    manager = PatchManager(comfyui_dir="/fake/path", main_py="/fake/path/main.py")

    title = manager._build_server_title()

    assert title == "ComfyUI Server"


def test_build_server_title_with_tag():
    """Test server title with version tag."""
    manager = PatchManager(comfyui_dir="/fake/path", main_py="/fake/path/main.py")

    title = manager._build_server_title("v0.5.0")

    assert title == "ComfyUI Server - v0.5.0"


# ============================================================================
# PATCH DETECTION TESTS
# ============================================================================


def test_is_patched_unpatched_file(tmp_path):
    """Test is_patched returns False for unpatched file."""
    main_py = tmp_path / "main.py"
    main_py.write_text("# Original content\nlogger.info('Hello')")

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    assert manager.is_patched() is False


def test_is_patched_patched_file(tmp_path):
    """Test is_patched returns True for patched file."""
    main_py = tmp_path / "main.py"
    main_py.write_text(
        """
import setproctitle
setproctitle.setproctitle("ComfyUI Server")
logger.info('Hello')
"""
    )

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    assert manager.is_patched() is True


def test_is_patched_with_version_tag(tmp_path):
    """Test is_patched detects version-specific patch."""
    main_py = tmp_path / "main.py"
    main_py.write_text(
        """
import setproctitle
setproctitle.setproctitle("ComfyUI Server - v0.5.0")
logger.info('Hello')
"""
    )

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    assert manager.is_patched() is True


def test_is_patched_no_main_py(tmp_path):
    """Test is_patched returns False when main.py doesn't exist."""
    main_py = tmp_path / "main.py"  # Don't create it

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    assert manager.is_patched() is False


# ============================================================================
# PATCH APPLICATION TESTS
# ============================================================================


def test_patch_main_py_success(tmp_path):
    """Test successful patch application."""
    main_py = tmp_path / "main.py"
    original_content = 'if __name__ == "__main__":\n    logger.info("Starting")'
    main_py.write_text(original_content)

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.patch_main_py()

    assert result is True

    # Check patch was applied
    content = main_py.read_text()
    assert "setproctitle" in content
    assert "ComfyUI Server" in content

    # Check backup was created
    backup = tmp_path / "main.py.bak"
    assert backup.exists()
    assert backup.read_text() == original_content


def test_patch_main_py_already_patched(tmp_path):
    """Test patch_main_py returns False when already patched."""
    main_py = tmp_path / "main.py"
    main_py.write_text(
        """
import setproctitle
setproctitle.setproctitle("ComfyUI Server")
if __name__ == "__main__":
    logger.info("Starting")
"""
    )

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.patch_main_py()

    # Already patched, should return False
    assert result is False


def test_patch_main_py_upgrades_old_patch(tmp_path):
    """Test patch_main_py upgrades old patch to version-specific one."""
    main_py = tmp_path / "main.py"
    main_py.write_text(
        """
import setproctitle
setproctitle.setproctitle("ComfyUI Server")
if __name__ == "__main__":
    logger.info("Starting")
"""
    )

    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.6.0"
    mock_version_mgr.get_active_version_path.return_value = tmp_path

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py, version_manager=mock_version_mgr)

    result = manager.patch_main_py()

    assert result is True

    # Check patch was upgraded to version-specific
    content = main_py.read_text()
    assert "ComfyUI Server - v0.6.0" in content


def test_patch_main_py_no_main_block(tmp_path):
    """Test patch_main_py when file has no __main__ block."""
    main_py = tmp_path / "main.py"
    main_py.write_text("logger.info('Hello')")

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.patch_main_py()

    assert result is True

    # Patch should be appended to end
    content = main_py.read_text()
    assert "setproctitle" in content


def test_patch_main_py_file_not_found(tmp_path):
    """Test patch_main_py returns False when file doesn't exist."""
    main_py = tmp_path / "main.py"  # Don't create it

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.patch_main_py()

    assert result is False


# ============================================================================
# PATCH REVERSION TESTS
# ============================================================================


def test_revert_main_py_from_backup(tmp_path):
    """Test revert_main_py restores from backup."""
    main_py = tmp_path / "main.py"
    backup = tmp_path / "main.py.bak"

    original = "# Original content"
    patched = "# Patched content"

    backup.write_text(original)
    main_py.write_text(patched)

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.revert_main_py()

    assert result is True
    assert main_py.read_text() == original
    assert not backup.exists()  # Backup should be deleted


def test_revert_main_py_from_git(mocker, tmp_path):
    """Test revert_main_py uses git when backup is missing."""
    main_py = tmp_path / "main.py"
    git_dir = tmp_path / ".git"
    git_dir.mkdir()

    main_py.write_text("# Patched")

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    # Mock successful git checkout
    mock_run = mocker.patch("subprocess.run")
    mock_run.return_value = Mock(returncode=0)

    result = manager.revert_main_py()

    assert result is True
    # Verify git checkout was called
    mock_run.assert_called_once()
    args = mock_run.call_args[0][0]
    assert "git" in args
    assert "checkout" in args


def test_revert_main_py_from_github(mocker, tmp_path):
    """Test revert_main_py downloads from GitHub when git fails."""
    main_py = tmp_path / "main.py"
    main_py.write_text("# Patched")

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    # Mock urllib to return original content
    original = b"# Original from GitHub"
    mock_urlopen = mocker.patch("urllib.request.urlopen")
    mock_response = MagicMock()
    mock_response.__enter__.return_value.read.return_value = original
    mock_urlopen.return_value = mock_response

    result = manager.revert_main_py()

    assert result is True
    assert main_py.read_bytes() == original


def test_revert_main_py_all_methods_fail(mocker, tmp_path):
    """Test revert_main_py returns False when all methods fail."""
    main_py = tmp_path / "main.py"
    main_py.write_text("# Patched")

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    # Mock urllib to raise URLError
    mock_urlopen = mocker.patch("urllib.request.urlopen")
    mock_urlopen.side_effect = urllib.error.URLError("Network error")

    result = manager.revert_main_py()

    assert result is False


def test_revert_main_py_no_main_py(tmp_path):
    """Test revert_main_py returns False when main.py doesn't exist."""
    main_py = tmp_path / "main.py"  # Don't create it

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    result = manager.revert_main_py()

    assert result is False


# ============================================================================
# VERSION-SPECIFIC PATCHING TESTS
# ============================================================================


def test_get_target_main_py_with_explicit_tag(mocker, tmp_path):
    """Test _get_target_main_py with explicit version tag."""
    version_dir = tmp_path / "v0.5.0"
    version_dir.mkdir()
    main_py = version_dir / "main.py"
    main_py.touch()

    mock_version_mgr = Mock()
    mock_version_mgr.get_version_path.return_value = version_dir

    manager = PatchManager(
        comfyui_dir=tmp_path,
        main_py=tmp_path / "main.py",
        version_manager=mock_version_mgr,
    )

    target, tag = manager._get_target_main_py("v0.5.0")

    assert target == main_py
    assert tag == "v0.5.0"


def test_get_target_main_py_with_active_version(mocker, tmp_path):
    """Test _get_target_main_py uses active version when no tag specified."""
    version_dir = tmp_path / "v0.6.0"
    version_dir.mkdir()
    main_py = version_dir / "main.py"
    main_py.touch()

    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.6.0"
    mock_version_mgr.get_active_version_path.return_value = version_dir

    manager = PatchManager(
        comfyui_dir=tmp_path,
        main_py=tmp_path / "main.py",
        version_manager=mock_version_mgr,
    )

    target, tag = manager._get_target_main_py()

    assert target == main_py
    assert tag == "v0.6.0"


def test_get_target_main_py_fallback_to_legacy(tmp_path):
    """Test _get_target_main_py falls back to legacy main.py."""
    main_py = tmp_path / "main.py"
    main_py.touch()

    manager = PatchManager(comfyui_dir=tmp_path, main_py=main_py)

    target, tag = manager._get_target_main_py()

    assert target == main_py
    assert tag is None
