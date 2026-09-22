"""Private image provider boundary: /api mounting and health handshake.

The public ``/v1/images/generations`` route is gateway-owned and must not
be served here. The provider operation lives at ``POST
/api/images/generate``, ``/v1/models`` stays OpenAI-shaped, and ``/health``
must match the staged runtime recipe, including the image capability.
"""

import asyncio
import base64
import json
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

import serve  # noqa: E402
import validate_runtime  # noqa: E402
from image_api import ImageRequest, generate_image  # noqa: E402


class ImageProviderBoundaryTests(unittest.TestCase):
    def test_image_operation_is_private_under_api(self):
        app = serve.create_app()
        paths = [route.path for route in app.routes]

        self.assertIn("/api/images/generate", paths)
        self.assertNotIn("/v1/images/generations", paths)
        self.assertTrue(all(not path.startswith("/v1/images") for path in paths))

    def test_v1_models_stays_mounted(self):
        app = serve.create_app()
        paths = [route.path for route in app.routes]

        self.assertIn("/v1/models", paths)
        self.assertIn("/v1/chat/completions", paths)
        self.assertIn("/v1/completions", paths)

    def test_health_handshake_matches_runtime_recipe(self):
        recipe = json.loads((TORCH_SERVER_ROOT / "runtime" / "runtime.json").read_text())
        app = serve.create_app()
        endpoint = next(route.endpoint for route in app.routes if route.path == "/health")
        health = asyncio.run(endpoint())

        self.assertEqual(health["status"], "ok")
        self.assertEqual(health["protocol"], recipe["protocol"])
        self.assertEqual(health["capabilities"], recipe["capabilities"])
        self.assertEqual(recipe["protocol"], 3)
        self.assertEqual(recipe["recipe_id"], "torch-runtime-0.1.6")

    def test_runtime_qualification_rejects_recipe_sidecar_drift(self):
        recipe = {"protocol": 3, "capabilities": ["image_generation"]}
        validate_runtime.validate_recipe_handshake(
            recipe,
            {"status": "ok", "protocol": 3, "capabilities": ["image_generation"]},
        )
        for health in (
            {"status": "ok", "protocol": 2, "capabilities": ["image_generation"]},
            {"status": "ok", "protocol": 3, "capabilities": []},
            {"status": "starting", "protocol": 3, "capabilities": ["image_generation"]},
        ):
            with self.subTest(health=health), self.assertRaises(RuntimeError):
                validate_runtime.validate_recipe_handshake(recipe, health)

    def test_generate_image_leases_by_model_id(self):
        seen = {}

        def generate(prompt, width, height, seed, cancel):
            seen.update(prompt=prompt, width=width, height=height, seed=seed)
            image = SimpleNamespace(size=(width, height))
            image.save = lambda output, format="PNG": output.write(b"private-png")
            return image

        adapter = SimpleNamespace(generate=generate, steps=8, guidance=0.0, memory_policy="fixture")

        from contextlib import asynccontextmanager

        @asynccontextmanager
        async def lease(model_name):
            seen["lease"] = model_name
            yield adapter

        async def connected():
            return False

        request = SimpleNamespace(
            app=SimpleNamespace(
                state=SimpleNamespace(model_manager=SimpleNamespace(image_lease=lease))
            ),
            is_disconnected=connected,
        )
        payload = ImageRequest(
            model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
        )

        result = asyncio.run(generate_image(payload, request))

        self.assertEqual(seen["lease"], "img-model")
        self.assertEqual(seen["width"], 64)
        self.assertEqual(base64.b64decode(result["png_base64"]), b"private-png")
        self.assertEqual(result["seed"], 3)


if __name__ == "__main__":
    unittest.main()
