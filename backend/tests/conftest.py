"""
Shared pytest fixtures for backend tests.
"""

import subprocess
from typing import Any, Dict
from unittest.mock import MagicMock, Mock

import psutil
import pytest


@pytest.fixture
def mock_process():
    """Create a mock psutil.Process object with common attributes."""
    process = Mock(spec=psutil.Process)
    process.pid = 12345
    process.name.return_value = "python"
    process.cpu_percent.return_value = 25.5
    process.memory_info.return_value = Mock(rss=1024 * 1024 * 100)  # 100 MB
    process.children.return_value = []
    process.is_running.return_value = True
    return process


@pytest.fixture
def mock_process_with_children():
    """Create a mock process with child processes."""
    parent = Mock(spec=psutil.Process)
    parent.pid = 12345
    parent.name.return_value = "python"
    parent.cpu_percent.return_value = 20.0
    parent.memory_info.return_value = Mock(rss=1024 * 1024 * 150)  # 150 MB
    parent.is_running.return_value = True

    child1 = Mock(spec=psutil.Process)
    child1.pid = 12346
    child1.cpu_percent.return_value = 10.0
    child1.memory_info.return_value = Mock(rss=1024 * 1024 * 50)  # 50 MB
    child1.children.return_value = []
    child1.is_running.return_value = True

    child2 = Mock(spec=psutil.Process)
    child2.pid = 12347
    child2.cpu_percent.return_value = 10.0
    child2.memory_info.return_value = Mock(rss=1024 * 1024 * 50)  # 50 MB
    child2.children.return_value = []
    child2.is_running.return_value = True

    parent.children.return_value = [child1, child2]

    return parent, child1, child2


@pytest.fixture
def mock_nvidia_smi_output():
    """Return mock nvidia-smi output."""
    return """12345, 500
12346, 250
12347, 250"""


@pytest.fixture
def mock_nvidia_smi_empty():
    """Return empty nvidia-smi output (no GPU usage)."""
    return ""


@pytest.fixture
def mock_subprocess_run(mocker):
    """Create a mock for subprocess.run."""
    mock = mocker.patch("subprocess.run")
    return mock


@pytest.fixture
def sample_process_info() -> Dict[str, Any]:
    """Sample process information dict."""
    return {
        "pid": 12345,
        "source": "/path/to/comfyui/main.py",
        "tag": "v0.5.0",
        "cpu_usage": 0.0,
        "ram_memory": 0.0,
        "gpu_memory": 0.0,
    }


@pytest.fixture
def sample_status_response() -> Dict[str, Any]:
    """Sample status response from SystemUtils.get_status()."""
    return {
        "success": True,
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
                "source": "/path/to/comfyui/main.py",
                "tag": "v0.5.0",
                "cpu_usage": 25.5,
                "ram_memory": 1.5,
                "gpu_memory": 2.0,
            }
        ],
        "app_resources": {
            "comfyui": {
                "cpu": 25.5,
                "ram_memory": 1.5,
                "gpu_memory": 2.0,
            }
        },
        "message": "",
        "release_info": {},
        "last_launch_log": None,
        "last_launch_error": None,
    }


@pytest.fixture
def mock_process_manager(mocker):
    """Create a mock ProcessManager."""
    from backend.api.process_manager import ProcessManager

    mock = mocker.Mock(spec=ProcessManager)
    mock.get_processes_with_resources.return_value = []
    return mock


@pytest.fixture
def mock_resource_tracker(mocker):
    """Create a mock ProcessResourceTracker."""
    from backend.api.process_resource_tracker import ProcessResourceTracker

    mock = mocker.Mock(spec=ProcessResourceTracker)
    mock.get_process_resources.return_value = {
        "cpu": 0.0,
        "ram_memory": 0.0,
        "gpu_memory": 0.0,
    }
    return mock
