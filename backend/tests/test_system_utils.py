"""
Unit tests for SystemUtils resource tracking integration.

Tests cover:
- Status reporting with process resources
- Resource aggregation for multiple processes
- Handling of missing resource data
"""

from pathlib import Path
from unittest.mock import MagicMock, Mock

import pytest

from backend.api.system_utils import SystemUtils

# ============================================================================
# TEST FIXTURES
# ============================================================================


@pytest.fixture
def mock_managers():
    """Create mock manager objects for SystemUtils."""
    return {
        "dependency_manager": Mock(get_missing_dependencies=Mock(return_value=[])),
        "patch_manager": Mock(is_patched=Mock(return_value=True)),
        "shortcut_manager": Mock(
            get_version_shortcut_state=Mock(return_value={"menu": True, "desktop": True})
        ),
        "process_manager": Mock(
            get_processes_with_resources=Mock(return_value=[]),
            last_launch_log=None,
            last_launch_error=None,
        ),
        "version_info_manager": Mock(
            get_comfyui_version=Mock(return_value="0.1.0"),
            check_for_new_release=Mock(return_value={}),
        ),
        "version_manager": Mock(get_active_version=Mock(return_value="v0.5.0")),
    }


@pytest.fixture
def system_utils(mock_managers):
    """Create a SystemUtils instance with mocked dependencies."""
    return SystemUtils(
        script_dir=Path("/fake/path"),
        dependency_manager=mock_managers["dependency_manager"],
        patch_manager=mock_managers["patch_manager"],
        shortcut_manager=mock_managers["shortcut_manager"],
        process_manager=mock_managers["process_manager"],
        version_info_manager=mock_managers["version_info_manager"],
        version_manager=mock_managers["version_manager"],
    )


# ============================================================================
# STATUS REPORTING TESTS
# ============================================================================


def test_get_status_no_running_processes(system_utils, mock_managers):
    """Test get_status when no processes are running."""
    mock_managers["process_manager"].get_processes_with_resources.return_value = []

    status = system_utils.get_status()

    assert status["comfyui_running"] is False
    assert status["running_processes"] == []
    assert status["app_resources"] == {}


def test_get_status_with_running_process(system_utils, mock_managers):
    """Test get_status with a single running process."""
    processes = [
        {
            "pid": 12345,
            "source": "/path/to/comfyui/main.py",
            "tag": "v0.5.0",
            "cpu_usage": 25.5,
            "ram_memory": 1.5,
            "gpu_memory": 2.0,
        }
    ]
    mock_managers["process_manager"].get_processes_with_resources.return_value = processes

    status = system_utils.get_status()

    assert status["comfyui_running"] is True
    assert len(status["running_processes"]) == 1
    assert status["running_processes"][0]["pid"] == 12345

    # Check aggregated resources
    assert "comfyui" in status["app_resources"]
    assert status["app_resources"]["comfyui"]["cpu"] == 25.5
    assert status["app_resources"]["comfyui"]["ram_memory"] == 1.5
    assert status["app_resources"]["comfyui"]["gpu_memory"] == 2.0


def test_get_status_aggregates_multiple_processes(system_utils, mock_managers):
    """Test that get_status correctly aggregates resources from multiple processes."""
    processes = [
        {
            "pid": 12345,
            "source": "/path/to/comfyui1/main.py",
            "tag": "v0.5.0",
            "cpu_usage": 20.0,
            "ram_memory": 1.0,
            "gpu_memory": 1.5,
        },
        {
            "pid": 67890,
            "source": "/path/to/comfyui2/main.py",
            "tag": "v0.6.0",
            "cpu_usage": 30.0,
            "ram_memory": 2.0,
            "gpu_memory": 2.5,
        },
    ]
    mock_managers["process_manager"].get_processes_with_resources.return_value = processes

    status = system_utils.get_status()

    assert status["comfyui_running"] is True
    assert len(status["running_processes"]) == 2

    # Check aggregated resources (should be sum of both processes)
    assert status["app_resources"]["comfyui"]["cpu"] == 50.0  # 20 + 30
    assert status["app_resources"]["comfyui"]["ram_memory"] == 3.0  # 1.0 + 2.0
    assert status["app_resources"]["comfyui"]["gpu_memory"] == 4.0  # 1.5 + 2.5


def test_get_status_handles_missing_resource_fields(system_utils, mock_managers):
    """Test that get_status handles processes missing resource fields."""
    processes = [
        {
            "pid": 12345,
            "source": "/path/to/comfyui/main.py",
            "tag": "v0.5.0",
            # Missing cpu_usage, ram_memory, gpu_memory
        }
    ]
    mock_managers["process_manager"].get_processes_with_resources.return_value = processes

    status = system_utils.get_status()

    # Should handle missing fields gracefully
    assert status["comfyui_running"] is True
    assert "comfyui" in status["app_resources"]
    assert status["app_resources"]["comfyui"]["cpu"] == 0.0
    assert status["app_resources"]["comfyui"]["ram_memory"] == 0.0
    assert status["app_resources"]["comfyui"]["gpu_memory"] == 0.0


def test_get_status_rounds_resource_values(system_utils, mock_managers):
    """Test that get_status rounds resource values correctly."""
    processes = [
        {
            "pid": 12345,
            "source": "/path/to/comfyui/main.py",
            "tag": "v0.5.0",
            "cpu_usage": 25.567,
            "ram_memory": 1.5678,
            "gpu_memory": 2.3456,
        }
    ]
    mock_managers["process_manager"].get_processes_with_resources.return_value = processes

    status = system_utils.get_status()

    # CPU rounded to 1 decimal place
    assert status["app_resources"]["comfyui"]["cpu"] == 25.6

    # RAM and GPU rounded to 2 decimal places
    assert status["app_resources"]["comfyui"]["ram_memory"] == 1.57
    assert status["app_resources"]["comfyui"]["gpu_memory"] == 2.35


def test_get_status_includes_all_required_fields(system_utils, mock_managers):
    """Test that get_status includes all required fields."""
    status = system_utils.get_status()

    required_fields = [
        "version",
        "deps_ready",
        "missing_deps",
        "patched",
        "menu_shortcut",
        "desktop_shortcut",
        "shortcut_version",
        "comfyui_running",
        "running_processes",
        "app_resources",
        "message",
        "release_info",
        "last_launch_log",
        "last_launch_error",
    ]

    for field in required_fields:
        assert field in status, f"Missing required field: {field}"


def test_get_status_calls_get_processes_with_resources(system_utils, mock_managers):
    """Test that get_status calls get_processes_with_resources instead of old method."""
    system_utils.get_status()

    # Should call the new method
    mock_managers["process_manager"].get_processes_with_resources.assert_called_once()


def test_get_status_message_with_running_processes(system_utils, mock_managers):
    """Test that status message is empty when processes are running."""
    processes = [
        {
            "pid": 12345,
            "source": "/path/to/comfyui/main.py",
            "tag": "v0.5.0",
            "cpu_usage": 25.5,
            "ram_memory": 1.5,
            "gpu_memory": 2.0,
        }
    ]
    mock_managers["process_manager"].get_processes_with_resources.return_value = processes

    status = system_utils.get_status()

    # Message should be empty when running
    assert status["message"] == ""
