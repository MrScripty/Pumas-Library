"""
Additional unit tests for ProcessManager to achieve 95%+ coverage.

Focuses on edge cases and error paths not covered by existing tests.
"""

import subprocess
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

import pytest

from backend.api.process_manager import ProcessManager


class TestProcessManagerEdgeCases:
    """Test edge cases and error paths in ProcessManager"""

    def test_detect_processes_empty_line_in_ps_output(self, mocker, tmp_path):
        """Test that empty lines in ps output are skipped."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess with empty lines
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(
            stdout="12345 python main.py comfyui\n\n67890 python comfyui/main.py\n  \n",
            returncode=0,
        )

        processes = manager._detect_comfyui_processes()

        # Should find 2 processes, skipping empty lines
        assert len(processes) == 2

    def test_detect_processes_malformed_line_single_part(self, mocker, tmp_path):
        """Test that ps lines with only PID (no cmdline) are skipped."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess with malformed line (only PID, no space or cmdline)
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(stdout="12345\n67890 python comfyui/main.py\n", returncode=0)

        processes = manager._detect_comfyui_processes()

        # Should only find the valid one
        assert len(processes) == 1
        assert processes[0]["pid"] == 67890

    def test_detect_processes_invalid_pid_string(self, mocker, tmp_path):
        """Test that non-numeric PIDs in ps output are skipped."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess with invalid PID
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(
            stdout="abc python main.py\n12345 python comfyui/main.py\n", returncode=0
        )

        processes = manager._detect_comfyui_processes()

        # Should only find the valid one
        assert len(processes) == 1
        assert processes[0]["pid"] == 12345

    def test_detect_processes_non_comfyui_process(self, mocker, tmp_path):
        """Test that processes without ComfyUI indicators are filtered out."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess with non-ComfyUI process
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(
            stdout="12345 python some_other_script.py\n67890 python main.py\n",
            returncode=0,
        )

        processes = manager._detect_comfyui_processes()

        # Should not find any processes (no 'comfyui' or 'ComfyUI Server' in cmdline)
        assert len(processes) == 0

    def test_detect_processes_with_title_in_cmdline(self, mocker, tmp_path):
        """Test detection of processes with 'ComfyUI Server' in title."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess with process having ComfyUI Server in title
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(
            stdout="12345 python -c 'setproctitle(\"ComfyUI Server v1.0\")'\n",
            returncode=0,
        )

        processes = manager._detect_comfyui_processes()

        # Should find the process by title
        assert len(processes) == 1
        assert processes[0]["pid"] == 12345
        assert processes[0]["source"] == "process_scan"

    def test_detect_processes_infers_tag_from_path(self, mocker, tmp_path):
        """Test that version tag is inferred from process command line path."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Create mock version manager with version paths
        mock_vm = Mock()
        version_path = tmp_path / "versions" / "v0.1.0"
        mock_vm.get_installed_versions.return_value = ["v0.1.0"]
        mock_vm.get_version_path.return_value = version_path
        manager.version_manager = mock_vm

        # Mock subprocess with process running from version path
        mock_ps = mocker.patch("subprocess.run")
        mock_ps.return_value = Mock(
            stdout=f"12345 python {version_path}/main.py --comfyui\n", returncode=0
        )

        processes = manager._detect_comfyui_processes()

        # Should find the process and infer tag
        assert len(processes) == 1
        assert processes[0]["pid"] == 12345
        assert processes[0]["tag"] == "v0.1.0"

    def test_stop_comfyui_brave_browser_found(self, mocker, tmp_path):
        """Test stopping ComfyUI kills Brave browser process."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock pgrep finding Brave processes
        mock_pgrep = mocker.patch("subprocess.run")

        def pgrep_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="99999\n88888\n", returncode=0)
            elif cmd[0] == "ps":
                return Mock(stdout="12345 python comfyui/main.py\n", returncode=0)
            return Mock(stdout="", returncode=1)

        mock_pgrep.side_effect = pgrep_side_effect

        # Mock os.kill to track calls
        mock_kill = mocker.patch("os.kill")

        result = manager.stop_comfyui()

        # Should kill both Brave processes (SIGKILL=9) and ComfyUI process (SIGTERM=15, SIGKILL=9)
        assert result is True
        kill_calls = mock_kill.call_args_list
        # At least 2 Brave processes killed + 1 ComfyUI process
        assert len(kill_calls) >= 3

    def test_stop_comfyui_brave_process_not_found_error(self, mocker, tmp_path):
        """Test that ProcessLookupError during Brave kill is handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock pgrep finding Brave process
        mock_pgrep = mocker.patch("subprocess.run")

        def pgrep_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="99999\n", returncode=0)
            elif cmd[0] == "ps":
                return Mock(stdout="12345 python comfyui/main.py\n", returncode=0)
            return Mock(stdout="", returncode=1)

        mock_pgrep.side_effect = pgrep_side_effect

        # Mock os.kill to raise ProcessLookupError for Brave, succeed for ComfyUI
        def kill_side_effect(pid, sig):
            if pid == 99999:
                raise ProcessLookupError("Process not found")
            # Let ComfyUI kill succeed

        mock_kill = mocker.patch("os.kill", side_effect=kill_side_effect)

        result = manager.stop_comfyui()

        # Should still succeed (Brave error is ignored)
        assert result is True

    def test_stop_comfyui_process_already_dead_sigkill(self, mocker, tmp_path):
        """Test that ProcessLookupError during SIGKILL is handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock no Brave processes
        mock_subprocess = mocker.patch("subprocess.run")

        def subprocess_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="", returncode=1)
            elif cmd[0] == "ps":
                return Mock(stdout="12345 python comfyui/main.py\n", returncode=0)
            elif cmd[0] == "pkill":
                return Mock(returncode=0)
            return Mock(stdout="", returncode=1)

        mock_subprocess.side_effect = subprocess_side_effect

        # Mock os.kill: SIGTERM succeeds, SIGKILL raises ProcessLookupError (already dead)
        def kill_side_effect(pid, sig):
            if sig == 9:  # SIGKILL
                raise ProcessLookupError("Already dead")
            # SIGTERM succeeds

        mocker.patch("os.kill", side_effect=kill_side_effect)

        result = manager.stop_comfyui()

        # Should succeed (process killed by SIGTERM)
        assert result is True

    def test_stop_comfyui_pid_file_cleanup_error(self, mocker, tmp_path):
        """Test that OSError during PID file cleanup is handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Create PID file
        pid_file = tmp_path / "comfyui.pid"
        pid_file.write_text("12345")

        # Mock os.kill to succeed (process exists)
        mocker.patch("os.kill", return_value=None)

        # Mock subprocess for no Brave, ps finds process
        mock_subprocess = mocker.patch("subprocess.run")

        def subprocess_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="", returncode=1)
            elif cmd[0] == "ps":
                return Mock(stdout="", returncode=0)
            return Mock(stdout="", returncode=1)

        mock_subprocess.side_effect = subprocess_side_effect

        # Mock Path.unlink to raise OSError
        mock_unlink = mocker.patch("pathlib.Path.unlink", side_effect=OSError("Permission denied"))

        result = manager.stop_comfyui()

        # Should succeed despite PID file cleanup error
        assert result is True

    def test_stop_comfyui_fallback_pkill_used(self, mocker, tmp_path):
        """Test that pkill fallback is used when no processes detected."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess: no Brave, no processes in ps
        mock_subprocess = mocker.patch("subprocess.run")

        def subprocess_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="", returncode=1)
            elif cmd[0] == "ps":
                return Mock(stdout="", returncode=0)
            elif cmd[0] == "pkill":
                return Mock(returncode=0)
            return Mock(stdout="", returncode=1)

        mock_subprocess.side_effect = subprocess_side_effect

        result = manager.stop_comfyui()

        # Should use pkill fallback and succeed
        assert result is True

    def test_stop_comfyui_all_methods_fail(self, mocker, tmp_path):
        """Test that False is returned when all stop methods fail."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        # Mock subprocess: no Brave, no processes, pkill fails
        mock_subprocess = mocker.patch("subprocess.run")

        def subprocess_side_effect(cmd, *args, **kwargs):
            if cmd[0] == "pgrep":
                return Mock(stdout="", returncode=1)
            elif cmd[0] == "ps":
                return Mock(stdout="", returncode=0)
            elif cmd[0] == "pkill":
                raise subprocess.SubprocessError("pkill failed")
            return Mock(stdout="", returncode=1)

        mock_subprocess.side_effect = subprocess_side_effect

        result = manager.stop_comfyui()

        # Should return False when all methods fail
        assert result is False


class TestProcessManagerLaunchEdgeCases:
    """Test launch_comfyui edge cases"""

    def test_launch_comfyui_version_manager_exception(self, mocker, tmp_path):
        """Test that exceptions from version_manager.launch_version are handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        mock_vm = Mock()
        mock_vm.get_active_version.return_value = "v0.1.0"
        mock_vm.launch_version.side_effect = RuntimeError("Launch failed")
        manager.version_manager = mock_vm

        result = manager.launch_comfyui()

        # Should catch exception and return failure
        assert result["success"] is False
        assert "Launch failed" in result["error"]
        assert manager.last_launch_error == "Launch failed"

    def test_launch_comfyui_type_error(self, mocker, tmp_path):
        """Test that TypeError exceptions are caught."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        mock_vm = Mock()
        mock_vm.get_active_version.side_effect = TypeError("Type mismatch")
        manager.version_manager = mock_vm

        result = manager.launch_comfyui()

        # Should catch TypeError and return failure
        assert result["success"] is False
        assert "Type mismatch" in result["error"]


class TestProcessDetectionErrorHandling:
    """Test error handling in _get_known_version_paths and _detect_comfyui_processes"""

    def test_get_known_version_paths_type_error(self, mocker, tmp_path):
        """Test that TypeError in get_installed_versions is handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        mock_vm = Mock()
        mock_vm.get_installed_versions.side_effect = TypeError("Type error")
        manager.version_manager = mock_vm

        paths = manager._get_known_version_paths()

        # Should return empty dict on error
        assert paths == {}

    def test_get_known_version_paths_value_error(self, mocker, tmp_path):
        """Test that ValueError in get_version_path is handled."""
        manager = ProcessManager(comfyui_dir=tmp_path)

        mock_vm = Mock()
        mock_vm.get_installed_versions.return_value = ["v0.1.0"]
        mock_vm.get_version_path.side_effect = ValueError("Invalid version")
        manager.version_manager = mock_vm

        paths = manager._get_known_version_paths()

        # Should return empty dict on error
        assert paths == {}
