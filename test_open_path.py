#!/usr/bin/env python3
"""
Unit tests for file manager path opening helpers (Phase 6.2.5e).
"""

import tempfile
from pathlib import Path
from unittest import TestCase, mock

from backend.api import ComfyUISetupAPI


class OpenPathTests(TestCase):
    def setUp(self):
        self.api = ComfyUISetupAPI()

    def test_open_path_rejects_missing(self):
        result = self.api.open_path("/path/that/does/not/exist")
        self.assertFalse(result["success"])
        self.assertIn("does not exist", result["error"])

    def test_open_path_uses_click_launch(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            target = Path(tmpdir)

            with mock.patch("backend.file_opener.click.launch", return_value=True) as mock_launch:
                result = self.api.open_path(str(target))

                self.assertTrue(result["success"])
                self.assertTrue(mock_launch.called)

                args, kwargs = mock_launch.call_args
                self.assertEqual(args[0], str(target))
                self.assertFalse(kwargs.get("locate"))
                self.assertFalse(kwargs.get("wait"))


if __name__ == "__main__":
    import unittest
    unittest.main()
