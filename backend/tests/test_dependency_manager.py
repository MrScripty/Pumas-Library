"""
Unit tests for DependencyManager.

Tests cover:
- Dependency checking (python packages, git, brave)
- Missing dependency detection
- Dependency installation
"""

import subprocess
from unittest.mock import Mock

import pytest

from backend.api.dependency_manager import DependencyManager

# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


def test_init_sets_script_dir(tmp_path):
    """Test that DependencyManager initializes with script directory."""
    manager = DependencyManager(script_dir=tmp_path)

    assert manager.script_dir == tmp_path


# ============================================================================
# DEPENDENCY CHECKING TESTS
# ============================================================================


def test_check_setproctitle_installed(mocker):
    """Test check_setproctitle returns True when module is installed."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock successful import
    mock_import = mocker.patch("builtins.__import__", return_value=Mock())

    result = manager.check_setproctitle()

    assert result is True


def test_check_setproctitle_missing(mocker):
    """Test check_setproctitle returns False when module is missing."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock import to raise ImportError
    mocker.patch("builtins.__import__", side_effect=ImportError())

    result = manager.check_setproctitle()

    assert result is False


def test_check_git_installed(mocker):
    """Test check_git returns True when git is installed."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock shutil.which to return git path
    mocker.patch("shutil.which", return_value="/usr/bin/git")

    result = manager.check_git()

    assert result is True


def test_check_git_missing(mocker):
    """Test check_git returns False when git is not installed."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock shutil.which to return None
    mocker.patch("shutil.which", return_value=None)

    result = manager.check_git()

    assert result is False


def test_check_brave_installed(mocker):
    """Test check_brave returns True when Brave is installed."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock shutil.which to return brave path
    mocker.patch("shutil.which", return_value="/usr/bin/brave-browser")

    result = manager.check_brave()

    assert result is True


def test_check_brave_missing(mocker):
    """Test check_brave returns False when Brave is not installed."""
    manager = DependencyManager(script_dir="/fake/path")

    # Mock shutil.which to return None
    mocker.patch("shutil.which", return_value=None)

    result = manager.check_brave()

    assert result is False


# ============================================================================
# MISSING DEPENDENCIES TESTS
# ============================================================================


def test_get_missing_dependencies_all_installed(mocker):
    """Test get_missing_dependencies when all deps are installed."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "check_python_package", return_value=True)
    mocker.patch.object(manager, "check_git", return_value=True)
    mocker.patch.object(manager, "check_brave", return_value=True)

    result = manager.get_missing_dependencies()

    assert result == []


def test_get_missing_dependencies_some_missing(mocker):
    """Test get_missing_dependencies when some deps are missing."""
    manager = DependencyManager(script_dir="/fake/path")

    def _check(module_name):
        return module_name != "setproctitle"

    mocker.patch.object(manager, "check_python_package", side_effect=_check)
    mocker.patch.object(manager, "check_git", return_value=True)
    mocker.patch.object(manager, "check_brave", return_value=False)

    result = manager.get_missing_dependencies()

    assert "setproctitle" in result
    assert "brave-browser" in result
    assert "git" not in result


def test_get_missing_dependencies_all_missing(mocker):
    """Test get_missing_dependencies when all deps are missing."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "check_python_package", return_value=False)
    mocker.patch.object(manager, "check_git", return_value=False)
    mocker.patch.object(manager, "check_brave", return_value=False)

    result = manager.get_missing_dependencies()

    assert len(result) == 7
    assert "setproctitle" in result
    assert "huggingface_hub" in result
    assert "pydantic" in result
    assert "tenacity" in result
    assert "blake3" in result
    assert "git" in result
    assert "brave-browser" in result


# ============================================================================
# INSTALLATION TESTS
# ============================================================================


def test_install_missing_dependencies_no_missing(mocker):
    """Test install_missing_dependencies when nothing is missing."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "get_missing_dependencies", return_value=[])

    result = manager.install_missing_dependencies()

    assert result is True


def test_install_missing_dependencies_setproctitle_success(mocker, tmp_path):
    """Test successful installation of setproctitle."""
    manager = DependencyManager(script_dir=tmp_path)

    mocker.patch.object(manager, "get_missing_dependencies", return_value=["setproctitle"])

    # Mock subprocess.run to simulate successful pip install
    mock_run = mocker.patch("subprocess.run")
    mock_run.return_value = Mock(returncode=0)

    result = manager.install_missing_dependencies()

    assert result is True
    # Verify pip3 install was called
    mock_run.assert_called_once()
    args = mock_run.call_args[0][0]
    assert "pip3" in args
    assert "install" in args
    assert "setproctitle" in args


def test_install_missing_dependencies_setproctitle_failure(mocker):
    """Test failed installation of setproctitle."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "get_missing_dependencies", return_value=["setproctitle"])

    # Mock subprocess.run to raise CalledProcessError
    mock_run = mocker.patch("subprocess.run")
    mock_run.side_effect = subprocess.CalledProcessError(1, "pip3")

    result = manager.install_missing_dependencies()

    assert result is False


def test_install_missing_dependencies_system_packages_success(mocker):
    """Test successful installation of system packages (git, brave)."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "get_missing_dependencies", return_value=["git", "brave-browser"])

    # Mock subprocess.run to simulate successful apt install
    mock_run = mocker.patch("subprocess.run")
    mock_run.return_value = Mock(returncode=0)

    result = manager.install_missing_dependencies()

    assert result is True
    # Should call apt update and apt install
    assert mock_run.call_count == 2


def test_install_missing_dependencies_system_packages_failure(mocker):
    """Test failed installation of system packages."""
    manager = DependencyManager(script_dir="/fake/path")

    mocker.patch.object(manager, "get_missing_dependencies", return_value=["git"])

    # Mock subprocess.run to raise exception on apt install
    mock_run = mocker.patch("subprocess.run")
    mock_run.side_effect = subprocess.CalledProcessError(1, "apt")

    result = manager.install_missing_dependencies()

    assert result is False


def test_install_missing_dependencies_mixed_success(mocker, tmp_path):
    """Test installation of both Python and system packages."""
    manager = DependencyManager(script_dir=tmp_path)

    mocker.patch.object(manager, "get_missing_dependencies", return_value=["setproctitle", "git"])

    # Mock successful subprocess calls
    mock_run = mocker.patch("subprocess.run")
    mock_run.return_value = Mock(returncode=0)

    result = manager.install_missing_dependencies()

    assert result is True
    # Should call pip3 install (1x) + apt update (1x) + apt install (1x) = 3x
    assert mock_run.call_count == 3
