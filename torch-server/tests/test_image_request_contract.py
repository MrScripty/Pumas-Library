"""Private image provider request contract: explicit model_id and dimensions.

`model_id`, `width` and `height` are required with no defaults and no
allowlist. The removed OpenAI-shaped fields (`model`, `n`,
`response_format`, `size`) are rejected, not ignored. `owned_generation`
returns the private provider result, never the public OpenAI shape.
"""

import asyncio
import base64
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


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

from image_api import MAX_PNG_BYTES, ImageRequest, owned_generation  # noqa: E402


def _request():
    async def connected():
        return False

    return SimpleNamespace(is_disconnected=connected)


def _image(size, content=b"fake-png-bytes"):
    image = SimpleNamespace(size=size)
    image.save = lambda output, format="PNG": output.write(content)
    return image


def _adapter(generate, steps=8, guidance=0, memory_policy="fixture"):
    return SimpleNamespace(
        generate=generate, steps=steps, guidance=guidance, memory_policy=memory_policy
    )


class ImageRequestDimensionsTests(unittest.TestCase):
    def test_explicit_dimensions_reach_adapter_unchanged(self):
        seen = {}

        def generate(prompt, width, height, seed, cancel):
            seen.update(prompt=prompt, width=width, height=height, seed=seed)
            self.assertFalse(cancel.is_set())
            return _image((1280, 720))

        payload = ImageRequest(
            model_id="fixture", prompt="kingfisher", width=1280, height=720, seed=7
        )

        async def run():
            result = await owned_generation(_adapter(generate), payload, _request())
            self.assertEqual(
                set(result),
                {"png_base64", "seed", "steps", "guidance", "memory_policy", "duration_seconds"},
            )
            self.assertNotIn("data", result)
            self.assertEqual(base64.b64decode(result["png_base64"]), b"fake-png-bytes")
            self.assertEqual(result["seed"], 7)
            self.assertEqual(
                (result["steps"], result["guidance"], result["memory_policy"]),
                (8, 0, "fixture"),
            )
            self.assertGreaterEqual(result["duration_seconds"], 0)

        asyncio.run(run())
        self.assertEqual(
            (seen["width"], seen["height"], seen["prompt"], seen["seed"]),
            (1280, 720, "kingfisher", 7),
        )

    def test_absent_seed_is_randomized_within_u32(self):
        seen = {}

        def generate(_prompt, _width, _height, seed, _cancel):
            seen["seed"] = seed
            return _image((64, 64))

        payload = ImageRequest(model_id="fixture", prompt="kingfisher", width=64, height=64)

        async def run():
            with patch("secrets.randbits", return_value=12345) as randbits:
                result = await owned_generation(_adapter(generate), payload, _request())
            randbits.assert_called_once_with(32)
            self.assertEqual(result["seed"], 12345)
            return result

        asyncio.run(run())
        self.assertEqual(seen["seed"], 12345)

    def test_removed_openai_fields_are_rejected_not_ignored(self):
        with self.assertRaises(ValidationError):
            ImageRequest(model_id="fixture", prompt="kingfisher", size="1024x1024")
        with self.assertRaises(ValidationError):
            ImageRequest(model_id="fixture", prompt="kingfisher", width=64, height=64, n=1)
        with self.assertRaises(ValidationError):
            ImageRequest(
                model_id="fixture",
                prompt="kingfisher",
                width=64,
                height=64,
                response_format="b64_json",
            )
        with self.assertRaises(ValidationError):
            ImageRequest(model="fixture", prompt="kingfisher", width=64, height=64)

    def test_numeric_strings_are_rejected_under_strict_typing(self):
        for field in ("width", "height"):
            with self.subTest(field=field):
                dimensions = {"width": 1280, "height": 720, field: "1280"}
                with self.assertRaises(ValidationError):
                    ImageRequest(model_id="fixture", prompt="kingfisher", **dimensions)

    def test_missing_or_non_positive_dimensions_fail(self):
        with self.assertRaises(ValidationError):
            ImageRequest(model_id="fixture", prompt="kingfisher", height=720)
        with self.assertRaises(ValidationError):
            ImageRequest(model_id="fixture", prompt="kingfisher", width=1280)
        for width, height in ((0, 720), (1280, 0), (-1, 720), (1280, -1)):
            with self.subTest(width=width, height=height):
                with self.assertRaises(ValidationError):
                    ImageRequest(
                        model_id="fixture", prompt="kingfisher", width=width, height=height
                    )

    def test_blank_model_id_or_prompt_fails(self):
        for kwargs in (
            {"model_id": "", "prompt": "kingfisher"},
            {"model_id": "   ", "prompt": "kingfisher"},
            {"model_id": "m" * 257, "prompt": "kingfisher"},
            {"model_id": "fixture", "prompt": ""},
            {"model_id": "fixture", "prompt": "   "},
            {"model_id": "fixture", "prompt": "p" * 4001},
        ):
            with self.subTest(kwargs={k: v[:8] for k, v in kwargs.items()}):
                with self.assertRaises(ValidationError):
                    ImageRequest(width=64, height=64, **kwargs)

    def test_seed_bounds(self):
        for seed in (0, 4294967295):
            with self.subTest(seed=seed):
                payload = ImageRequest(
                    model_id="fixture",
                    prompt="kingfisher",
                    width=64,
                    height=64,
                    seed=seed,
                )
                self.assertEqual(payload.seed, seed)
        for seed in (-1, 4294967296, "7"):
            with self.subTest(seed=seed):
                with self.assertRaises(ValidationError):
                    ImageRequest(
                        model_id="fixture",
                        prompt="kingfisher",
                        width=64,
                        height=64,
                        seed=seed,
                    )

    def test_backend_dimension_mismatch_is_not_false_success(self):
        from fastapi import HTTPException

        def generate(_prompt, _width, _height, _seed, _cancel):
            return _image((640, 480))

        payload = ImageRequest(model_id="fixture", prompt="kingfisher", width=1280, height=720)

        async def run():
            with self.assertRaises(HTTPException) as caught:
                await owned_generation(_adapter(generate), payload, _request())
            self.assertEqual(caught.exception.status_code, 502)
            self.assertEqual(caught.exception.detail["code"], "invalid_backend_response")

        asyncio.run(run())

    def test_oversized_png_is_rejected(self):
        from fastapi import HTTPException

        def generate(_prompt, _width, _height, _seed, _cancel):
            return _image((64, 64), content=b"x" * (MAX_PNG_BYTES + 1))

        payload = ImageRequest(model_id="fixture", prompt="kingfisher", width=64, height=64)

        async def run():
            with self.assertRaises(HTTPException) as caught:
                await owned_generation(_adapter(generate), payload, _request())
            self.assertEqual(caught.exception.status_code, 502)
            self.assertEqual(caught.exception.detail["code"], "invalid_backend_response")

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
