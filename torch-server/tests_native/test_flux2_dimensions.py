"""FLUX.2 preserves requested image dimensions across Diffusers rounding."""

from pathlib import Path
import sys
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from PIL import Image  # noqa: E402
from flux2 import Flux2Klein  # noqa: E402


class Flux2DimensionTests(unittest.TestCase):
    def test_1920x1080_rounds_inference_up_then_center_crops_for_provider(self):
        seen = {}

        class Pipeline:
            def __call__(self, **kwargs):
                seen.update(kwargs)
                # Match Diffusers' current behavior when it receives a size
                # that is not divisible by 16.
                width = kwargs["width"] // 16 * 16
                height = kwargs["height"] // 16 * 16
                image = Image.new("RGB", (width, height))
                image.putpixel((0, 4), (255, 0, 0))
                image.putpixel((0, height - 5), (0, 0, 255))
                return SimpleNamespace(images=[image])

            def maybe_free_model_hooks(self):
                pass

        adapter = Flux2Klein.__new__(Flux2Klein)
        adapter.pipeline = Pipeline()

        with patch("torch.cuda.synchronize"):
            image = adapter.generate("test", 1920, 1080, 7, threading.Event())

        self.assertEqual((seen["width"], seen["height"]), (1920, 1088))
        self.assertEqual(image.size, (1920, 1080))
        self.assertEqual(image.getpixel((0, 0)), (255, 0, 0))
        self.assertEqual(image.getpixel((0, 1079)), (0, 0, 255))


if __name__ == "__main__":
    unittest.main()
