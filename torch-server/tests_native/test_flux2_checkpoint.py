"""Scaled FP8 fixture evidence; actual Klein image acceptance is separate."""

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import torch  # noqa: E402
from safetensors.torch import save_file  # noqa: E402
from flux2 import load_scaled_fp8  # noqa: E402


class ScaledFP8Tests(unittest.TestCase):
    def test_applies_weight_scale_and_keeps_unquantized_values(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.safetensors"
            save_file(
                {
                    "layer.weight": torch.tensor([[2.0, -4.0]]).to(torch.float8_e4m3fn),
                    "layer.weight_scale": torch.tensor(0.25),
                    "layer.input_scale": torch.tensor(0.5),
                    "other.weight": torch.tensor([0.75], dtype=torch.bfloat16),
                },
                path,
                metadata={
                    "_quantization_metadata": json.dumps(
                        {"format_version": "1.0", "layers": {"layer": {"format": "float8_e4m3fn"}}}
                    )
                },
            )
            result = load_scaled_fp8(path)
            self.assertEqual(set(result), {"layer.weight", "other.weight"})
            torch.testing.assert_close(
                result["layer.weight"], torch.tensor([[0.5, -1.0]], dtype=torch.bfloat16)
            )
            self.assertEqual(result["other.weight"].item(), 0.75)

    def test_missing_or_invalid_scale_is_rejected_before_conversion(self):
        for scale in [None, torch.tensor(float("nan")), torch.tensor(0.0), torch.ones(2)]:
            with self.subTest(scale=scale), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "fixture.safetensors"
                state = {
                    "layer.weight": torch.ones(2).to(torch.float8_e4m3fn),
                    "layer.input_scale": torch.tensor(1.0),
                }
                if scale is not None:
                    state["layer.weight_scale"] = scale
                save_file(
                    state,
                    path,
                    metadata={
                        "_quantization_metadata": json.dumps(
                            {
                                "format_version": "1.0",
                                "layers": {"layer": {"format": "float8_e4m3fn"}},
                            }
                        )
                    },
                )
                with self.assertRaises(ValueError):
                    load_scaled_fp8(path)
