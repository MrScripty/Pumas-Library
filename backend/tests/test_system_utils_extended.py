"""
Extended unit tests for SystemUtils.

Tests cover:
- Disk space reporting
- Toggle operations (patch, menu, desktop)
- Path and URL operations
- System resource monitoring
"""

import subprocess
from pathlib import Path
from unittest.mock import MagicMock, Mock
from unittest.mock import patch as mock_patch

import pytest

from backend.api.system_utils import SystemUtils
from backend.exceptions import ValidationError

# ============================================================================
# DISK SPACE TESTS
# ============================================================================


def test_get_disk_space_success(mocker, tmp_path):
    """Test successful disk space retrieval."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock shutil.disk_usage
    mock_stat = Mock(total=1000000000, used=400000000, free=600000000)
    mocker.patch("shutil.disk_usage", return_value=mock_stat)

    result = system_utils.get_disk_space()

    assert result["success"] is True
    assert result["total"] == 1000000000
    assert result["used"] == 400000000
    assert result["free"] == 600000000
    assert result["percent"] == 40.0


def test_get_disk_space_error_handling(mocker, tmp_path):
    """Test disk space error handling."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock shutil.disk_usage to raise OSError
    mocker.patch("shutil.disk_usage", side_effect=OSError("Permission denied"))

    result = system_utils.get_disk_space()

    assert result["success"] is False
    assert "Permission denied" in result["error"]
    assert result["total"] == 0


# ============================================================================
# TOGGLE PATCH TESTS
# ============================================================================


def test_toggle_patch_apply(mocker, tmp_path):
    """Test toggle_patch applies patch when not patched."""
    mock_patch_mgr = Mock()
    mock_patch_mgr.is_patched.return_value = False
    mock_patch_mgr.patch_main_py.return_value = True

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": mock_patch_mgr,
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_patch()

    assert result is True
    mock_patch_mgr.patch_main_py.assert_called_once()


def test_toggle_patch_revert(mocker, tmp_path):
    """Test toggle_patch reverts patch when already patched."""
    mock_patch_mgr = Mock()
    mock_patch_mgr.is_patched.return_value = True
    mock_patch_mgr.revert_main_py.return_value = True

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": mock_patch_mgr,
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_patch()

    assert result is True
    mock_patch_mgr.revert_main_py.assert_called_once()


# ============================================================================
# TOGGLE MENU TESTS
# ============================================================================


def test_toggle_menu_with_active_version(mocker, tmp_path):
    """Test toggle_menu with active version."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.5.0"

    mock_shortcut_mgr = Mock()
    mock_shortcut_mgr.toggle_version_menu_shortcut.return_value = {"success": True}

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": mock_shortcut_mgr,
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_menu()

    assert result is True
    mock_shortcut_mgr.toggle_version_menu_shortcut.assert_called_once_with("v0.5.0")


def test_toggle_menu_no_active_version(mocker, tmp_path):
    """Test toggle_menu with no active version."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = None

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_menu()

    assert result is False


def test_toggle_menu_with_explicit_tag(mocker, tmp_path):
    """Test toggle_menu with explicit version tag."""
    mock_shortcut_mgr = Mock()
    mock_shortcut_mgr.toggle_version_menu_shortcut.return_value = {"success": True}

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": mock_shortcut_mgr,
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_menu("v0.6.0")

    assert result is True
    mock_shortcut_mgr.toggle_version_menu_shortcut.assert_called_once_with("v0.6.0")


# ============================================================================
# TOGGLE DESKTOP TESTS
# ============================================================================


def test_toggle_desktop_with_active_version(mocker, tmp_path):
    """Test toggle_desktop with active version."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.5.0"

    mock_shortcut_mgr = Mock()
    mock_shortcut_mgr.toggle_version_desktop_shortcut.return_value = {"success": True}

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": mock_shortcut_mgr,
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_desktop()

    assert result is True
    mock_shortcut_mgr.toggle_version_desktop_shortcut.assert_called_once_with("v0.5.0")


def test_toggle_desktop_no_active_version(mocker, tmp_path):
    """Test toggle_desktop with no active version."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = None

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.toggle_desktop()

    assert result is False


# ============================================================================
# OPEN PATH TESTS
# ============================================================================


def test_open_path_success(mocker, tmp_path):
    """Test successful path opening."""
    target_dir = tmp_path / "target"
    target_dir.mkdir()

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock open_in_file_manager
    mock_open = mocker.patch(
        "backend.api.system_utils.open_in_file_manager", return_value={"success": True}
    )

    result = system_utils.open_path(str(target_dir))

    assert result["success"] is True
    mock_open.assert_called_once()


def test_open_path_validation_error(mocker, tmp_path):
    """Test open_path with invalid path."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock sanitize_path to raise ValidationError
    mocker.patch(
        "backend.api.system_utils.sanitize_path",
        side_effect=ValidationError("Invalid path"),
    )

    result = system_utils.open_path("../../../etc/passwd")

    assert result["success"] is False
    assert "Invalid path" in result["error"]


# ============================================================================
# OPEN URL TESTS
# ============================================================================


def test_open_url_success(mocker, tmp_path):
    """Test successful URL opening."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock webbrowser.open
    mock_browser = mocker.patch("webbrowser.open", return_value=True)

    result = system_utils.open_url("https://example.com")

    assert result["success"] is True
    mock_browser.assert_called_once_with("https://example.com", new=2)


def test_open_url_invalid_url(mocker, tmp_path):
    """Test open_url with invalid URL."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.open_url("javascript:alert('xss')")

    assert result["success"] is False
    assert "http/https" in result["error"]


def test_open_url_xdg_open_fallback(mocker, tmp_path):
    """Test open_url falls back to xdg-open when webbrowser fails."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock webbrowser.open to return False
    mocker.patch("webbrowser.open", return_value=False)

    # Mock shutil.which to return xdg-open path
    mocker.patch("shutil.which", return_value="/usr/bin/xdg-open")

    # Mock subprocess.run for xdg-open
    mock_run = mocker.patch("subprocess.run", return_value=Mock(returncode=0))

    result = system_utils.open_url("https://example.com")

    assert result["success"] is True
    mock_run.assert_called_once()


# ============================================================================
# OPEN ACTIVE INSTALL TESTS
# ============================================================================


def test_open_active_install_success(mocker, tmp_path):
    """Test successful opening of active installation."""
    version_path = tmp_path / "v0.5.0"
    version_path.mkdir()

    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version_path.return_value = version_path

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock open_in_file_manager
    mock_open = mocker.patch(
        "backend.api.system_utils.open_in_file_manager", return_value={"success": True}
    )

    result = system_utils.open_active_install()

    assert result["success"] is True


def test_open_active_install_no_version_manager(mocker, tmp_path):
    """Test open_active_install with no version manager."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.open_active_install()

    assert result["success"] is False
    assert "not initialized" in result["error"]


def test_open_active_install_no_active_version(mocker, tmp_path):
    """Test open_active_install with no active version."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version_path.return_value = None

    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
        "version_manager": mock_version_mgr,
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    result = system_utils.open_active_install()

    assert result["success"] is False
    assert "No active version" in result["error"]


# ============================================================================
# SYSTEM RESOURCES TESTS
# ============================================================================


def test_get_system_resources_with_psutil(mocker, tmp_path):
    """Test get_system_resources with psutil available."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock psutil functions
    with mock_patch("backend.api.system_utils.psutil") as mock_psutil:
        mock_psutil.cpu_percent.return_value = 45.2
        mock_psutil.virtual_memory.return_value = Mock(total=16 * 1024**3, available=8 * 1024**3)

        # Mock shutil.disk_usage
        mocker.patch(
            "shutil.disk_usage",
            return_value=Mock(total=500 * 1024**3, used=300 * 1024**3, free=200 * 1024**3),
        )

        # Mock nvidia-smi
        mock_run = mocker.patch("subprocess.run")
        mock_run.return_value = Mock(returncode=0, stdout="75, 8192, 11264\n")

        result = system_utils.get_system_resources()

        assert result["success"] is True
        assert result["resources"]["cpu"]["usage"] == 45.2
        assert result["resources"]["ram"]["usage"] == 8.0
        assert result["resources"]["gpu"]["usage"] == 75.0
        assert result["resources"]["gpu"]["memory"] == 8.0
        assert result["resources"]["gpu"]["memory_total"] == 11.0


def test_get_system_resources_without_psutil(mocker, tmp_path):
    """Test get_system_resources when psutil is not available."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    # Mock psutil to be None
    with mock_patch("backend.api.system_utils.psutil", None):
        result = system_utils.get_system_resources()

        assert result["success"] is False
        assert "psutil not available" in result["error"]
        assert result["resources"]["cpu"]["usage"] == 0


def test_get_system_resources_gpu_not_available(mocker, tmp_path):
    """Test get_system_resources when GPU is not available."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    with mock_patch("backend.api.system_utils.psutil") as mock_psutil:
        mock_psutil.cpu_percent.return_value = 30.0
        mock_psutil.virtual_memory.return_value = Mock(total=8 * 1024**3, available=4 * 1024**3)

        mocker.patch(
            "shutil.disk_usage",
            return_value=Mock(total=100 * 1024**3, used=50 * 1024**3, free=50 * 1024**3),
        )

        # Mock nvidia-smi to fail
        mock_run = mocker.patch("subprocess.run")
        mock_run.side_effect = FileNotFoundError()

        result = system_utils.get_system_resources()

        assert result["success"] is True
        assert result["resources"]["gpu"]["usage"] == 0
        assert result["resources"]["gpu"]["memory"] == 0


def test_get_system_resources_exception_handling(mocker, tmp_path):
    """Test get_system_resources handles exceptions gracefully."""
    mock_managers = {
        "dependency_manager": Mock(),
        "patch_manager": Mock(),
        "shortcut_manager": Mock(),
        "process_manager": Mock(),
        "version_info_manager": Mock(),
    }

    system_utils = SystemUtils(script_dir=tmp_path, **mock_managers)

    with mock_patch("backend.api.system_utils.psutil") as mock_psutil:
        mock_psutil.cpu_percent.side_effect = ValueError("CPU error")

        result = system_utils.get_system_resources()

        assert result["success"] is False
        assert "CPU error" in result["error"]
