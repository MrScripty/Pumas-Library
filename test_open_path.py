#!/usr/bin/env python3
"""
Unit tests for file manager path opening helpers (Phase 6.2.5e).
"""

import os
import sys
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

    def test_open_path_runs_platform_command(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            target = Path(tmpdir)

            with mock.patch("shutil.which", return_value="/usr/bin/fake-opener"), \
                    mock.patch("subprocess.run") as mock_run:
                mock_run.return_value = mock.Mock(returncode=0)

                result = self.api.open_path(str(target))

                self.assertTrue(result["success"])
                called_cmd = mock_run.call_args[0][0]

                if sys.platform.startswith("darwin"):
                    self.assertEqual(called_cmd[0], "open")
                elif os.name == "nt":
                    self.assertEqual(called_cmd[0], "explorer")
                else:
                    self.assertEqual(called_cmd[0], "xdg-open")


if __name__ == "__main__":
    import unittest
    unittest.main()
