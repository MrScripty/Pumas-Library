"""Real transport dependencies with controlled worker failures; no GPU claim."""

import asyncio
import base64
import io
from contextlib import asynccontextmanager
from pathlib import Path
import sys
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from fastapi import HTTPException  # noqa: E402
from PIL import Image  # noqa: E402
import torch  # noqa: E402
from diffusion import GenerationCancelled  # noqa: E402
from image_api import ImageRequest, generate_image  # noqa: E402


class ImageFailureTests(unittest.IsolatedAsyncioTestCase):
    async def test_worker_failure_releases_lease_and_allows_explicit_next_request(self):
        for error, status, code in (
            (torch.cuda.OutOfMemoryError("fixture"), 507, "out_of_memory"),
            (RuntimeError("fixture"), 502, "backend_failure"),
        ):
            with self.subTest(code=code):
                adapter = SimpleNamespace(steps=8, guidance=0, memory_policy="fixture")
                held = False

                @asynccontextmanager
                async def lease(_model):
                    nonlocal held
                    self.assertFalse(held)
                    held = True
                    try:
                        yield adapter
                    finally:
                        held = False

                async def connected():
                    return False

                request = SimpleNamespace(
                    app=SimpleNamespace(
                        state=SimpleNamespace(model_manager=SimpleNamespace(image_lease=lease))
                    ),
                    is_disconnected=connected,
                )

                def fail(*_args):
                    raise error

                adapter.generate = fail
                payload = ImageRequest(model_id="fixture", prompt="test", width=512, height=512)
                with self.assertRaises(HTTPException) as caught:
                    await generate_image(payload, request)
                self.assertEqual(
                    (caught.exception.status_code, caught.exception.detail["code"]), (status, code)
                )
                self.assertFalse(held)
                adapter.generate = lambda *_args: Image.new("RGB", (512, 512))
                result = await generate_image(payload, request)
                raw = base64.b64decode(result["png_base64"])
                self.assertEqual(Image.open(io.BytesIO(raw)).size, (512, 512))
                self.assertFalse(held)

    async def test_deadline_keeps_lease_until_cancelled_worker_has_stopped(self):
        from image_api import owned_generation

        stopped = threading.Event()

        def generate(_prompt, _width, _height, _seed, cancel):
            self.assertTrue(cancel.wait(3))
            stopped.set()
            raise GenerationCancelled()

        async def connected():
            return False

        with patch("image_api.GENERATION_DEADLINE_SECONDS", 0):
            with self.assertRaises(HTTPException) as caught:
                await asyncio.wait_for(
                    owned_generation(
                        SimpleNamespace(generate=generate),
                        ImageRequest(model_id="fixture", prompt="test", width=512, height=512),
                        SimpleNamespace(is_disconnected=connected),
                    ),
                    4,
                )
        self.assertTrue(stopped.is_set())
        self.assertEqual(caught.exception.status_code, 504)
        self.assertEqual(caught.exception.detail["code"], "deadline_exceeded")
