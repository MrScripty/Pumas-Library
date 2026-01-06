"""
Unit tests for ProcessManager resource tracking integration.

Tests cover:
- Resource tracker initialization
- Process detection with resource enrichment
- Error handling when resource tracking fails
"""

from unittest.mock import MagicMock, Mock, patch

import psutil
import pytest

from backend.api.process_manager import ProcessManager
from backend.api.process_resource_tracker import ProcessResourceTracker

# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


def test_init_creates_resource_tracker():
    """Test that ProcessManager initializes with a ProcessResourceTracker."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    assert hasattr(manager, "resource_tracker")
    assert isinstance(manager.resource_tracker, ProcessResourceTracker)


def test_init_resource_tracker_with_custom_ttl():
    """Test resource tracker initialization with custom TTL."""
    # ProcessManager uses default TTL of 2.0
    manager = ProcessManager(comfyui_dir="/fake/path")

    # Check that tracker was created with expected TTL
    assert manager.resource_tracker._cache_ttl == 2.0


# ============================================================================
# RESOURCE ENRICHMENT TESTS
# ============================================================================


def test_get_processes_with_resources_empty_list(mocker):
    """Test resource enrichment with no running processes."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    # Mock _detect_comfyui_processes to return empty list
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[])

    result = manager.get_processes_with_resources()

    assert result == []


def test_get_processes_with_resources_single_process(mocker, sample_process_info):
    """Test resource enrichment for a single process."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    # Mock process detection
    processes = [sample_process_info.copy()]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    # Mock resource tracker
    mock_resources = {
        "cpu": 25.5,
        "ram_memory": 1.5,
        "gpu_memory": 2.0,
    }
    mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
        return_value=mock_resources,
    )

    result = manager.get_processes_with_resources()

    assert len(result) == 1
    assert result[0]["pid"] == 12345
    assert result[0]["cpu_usage"] == 25.5
    assert result[0]["ram_memory"] == 1.5
    assert result[0]["gpu_memory"] == 2.0


def test_get_processes_with_resources_multiple_processes(mocker):
    """Test resource enrichment for multiple processes."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    # Mock multiple processes
    processes = [
        {"pid": 12345, "source": "/path/to/comfyui1/main.py", "tag": "v0.5.0"},
        {"pid": 67890, "source": "/path/to/comfyui2/main.py", "tag": "v0.6.0"},
    ]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    # Mock resource tracker to return different values for each PID
    def mock_get_resources(pid, include_children):
        if pid == 12345:
            return {"cpu": 20.0, "ram_memory": 1.0, "gpu_memory": 1.5}
        else:
            return {"cpu": 30.0, "ram_memory": 2.0, "gpu_memory": 2.5}

    mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
        side_effect=mock_get_resources,
    )

    result = manager.get_processes_with_resources()

    assert len(result) == 2
    assert result[0]["cpu_usage"] == 20.0
    assert result[1]["cpu_usage"] == 30.0


def test_get_processes_with_resources_includes_children(mocker, sample_process_info):
    """Test that resource tracking includes child processes."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    processes = [sample_process_info.copy()]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    mock_resources = {"cpu": 40.0, "ram_memory": 3.0, "gpu_memory": 4.0}
    mock_get_resources = mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
        return_value=mock_resources,
    )

    manager.get_processes_with_resources()

    # Verify include_children=True was passed
    mock_get_resources.assert_called_once_with(12345, include_children=True)


# ============================================================================
# ERROR HANDLING TESTS
# ============================================================================


def test_get_processes_with_resources_resource_tracker_fails(mocker, sample_process_info):
    """Test that process info is still returned when resource tracking fails."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    processes = [sample_process_info.copy()]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    # Mock resource tracker to raise an exception
    mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
        side_effect=ValueError("Resource tracking failed"),
    )

    result = manager.get_processes_with_resources()

    # Process should still be in list with zero resources
    assert len(result) == 1
    assert result[0]["pid"] == 12345
    assert result[0]["cpu_usage"] == 0.0
    assert result[0]["ram_memory"] == 0.0
    assert result[0]["gpu_memory"] == 0.0


def test_get_processes_with_resources_invalid_pid(mocker):
    """Test handling of process with invalid PID."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    # Process with non-integer PID
    processes = [{"pid": None, "source": "/path/to/comfyui/main.py", "tag": "v0.5.0"}]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    mock_get_resources = mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
    )

    result = manager.get_processes_with_resources()

    # Resource tracker should not be called for invalid PID
    mock_get_resources.assert_not_called()

    # Process should be returned unchanged
    assert len(result) == 1
    assert result[0]["pid"] is None


def test_get_processes_with_resources_mixed_valid_invalid_pids(mocker):
    """Test handling of mixed valid and invalid PIDs."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    processes = [
        {"pid": 12345, "source": "/path/to/comfyui1/main.py", "tag": "v0.5.0"},
        {"pid": None, "source": "/path/to/comfyui2/main.py", "tag": "v0.6.0"},
        {"pid": 67890, "source": "/path/to/comfyui3/main.py", "tag": "v0.7.0"},
    ]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=processes)

    # Mock resource tracker
    def mock_get_resources(pid, include_children):
        if pid == 12345:
            return {"cpu": 20.0, "ram_memory": 1.0, "gpu_memory": 1.5}
        else:
            return {"cpu": 30.0, "ram_memory": 2.0, "gpu_memory": 2.5}

    mocker.patch.object(
        manager.resource_tracker,
        "get_process_resources",
        side_effect=mock_get_resources,
    )

    result = manager.get_processes_with_resources()

    assert len(result) == 3
    # Valid PIDs should have resource data
    assert result[0]["cpu_usage"] == 20.0
    assert result[2]["cpu_usage"] == 30.0
    # Invalid PID should still have resource fields (set to 0.0)
    assert result[1]["cpu_usage"] == 0.0
    assert result[1]["ram_memory"] == 0.0
    assert result[1]["gpu_memory"] == 0.0
