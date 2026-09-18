"""Image generation request contract: explicit numeric dimensions.

`width` and `height` are required positive ints with no defaults and no
allowlist. The removed `size` string is rejected, not ignored.
"""

import sys
import unittest
from pathlib import Path
from types import SimpleNamespace


TORCH_SERVER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TORCH_SERVER_ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))

# Reuse the canonical unit-suite dependency stubs so this module never shadows
# them with a competing partial set under unittest discovery.
from test_validation_and_app import (  # noqa: E402
    _install_optional_dependency_stubs as _install_dependency_stubs,
)

_install_dependency_stubs()

from pydantic import ValidationError  # noqa: E402

from image_api import ImageRequest, owned_generation  # noqa: E402


def _request():
    async def connected():
        return False

    return SimpleNamespace(is_disconnected=connected)


def _image(size, content=b"fake-png-bytes"):
    image = SimpleNamespace(size=size)
    image.save = lambda output, format="PNG": output.write(content)
    return image


class ImageRequestDimensionsTests(unittest.TestCase):
    def test_explicit_dimensions_reach_adapter_unchanged(self):
        import asyncio

        seen = {}

        def generate(prompt, width, height, seed, cancel):
            seen.update(prompt=prompt, width=width, height=height, seed=seed)
            self.assertFalse(cancel.is_set())
            return _image((1280, 720))

        payload = ImageRequest(model="fixture", prompt="kingfisher", width=1280, height=720, seed=7)

        async def run():
            adapter = SimpleNamespace(
                generate=generate, steps=8, guidance=0, memory_policy="fixture"
            )
            result = await owned_generation(adapter, payload, _request())
            self.assertEqual(len(result["data"]), 1)
            self.assertEqual(result["metadata"]["seed"], 7)

        asyncio.run(run())
        self.assertEqual(
            (seen["width"], seen["height"], seen["prompt"], seen["seed"]),
            (1280, 720, "kingfisher", 7),
        )

    def test_removed_size_field_is_rejected_not_ignored(self):
        with self.assertRaises(ValidationError):
            ImageRequest(model="fixture", prompt="kingfisher", size="1024x1024")

    def test_numeric_strings_are_rejected_under_strict_typing(self):
        for field in ("width", "height"):
            with self.subTest(field=field):
                dimensions = {"width": 1280, "height": 720, field: "1280"}
                with self.assertRaises(ValidationError):
                    ImageRequest(model="fixture", prompt="kingfisher", **dimensions)

    def test_missing_or_non_positive_dimensions_fail(self):
        with self.assertRaises(ValidationError):
            ImageRequest(model="fixture", prompt="kingfisher", height=720)
        with self.assertRaises(ValidationError):
            ImageRequest(model="fixture", prompt="kingfisher", width=1280)
        for width, height in ((0, 720), (1280, 0), (-1, 720), (1280, -1)):
            with self.subTest(width=width, height=height):
                with self.assertRaises(ValidationError):
                    ImageRequest(model="fixture", prompt="kingfisher", width=width, height=height)

    def test_backend_dimension_mismatch_is_not_false_success(self):
        from fastapi import HTTPException

        def generate(_prompt, _width, _height, _seed, _cancel):
            return _image((640, 480))

        payload = ImageRequest(model="fixture", prompt="kingfisher", width=1280, height=720)

        async def run():
            with self.assertRaises(HTTPException) as caught:
                await owned_generation(SimpleNamespace(generate=generate), payload, _request())
            self.assertEqual(caught.exception.status_code, 502)
            self.assertEqual(caught.exception.detail["code"], "invalid_backend_response")

        import asyncio

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
