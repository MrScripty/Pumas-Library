"""Optional adapter diagnostics never qualify or invalidate core Torch."""

import importlib.util
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "probe_runtime", Path(__file__).resolve().parents[1] / "probe_runtime.py"
)
probe_runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe_runtime)


class AdapterImportProbeTests(TestCase):
    def test_missing_adapter_is_scoped_to_its_feature(self):
        with patch.object(
            probe_runtime.importlib, "import_module", side_effect=ImportError("nunchaku missing")
        ):
            result = probe_runtime.adapter_import_probe(("nunchaku",), ())
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["scope"], "adapter imports")
        self.assertIn("nunchaku missing", result["error"])

    def test_imports_do_not_claim_model_execution(self):
        with patch.object(
            probe_runtime.importlib,
            "import_module",
            return_value=SimpleNamespace(ZImagePipeline=object()),
        ):
            result = probe_runtime.adapter_import_probe(
                ("diffusers",), (("diffusers", "ZImagePipeline"),)
            )
        self.assertEqual(result["status"], "inconclusive")
