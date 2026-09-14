"""Run with the qualified native runtime; no checkpoint or GPU allocation required."""

import inspect
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from diffusers.models.transformers.transformer_z_image import ZImageTransformer2DModel
from nunchaku_compat import PumasNunchakuZImageTransformer


class NativeNunchakuBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.model = object.__new__(PumasNunchakuZImageTransformer)
        self.register = Mock()
        self.unregister = Mock()
        object.__setattr__(self.model, "register_rope_hook", self.register)
        object.__setattr__(self.model, "unregister_rope_hook", self.unregister)

    def test_patch_sizes_do_not_become_controlnet_inputs(self):
        signature = inspect.signature(ZImageTransformer2DModel.forward)

        def observe(*args, **kwargs):
            bound = signature.bind(*args, **kwargs)
            bound.apply_defaults()
            self.assertIsNone(bound.arguments["controlnet_block_samples"])
            self.assertIsNone(bound.arguments["siglip_feats"])
            self.assertIs(bound.arguments["return_dict"], False)
            self.assertEqual(bound.arguments["patch_size"], 2)
            self.assertEqual(bound.arguments["f_patch_size"], 1)
            return ("output",)

        with patch.object(ZImageTransformer2DModel, "forward", observe):
            result = self.model.forward([], None, [], return_dict=False)
        self.assertEqual(result, ("output",))
        self.register.assert_called_once()
        self.unregister.assert_called_once()

    def test_failed_forward_removes_rope_hooks(self):
        with patch.object(
            ZImageTransformer2DModel, "forward", side_effect=RuntimeError("worker failed")
        ):
            with self.assertRaisesRegex(RuntimeError, "worker failed"):
                self.model.forward([], None, [], return_dict=False)
        self.unregister.assert_called_once()
