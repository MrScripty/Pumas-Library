"""
Unit tests for JavaScriptAPI resource tracking exposure.

Tests cover:
- JavaScriptAPI initialization with ComfyUISetupAPI
- Resource data exposure through JavaScript bridge
"""

from unittest.mock import Mock, patch

import pytest

from backend.main import JavaScriptAPI

# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


@patch("backend.main.ComfyUISetupAPI")
def test_javascript_api_init(MockComfyUISetupAPI):
    """Test JavaScriptAPI initialization creates ComfyUISetupAPI."""
    mock_api = Mock()
    MockComfyUISetupAPI.return_value = mock_api

    js_api = JavaScriptAPI()

    MockComfyUISetupAPI.assert_called_once()
    assert js_api.api == mock_api


# ============================================================================
# STATUS METHOD TESTS
# ============================================================================


@patch("backend.main.ComfyUISetupAPI")
def test_get_status_exposes_resource_data(MockComfyUISetupAPI, sample_status_response):
    """Test that get_status exposes resource data to JavaScript."""
    mock_api = Mock()
    mock_api.get_status.return_value = sample_status_response
    MockComfyUISetupAPI.return_value = mock_api

    js_api = JavaScriptAPI()
    status = js_api.get_status()

    # Verify get_status was called on the API
    mock_api.get_status.assert_called_once()

    # Verify the response includes resource data
    assert "running_processes" in status
    assert "app_resources" in status
    assert len(status["running_processes"]) == 1
    assert status["running_processes"][0]["cpu_usage"] == 25.5
    assert status["app_resources"]["comfyui"]["cpu"] == 25.5


@patch("backend.main.ComfyUISetupAPI")
def test_get_status_with_multiple_processes(MockComfyUISetupAPI):
    """Test get_status with multiple running processes."""
    mock_api = Mock()
    mock_api.get_status.return_value = {
        "version": "0.1.0",
        "deps_ready": True,
        "missing_deps": [],
        "patched": True,
        "menu_shortcut": True,
        "desktop_shortcut": True,
        "shortcut_version": "v0.5.0",
        "comfyui_running": True,
        "running_processes": [
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
        ],
        "app_resources": {
            "comfyui": {
                "cpu": 50.0,
                "ram_memory": 3.0,
                "gpu_memory": 4.0,
            }
        },
        "message": "",
        "release_info": {},
        "last_launch_log": None,
        "last_launch_error": None,
    }
    MockComfyUISetupAPI.return_value = mock_api

    js_api = JavaScriptAPI()
    status = js_api.get_status()

    # Verify aggregated resources
    assert len(status["running_processes"]) == 2
    assert status["app_resources"]["comfyui"]["cpu"] == 50.0
    assert status["app_resources"]["comfyui"]["ram_memory"] == 3.0
    assert status["app_resources"]["comfyui"]["gpu_memory"] == 4.0
