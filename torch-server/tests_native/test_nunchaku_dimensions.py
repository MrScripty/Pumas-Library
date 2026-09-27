"""Nunchaku Z-Image preserves requested dimensions across pipeline alignment."""

from pathlib import Path
import sys
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from PIL import Image  # noqa: E402
from diffusion import NunchakuZImage  # noqa: E402


class NunchakuDimensionTests(unittest.TestCase):
    def test_1920x1080_rounds_inference_up_then_center_crops(self):
        seen = {}

        class Pipeline:
            def __call__(self, **kwargs):
                width, height = kwargs["width"], kwargs["height"]
                if width % 16 or height % 16:
                    raise ValueError("ZImagePipeline requires dimensions divisible by 16")
                seen.update(kwargs)
                image = Image.new("RGB", (width, height))
                image.putpixel((0, 4), (255, 0, 0))
                image.putpixel((0, height - 5), (0, 0, 255))
                return SimpleNamespace(images=[image])

            def maybe_free_model_hooks(self):
                pass

        adapter = NunchakuZImage.__new__(NunchakuZImage)
        adapter.pipeline = Pipeline()

        with patch("torch.cuda.synchronize"):
            image = adapter.generate("test", 1920, 1080, 7, threading.Event())

        self.assertEqual((seen["width"], seen["height"]), (1920, 1088))
        self.assertEqual(image.size, (1920, 1080))
        self.assertEqual(image.getpixel((0, 0)), (255, 0, 0))
        self.assertEqual(image.getpixel((0, 1079)), (0, 0, 255))


if __name__ == "__main__":
    unittest.main()
