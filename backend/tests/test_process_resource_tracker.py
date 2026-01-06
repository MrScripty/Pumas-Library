"""
Unit tests for ProcessResourceTracker.

Tests cover:
- Initialization and configuration
- CPU usage tracking (single process and with children)
- RAM memory tracking (single process and with children)
- GPU memory tracking via nvidia-smi
- Caching mechanisms (process cache and GPU cache)
- Error handling and edge cases
- Integration scenarios
"""

import subprocess
import time
from unittest.mock import MagicMock, Mock, patch

import psutil
import pytest
from freezegun import freeze_time

from backend.api.process_resource_tracker import ProcessResourceTracker

# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


def test_init_default_ttl():
    """Test initialization with default cache TTL."""
    tracker = ProcessResourceTracker()
    assert tracker._cache_ttl == 2.0
    assert tracker._cache == {}
    assert tracker._gpu_cache is None
    assert tracker._gpu_cache_time == 0.0


def test_init_custom_ttl():
    """Test initialization with custom cache TTL."""
    tracker = ProcessResourceTracker(cache_ttl=5.0)
    assert tracker._cache_ttl == 5.0


def test_init_zero_ttl():
    """Test initialization with zero TTL (caching disabled)."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    assert tracker._cache_ttl == 0.0


# ============================================================================
# CPU TRACKING TESTS
# ============================================================================


def test_get_process_cpu_single_process(mocker, mock_process):
    """Test CPU usage tracking for single process without children."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)  # Disable cache for test
    mocker.patch("psutil.Process", return_value=mock_process)

    cpu = tracker._get_process_cpu(12345, include_children=False)

    assert cpu == 25.5
    mock_process.cpu_percent.assert_called_once()
    mock_process.children.assert_not_called()


def test_get_process_cpu_with_children(mocker, mock_process_with_children):
    """Test CPU usage aggregation across process tree."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    parent, child1, child2 = mock_process_with_children

    mocker.patch("psutil.Process", return_value=parent)

    cpu = tracker._get_process_cpu(12345, include_children=True)

    # Should be 20.0 (parent) + 10.0 (child1) + 10.0 (child2) = 40.0
    assert cpu == 40.0
    parent.children.assert_called_once_with(recursive=True)


def test_get_process_cpu_nosuchprocess(mocker):
    """Test CPU tracking when process doesn't exist."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mocker.patch("psutil.Process", side_effect=psutil.NoSuchProcess(99999))

    cpu = tracker._get_process_cpu(99999, include_children=False)

    assert cpu == 0.0


def test_get_process_cpu_accessdenied(mocker, mock_process):
    """Test CPU tracking when access is denied."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mock_process.cpu_percent.side_effect = psutil.AccessDenied()
    mocker.patch("psutil.Process", return_value=mock_process)

    cpu = tracker._get_process_cpu(12345, include_children=False)

    assert cpu == 0.0


def test_get_process_cpu_child_error(mocker, mock_process_with_children):
    """Test CPU tracking when a child process raises an error."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    parent, child1, child2 = mock_process_with_children

    # Make child1 raise an error
    child1.cpu_percent.side_effect = psutil.NoSuchProcess(12346)

    mocker.patch("psutil.Process", return_value=parent)

    cpu = tracker._get_process_cpu(12345, include_children=True)

    # Should still get parent (20.0) + child2 (10.0) = 30.0
    assert cpu == 30.0


def test_get_process_cpu_zombieprocess(mocker, mock_process):
    """Test CPU tracking for zombie process."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mock_process.cpu_percent.side_effect = psutil.ZombieProcess(12345)
    mocker.patch("psutil.Process", return_value=mock_process)

    cpu = tracker._get_process_cpu(12345, include_children=False)

    assert cpu == 0.0


# ============================================================================
# RAM TRACKING TESTS
# ============================================================================


def test_get_process_ram_single_process(mocker, mock_process):
    """Test RAM usage tracking for single process without children."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mocker.patch("psutil.Process", return_value=mock_process)

    ram = tracker._get_process_ram_memory(12345, include_children=False)

    # 100 MB in bytes = 104857600, converted to GB = 0.09765625
    expected = round(100 * 1024 * 1024 / (1024**3), 2)
    assert ram == expected
    mock_process.memory_info.assert_called_once()


def test_get_process_ram_with_children(mocker, mock_process_with_children):
    """Test RAM usage aggregation across process tree."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    parent, child1, child2 = mock_process_with_children

    mocker.patch("psutil.Process", return_value=parent)

    ram = tracker._get_process_ram_memory(12345, include_children=True)

    # 150 MB (parent) + 50 MB (child1) + 50 MB (child2) = 250 MB
    expected = round(250 * 1024 * 1024 / (1024**3), 2)
    assert ram == expected


def test_get_process_ram_nosuchprocess(mocker):
    """Test RAM tracking when process doesn't exist."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mocker.patch("psutil.Process", side_effect=psutil.NoSuchProcess(99999))

    ram = tracker._get_process_ram_memory(99999, include_children=False)

    assert ram == 0.0


def test_get_process_ram_accessdenied(mocker, mock_process):
    """Test RAM tracking when access is denied."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mock_process.memory_info.side_effect = psutil.AccessDenied()
    mocker.patch("psutil.Process", return_value=mock_process)

    ram = tracker._get_process_ram_memory(12345, include_children=False)

    assert ram == 0.0


def test_get_process_ram_child_error(mocker, mock_process_with_children):
    """Test RAM tracking when a child process raises an error."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    parent, child1, child2 = mock_process_with_children

    # Make child1 raise an error
    child1.memory_info.side_effect = psutil.NoSuchProcess(12346)

    mocker.patch("psutil.Process", return_value=parent)

    ram = tracker._get_process_ram_memory(12345, include_children=True)

    # Should still get parent (150 MB) + child2 (50 MB) = 200 MB
    expected = round(200 * 1024 * 1024 / (1024**3), 2)
    assert ram == expected


def test_get_process_ram_zombieprocess(mocker, mock_process):
    """Test RAM tracking for zombie process."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mock_process.memory_info.side_effect = psutil.ZombieProcess(12345)
    mocker.patch("psutil.Process", return_value=mock_process)

    ram = tracker._get_process_ram_memory(12345, include_children=False)

    assert ram == 0.0


# ============================================================================
# GPU TRACKING TESTS
# ============================================================================


def test_get_process_gpu_success(mocker, mock_nvidia_smi_output):
    """Test GPU memory tracking with successful nvidia-smi call."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    gpu = tracker._get_process_gpu_memory(12345)

    # Process 12345 uses 500 MB according to mock output
    expected = round(500 / 1024, 2)  # Convert to GB
    assert gpu == expected


def test_get_process_gpu_process_not_found(mocker, mock_nvidia_smi_output):
    """Test GPU tracking when process is not using GPU."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    gpu = tracker._get_process_gpu_memory(99999)  # PID not in output

    assert gpu == 0.0


def test_get_process_gpu_nvidia_smi_not_available(mocker):
    """Test GPU tracking when nvidia-smi is not available."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run",
        side_effect=FileNotFoundError(),
    )

    gpu = tracker._get_process_gpu_memory(12345)

    assert gpu == 0.0


def test_get_process_gpu_nvidia_smi_error(mocker):
    """Test GPU tracking when nvidia-smi returns an error."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mock_run = Mock()
    mock_run.returncode = 1
    mock_run.stdout = ""
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    gpu = tracker._get_process_gpu_memory(12345)

    assert gpu == 0.0


def test_get_process_gpu_malformed_output(mocker):
    """Test GPU tracking with malformed nvidia-smi output."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mock_run = Mock()
    mock_run.stdout = "invalid, data\n12345, not_a_number"
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    gpu = tracker._get_process_gpu_memory(12345)

    assert gpu == 0.0


def test_get_process_gpu_empty_output(mocker, mock_nvidia_smi_empty):
    """Test GPU tracking with empty nvidia-smi output."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_empty
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    gpu = tracker._get_process_gpu_memory(12345)

    assert gpu == 0.0


def test_get_process_gpu_timeout(mocker):
    """Test GPU tracking when nvidia-smi times out."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run",
        side_effect=subprocess.TimeoutExpired("nvidia-smi", 5),
    )

    gpu = tracker._get_process_gpu_memory(12345)

    assert gpu == 0.0


# ============================================================================
# PROCESS CACHE TESTS
# ============================================================================


@freeze_time("2025-01-01 12:00:00")
def test_cache_hit_within_ttl(mocker, mock_process):
    """Test that cached data is used when within TTL."""
    tracker = ProcessResourceTracker(cache_ttl=2.0)
    mocker.patch("psutil.Process", return_value=mock_process)

    # First call - should populate cache
    result1 = tracker.get_process_resources(12345, include_children=False)

    # Mock should have been called once
    assert mock_process.cpu_percent.call_count == 1

    # Second call within TTL - should use cache
    with freeze_time("2025-01-01 12:00:01"):  # 1 second later
        result2 = tracker.get_process_resources(12345, include_children=False)

    # Mock should still have been called only once
    assert mock_process.cpu_percent.call_count == 1
    assert result1 == result2


@freeze_time("2025-01-01 12:00:00")
def test_cache_miss_after_ttl(mocker, mock_process):
    """Test that cache is invalidated after TTL expires."""
    tracker = ProcessResourceTracker(cache_ttl=2.0)
    mocker.patch("psutil.Process", return_value=mock_process)

    # First call - should populate cache
    tracker.get_process_resources(12345, include_children=False)
    assert mock_process.cpu_percent.call_count == 1

    # Second call after TTL - should refresh cache
    with freeze_time("2025-01-01 12:00:03"):  # 3 seconds later (exceeds 2s TTL)
        tracker.get_process_resources(12345, include_children=False)

    # Mock should have been called twice
    assert mock_process.cpu_percent.call_count == 2


def test_cache_disabled_with_zero_ttl(mocker, mock_process):
    """Test that caching is disabled when TTL is 0."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)
    mocker.patch("psutil.Process", return_value=mock_process)

    # Multiple calls should not use cache
    tracker.get_process_resources(12345, include_children=False)
    tracker.get_process_resources(12345, include_children=False)

    # Mock should be called twice (no caching)
    assert mock_process.cpu_percent.call_count == 2


def test_cache_separate_for_different_pids(mocker):
    """Test that different PIDs have separate cache entries."""
    tracker = ProcessResourceTracker(cache_ttl=2.0)

    mock_process1 = Mock(spec=psutil.Process)
    mock_process1.pid = 12345
    mock_process1.cpu_percent.return_value = 20.0
    mock_process1.memory_info.return_value = Mock(rss=1024 * 1024 * 100)
    mock_process1.children.return_value = []
    mock_process1.is_running.return_value = True

    mock_process2 = Mock(spec=psutil.Process)
    mock_process2.pid = 67890
    mock_process2.cpu_percent.return_value = 30.0
    mock_process2.memory_info.return_value = Mock(rss=1024 * 1024 * 200)
    mock_process2.children.return_value = []
    mock_process2.is_running.return_value = True

    def process_factory(pid):
        return mock_process1 if pid == 12345 else mock_process2

    mocker.patch("psutil.Process", side_effect=process_factory)

    # Get resources for both PIDs
    result1 = tracker.get_process_resources(12345, include_children=False)
    result2 = tracker.get_process_resources(67890, include_children=False)

    # Results should be different
    assert result1["cpu"] == 20.0
    assert result2["cpu"] == 30.0


# ============================================================================
# GPU CACHE TESTS
# ============================================================================


@freeze_time("2025-01-01 12:00:00")
def test_gpu_cache_shared_across_processes(mocker, mock_nvidia_smi_output, mock_process):
    """Test that GPU cache is shared across all processes."""
    tracker = ProcessResourceTracker(cache_ttl=2.0)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    # Mock psutil.Process for both PIDs
    mocker.patch("psutil.Process", return_value=mock_process)

    # Track subprocess.run calls
    mock_subprocess = mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run", return_value=mock_run
    )

    # First call for PID 12345
    tracker.get_process_resources(12345, include_children=False)

    # Second call for PID 12346 - should use same GPU cache
    tracker.get_process_resources(12346, include_children=False)

    # nvidia-smi should only be called once (GPU cache shared)
    assert mock_subprocess.call_count == 1


@freeze_time("2025-01-01 12:00:00")
def test_gpu_cache_invalidated_after_ttl(mocker, mock_nvidia_smi_output, mock_process):
    """Test that GPU cache is invalidated after TTL expires."""
    tracker = ProcessResourceTracker(cache_ttl=2.0)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    # Mock psutil.Process
    mocker.patch("psutil.Process", return_value=mock_process)

    # Track subprocess.run calls
    mock_subprocess = mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run", return_value=mock_run
    )

    # First call
    tracker.get_process_resources(12345, include_children=False)
    assert mock_subprocess.call_count == 1

    # Second call after TTL
    with freeze_time("2025-01-01 12:00:03"):  # 3 seconds later
        tracker.get_process_resources(12345, include_children=False)

    # nvidia-smi should be called twice (cache expired)
    assert mock_subprocess.call_count == 2


# ============================================================================
# INTEGRATION TESTS
# ============================================================================


def test_get_process_resources_complete(mocker, mock_process, mock_nvidia_smi_output):
    """Test complete resource retrieval with all components."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mocker.patch("psutil.Process", return_value=mock_process)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    result = tracker.get_process_resources(12345, include_children=False)

    assert "cpu" in result
    assert "ram_memory" in result
    assert "gpu_memory" in result
    assert result["cpu"] == 25.5
    assert result["ram_memory"] > 0
    assert result["gpu_memory"] > 0


def test_get_process_resources_with_children(
    mocker, mock_process_with_children, mock_nvidia_smi_output
):
    """Test resource retrieval including child processes."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    parent, child1, child2 = mock_process_with_children
    mocker.patch("psutil.Process", return_value=parent)

    mock_run = Mock()
    mock_run.stdout = mock_nvidia_smi_output
    mock_run.returncode = 0
    mocker.patch("backend.api.process_resource_tracker.subprocess.run", return_value=mock_run)

    result = tracker.get_process_resources(12345, include_children=True)

    # CPU should be aggregated
    assert result["cpu"] == 40.0  # 20 + 10 + 10

    # RAM should be aggregated
    expected_ram = round(250 * 1024 * 1024 / (1024**3), 2)
    assert result["ram_memory"] == expected_ram


def test_get_process_resources_no_gpu(mocker, mock_process):
    """Test resource retrieval when GPU is not available."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mocker.patch("psutil.Process", return_value=mock_process)
    mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run",
        side_effect=FileNotFoundError(),
    )

    result = tracker.get_process_resources(12345, include_children=False)

    assert result["cpu"] == 25.5
    assert result["ram_memory"] > 0
    assert result["gpu_memory"] == 0.0


def test_get_process_resources_partial_failure(mocker, mock_process):
    """Test resource retrieval when some components fail."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    # CPU fails but RAM succeeds
    mock_process.cpu_percent.side_effect = psutil.AccessDenied()
    mocker.patch("psutil.Process", return_value=mock_process)
    mocker.patch(
        "backend.api.process_resource_tracker.subprocess.run",
        side_effect=FileNotFoundError(),
    )

    result = tracker.get_process_resources(12345, include_children=False)

    assert result["cpu"] == 0.0  # Failed
    assert result["ram_memory"] > 0  # Succeeded
    assert result["gpu_memory"] == 0.0  # No GPU


def test_get_process_resources_nonexistent_process(mocker):
    """Test resource retrieval for non-existent process."""
    tracker = ProcessResourceTracker(cache_ttl=0.0)

    mocker.patch("psutil.Process", side_effect=psutil.NoSuchProcess(99999))

    result = tracker.get_process_resources(99999, include_children=False)

    assert result["cpu"] == 0.0
    assert result["ram_memory"] == 0.0
    assert result["gpu_memory"] == 0.0
