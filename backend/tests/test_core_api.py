"""
Unit tests for ComfyUISetupAPI initialization with ProcessResourceTracker.

Tests cover:
- ProcessManager initialization with ProcessResourceTracker
- Integration of resource tracking in API workflows
"""

from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.api.process_manager import ProcessManager
from backend.api.process_resource_tracker import ProcessResourceTracker

# ============================================================================
# COMPONENT INTEGRATION TESTS
# ============================================================================


def test_process_manager_has_resource_tracker():
    """Test that ProcessManager has a resource tracker after initialization."""
    # Direct test - ProcessManager should create its own ProcessResourceTracker
    manager = ProcessManager(comfyui_dir=Path("/fake/path"))

    assert hasattr(manager, "resource_tracker")
    assert isinstance(manager.resource_tracker, ProcessResourceTracker)
    assert manager.resource_tracker._cache_ttl == 2.0


def test_process_manager_get_processes_with_resources_exists():
    """Test that ProcessManager has get_processes_with_resources method."""
    manager = ProcessManager(comfyui_dir=Path("/fake/path"))

    # Method should exist
    assert hasattr(manager, "get_processes_with_resources")
    assert callable(manager.get_processes_with_resources)


# ============================================================================
# SYSTEM INTEGRATION TESTS
# ============================================================================


def test_resource_tracking_integration(mocker):
    """Test that resource tracking integrates into the process detection workflow."""
    manager = ProcessManager(comfyui_dir=Path("/fake/path"))

    # Mock process detection
    mock_processes = [{"pid": 12345, "source": "test", "tag": "v0.5.0"}]
    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=mock_processes)

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

    # Call get_processes_with_resources
    result = manager.get_processes_with_resources()

    # Verify resource tracker was used
    manager.resource_tracker.get_process_resources.assert_called_once_with(
        12345, include_children=True
    )

    # Verify results include resource data
    assert len(result) == 1
    assert result[0]["cpu_usage"] == 25.5
    assert result[0]["ram_memory"] == 1.5
    assert result[0]["gpu_memory"] == 2.0
