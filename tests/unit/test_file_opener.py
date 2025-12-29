"""
Unit tests for file manager path opening functionality.
"""

from pathlib import Path
from unittest import mock

import pytest

from backend.api import ComfyUISetupAPI


@pytest.fixture
def api():
    """Create a ComfyUISetupAPI instance for testing."""
    return ComfyUISetupAPI()


@pytest.mark.unit
class TestFileOpener:
    """Tests for file opener functionality."""

    def test_open_path_rejects_missing(self, api):
        """Test that open_path rejects non-existent paths."""
        result = api.open_path("/path/that/does/not/exist")
        assert result["success"] is False
        assert "does not exist" in result["error"]

    def test_open_path_uses_click_launch(self, api, tmp_path):
        """Test that open_path uses click.launch to open existing paths."""
        target = tmp_path

        with mock.patch("backend.file_opener.click.launch", return_value=True) as mock_launch:
            result = api.open_path(str(target))

            assert result["success"] is True
            assert mock_launch.called

            args, kwargs = mock_launch.call_args
            assert args[0] == str(target)
            assert kwargs.get("locate") is False
            assert kwargs.get("wait") is False

    def test_open_path_with_file(self, api, tmp_path):
        """Test opening a file path (not just directories)."""
        test_file = tmp_path / "test.txt"
        test_file.write_text("test content")

        with mock.patch("backend.file_opener.click.launch", return_value=True) as mock_launch:
            result = api.open_path(str(test_file))

            assert result["success"] is True
            assert mock_launch.called
