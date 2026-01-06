"""
Extended unit tests for ProcessManager.

Additional tests for process detection, lifecycle, and edge cases.
These tests should be merged with test_process_manager.py.
"""

import subprocess
from unittest.mock import MagicMock, Mock, patch

import psutil
import pytest

from backend.api.process_manager import ProcessManager
from backend.api.process_resource_tracker import ProcessResourceTracker

# ============================================================================
# PROCESS DETECTION TESTS (Extended)
# ============================================================================


def test_detect_comfyui_processes_no_processes(mocker, tmp_path):
    """Test process detection when no ComfyUI processes are running."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Mock subprocess to return empty process list
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout="", returncode=0)

    processes = manager._detect_comfyui_processes()

    assert processes == []


def test_detect_comfyui_processes_from_pid_file(mocker, tmp_path):
    """Test process detection from PID file."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Create PID file
    pid_file = tmp_path / "comfyui.pid"
    pid_file.write_text("12345")

    # Mock os.kill to simulate process exists
    mocker.patch("os.kill", return_value=None)

    # Mock subprocess to return empty (so PID file is only source)
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout="", returncode=0)

    processes = manager._detect_comfyui_processes()

    assert len(processes) == 1
    assert processes[0]["pid"] == 12345
    assert processes[0]["source"] == "pid_file"


def test_detect_comfyui_processes_stale_pid_file(mocker, tmp_path):
    """Test that stale PID files are skipped."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Create PID file with non-existent PID
    pid_file = tmp_path / "comfyui.pid"
    pid_file.write_text("99999")

    # Mock os.kill to raise ProcessLookupError (process doesn't exist)
    mocker.patch("os.kill", side_effect=ProcessLookupError())

    # Mock subprocess
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout="", returncode=0)

    processes = manager._detect_comfyui_processes()

    assert processes == []


def test_detect_comfyui_processes_from_process_scan(mocker, tmp_path):
    """Test process detection from ps command."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Mock subprocess to return process with "ComfyUI Server" in cmdline
    ps_output = "12345 python main.py comfyui server\n"
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout=ps_output, returncode=0)

    processes = manager._detect_comfyui_processes()

    assert len(processes) == 1
    assert processes[0]["pid"] == 12345
    assert processes[0]["source"] == "process_scan"


def test_detect_comfyui_processes_deduplication(mocker, tmp_path):
    """Test that duplicate PIDs from different sources are deduplicated."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Create PID file
    pid_file = tmp_path / "comfyui.pid"
    pid_file.write_text("12345")

    # Mock os.kill for PID file check
    mocker.patch("os.kill", return_value=None)

    # Mock subprocess to return same PID
    ps_output = "12345 python main.py comfyui server\n"
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout=ps_output, returncode=0)

    processes = manager._detect_comfyui_processes()

    # Should only have one entry despite being found via both methods
    assert len(processes) == 1
    assert processes[0]["pid"] == 12345
    assert processes[0]["source"] == "pid_file"  # PID file takes precedence


def test_detect_comfyui_processes_with_version_manager(mocker, tmp_path):
    """Test process detection with version manager providing version paths."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_installed_versions.return_value = ["v0.5.0", "v0.6.0"]
    mock_version_mgr.get_version_path.side_effect = lambda tag: tmp_path / tag

    manager = ProcessManager(comfyui_dir=tmp_path, version_manager=mock_version_mgr)

    # Create version-specific PID file
    version_dir = tmp_path / "v0.5.0"
    version_dir.mkdir()
    pid_file = version_dir / "comfyui.pid"
    pid_file.write_text("12345")

    # Mock os.kill
    mocker.patch("os.kill", return_value=None)

    # Mock subprocess
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout="", returncode=0)

    processes = manager._detect_comfyui_processes()

    assert len(processes) == 1
    assert processes[0]["pid"] == 12345
    assert processes[0]["tag"] == "v0.5.0"


def test_detect_comfyui_processes_invalid_pid_file(mocker, tmp_path):
    """Test handling of invalid PID file content."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Create PID file with invalid content
    pid_file = tmp_path / "comfyui.pid"
    pid_file.write_text("not_a_number")

    # Mock subprocess
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.return_value = Mock(stdout="", returncode=0)

    processes = manager._detect_comfyui_processes()

    assert processes == []


def test_detect_comfyui_processes_subprocess_error(mocker, tmp_path):
    """Test handling of subprocess errors during process scanning."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    # Mock subprocess to raise exception
    mock_ps = mocker.patch("subprocess.run")
    mock_ps.side_effect = subprocess.TimeoutExpired("ps", 3)

    processes = manager._detect_comfyui_processes()

    # Should return empty list on error, not crash
    assert processes == []


# ============================================================================
# is_comfyui_running TESTS (Extended)
# ============================================================================


def test_is_comfyui_running_true(mocker):
    """Test is_comfyui_running returns True when processes found."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[{"pid": 12345}])

    assert manager.is_comfyui_running() is True


def test_is_comfyui_running_false(mocker):
    """Test is_comfyui_running returns False when no processes found."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[])

    assert manager.is_comfyui_running() is False


def test_is_comfyui_running_handles_exception(mocker):
    """Test is_comfyui_running returns False on exception."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(
        manager, "_detect_comfyui_processes", side_effect=OSError("Permission denied")
    )

    assert manager.is_comfyui_running() is False


# ============================================================================
# stop_comfyui TESTS (Extended)
# ============================================================================


def test_stop_comfyui_kills_process(mocker):
    """Test that stop_comfyui kills detected processes."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(
        manager,
        "_detect_comfyui_processes",
        return_value=[{"pid": 12345, "pid_file": "/fake/path/comfyui.pid"}],
    )

    mock_kill = mocker.patch("os.kill")
    mock_unlink = mocker.patch("pathlib.Path.unlink")

    # Mock subprocess for Brave browser check
    mocker.patch("subprocess.run", return_value=Mock(returncode=1, stdout=""))

    result = manager.stop_comfyui()

    assert result is True
    # Should try SIGTERM (15) and SIGKILL (9)
    assert mock_kill.call_count >= 1


def test_stop_comfyui_no_processes(mocker):
    """Test stop_comfyui when no processes are running."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[])

    # Mock pkill fallback
    mock_pkill = mocker.patch("subprocess.run", return_value=Mock(returncode=0))

    result = manager.stop_comfyui()

    # Should attempt fallback pkill
    assert mock_pkill.called


def test_stop_comfyui_kills_brave_browser(mocker):
    """Test that stop_comfyui kills Brave browser instances."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[])

    # Mock pgrep to find Brave process
    mock_subprocess = mocker.patch("subprocess.run")
    mock_subprocess.side_effect = [
        Mock(returncode=0, stdout="99999\n"),  # pgrep finds Brave
        Mock(returncode=0),  # pkill fallback
    ]

    mock_kill = mocker.patch("os.kill")

    manager.stop_comfyui()

    # Should kill Brave PID 99999
    mock_kill.assert_any_call(99999, 9)


def test_stop_comfyui_multiple_processes(mocker):
    """Test stopping multiple ComfyUI processes."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(
        manager,
        "_detect_comfyui_processes",
        return_value=[
            {"pid": 12345, "pid_file": "/fake/path/v1/comfyui.pid"},
            {"pid": 67890, "pid_file": "/fake/path/v2/comfyui.pid"},
        ],
    )

    mock_kill = mocker.patch("os.kill")
    mocker.patch("pathlib.Path.unlink")
    mocker.patch("subprocess.run", return_value=Mock(returncode=1, stdout=""))

    result = manager.stop_comfyui()

    assert result is True
    # Should kill both processes
    assert mock_kill.call_count >= 2


def test_stop_comfyui_handles_process_already_dead(mocker):
    """Test stop_comfyui handles processes that are already dead."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    mocker.patch.object(manager, "_detect_comfyui_processes", return_value=[{"pid": 12345}])

    # Mock os.kill to raise ProcessLookupError (process already dead)
    mock_kill = mocker.patch("os.kill", side_effect=ProcessLookupError())
    mocker.patch("subprocess.run", return_value=Mock(returncode=1, stdout=""))

    result = manager.stop_comfyui()

    # Should not fail even if process is already dead
    assert mock_kill.called


# ============================================================================
# launch_comfyui TESTS (Extended)
# ============================================================================


def test_launch_comfyui_success(mocker):
    """Test successful ComfyUI launch."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.5.0"
    mock_version_mgr.launch_version.return_value = (
        True,  # success
        Mock(),  # process
        "/fake/log.txt",  # log_path
        None,  # error_msg
        True,  # ready
    )

    manager = ProcessManager(comfyui_dir="/fake/path", version_manager=mock_version_mgr)

    result = manager.launch_comfyui()

    assert result["success"] is True
    assert result["log_path"] == "/fake/log.txt"
    assert result["ready"] is True
    assert manager.last_launch_log == "/fake/log.txt"
    assert manager.last_launch_error is None


def test_launch_comfyui_no_active_version(mocker):
    """Test launch failure when no active version selected."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = None

    manager = ProcessManager(comfyui_dir="/fake/path", version_manager=mock_version_mgr)

    result = manager.launch_comfyui()

    assert result["success"] is False
    assert "No active version selected" in result["error"]


def test_launch_comfyui_launch_failure(mocker):
    """Test launch failure with error message."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.return_value = "v0.5.0"
    mock_version_mgr.launch_version.return_value = (
        False,  # success
        None,  # process
        "/fake/log.txt",  # log_path
        "Failed to start",  # error_msg
        False,  # ready
    )

    manager = ProcessManager(comfyui_dir="/fake/path", version_manager=mock_version_mgr)

    result = manager.launch_comfyui()

    assert result["success"] is False
    assert result["error"] == "Failed to start"
    assert manager.last_launch_error == "Failed to start"


def test_launch_comfyui_no_version_manager():
    """Test launch when version manager is not configured."""
    manager = ProcessManager(comfyui_dir="/fake/path")

    result = manager.launch_comfyui()

    assert result["success"] is False
    assert "No active version" in result["error"]


def test_launch_comfyui_exception_handling(mocker):
    """Test launch handles exceptions gracefully."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_active_version.side_effect = RuntimeError("Version error")

    manager = ProcessManager(comfyui_dir="/fake/path", version_manager=mock_version_mgr)

    result = manager.launch_comfyui()

    assert result["success"] is False
    assert "Version error" in result["error"]


# ============================================================================
# VERSION PATH MAPPING TESTS
# ============================================================================


def test_get_known_version_paths_with_versions(mocker, tmp_path):
    """Test _get_known_version_paths returns installed version paths."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_installed_versions.return_value = ["v0.5.0", "v0.6.0"]
    mock_version_mgr.get_version_path.side_effect = lambda tag: tmp_path / tag

    manager = ProcessManager(comfyui_dir=tmp_path, version_manager=mock_version_mgr)

    paths = manager._get_known_version_paths()

    assert len(paths) == 2
    assert paths["v0.5.0"] == tmp_path / "v0.5.0"
    assert paths["v0.6.0"] == tmp_path / "v0.6.0"


def test_get_known_version_paths_no_version_manager(tmp_path):
    """Test _get_known_version_paths without version manager."""
    manager = ProcessManager(comfyui_dir=tmp_path)

    paths = manager._get_known_version_paths()

    assert paths == {}


def test_get_known_version_paths_handles_error(mocker, tmp_path):
    """Test _get_known_version_paths handles errors gracefully."""
    mock_version_mgr = Mock()
    mock_version_mgr.get_installed_versions.side_effect = OSError("Disk error")

    manager = ProcessManager(comfyui_dir=tmp_path, version_manager=mock_version_mgr)

    paths = manager._get_known_version_paths()

    # Should return empty dict on error, not crash
    assert paths == {}
