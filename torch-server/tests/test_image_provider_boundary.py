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
import threading
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

import serve  # noqa: E402
import validate_runtime  # noqa: E402
from fastapi import HTTPException  # noqa: E402
from image_api import ImageRequest, generate_image  # noqa: E402
from model_manager import LoadedModel, ModelSlot, SlotState  # noqa: E402


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
        validate_runtime.validate_recipe_handshake(
            recipe,
            {
                "status": "ok",
                "protocol": 3,
                "capabilities": ["image_generation", "future_capability"],
            },
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

    def test_route_sanitizes_unexpected_lease_acquisition_failure(self):
        from contextlib import asynccontextmanager

        @asynccontextmanager
        async def failing_lease(_model_name):
            raise RuntimeError("private lease failure")
            yield  # pragma: no cover - required to define an async context manager

        async def connected():
            return False

        request = SimpleNamespace(
            app=SimpleNamespace(
                state=SimpleNamespace(model_manager=SimpleNamespace(image_lease=failing_lease))
            ),
            is_disconnected=connected,
        )
        payload = ImageRequest(
            model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
        )

        with self.assertLogs("image_api", level="ERROR") as logged:
            with self.assertRaises(HTTPException) as failed:
                asyncio.run(generate_image(payload, request))

        self.assertEqual(failed.exception.status_code, 502)
        self.assertEqual(failed.exception.detail["code"], "backend_failure")
        self.assertNotIn("private", json.dumps(failed.exception.detail))
        self.assertIsInstance(failed.exception.__cause__, RuntimeError)
        self.assertEqual(str(failed.exception.__cause__), "private lease failure")
        self.assertTrue(any(record.exc_info is not None for record in logged.records))

    def test_route_logs_generation_causes_without_leaking_them_to_clients(self):
        from contextlib import asynccontextmanager
        import torch

        out_of_memory_error = getattr(
            torch.cuda, "OutOfMemoryError", type("OutOfMemoryError", (Exception,), {})
        )

        @asynccontextmanager
        async def available_lease(_model_name):
            yield object()

        async def connected():
            return False

        request = SimpleNamespace(
            app=SimpleNamespace(
                state=SimpleNamespace(model_manager=SimpleNamespace(image_lease=available_lease))
            ),
            is_disconnected=connected,
        )
        payload = ImageRequest(
            model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
        )
        cases = (
            (RuntimeError("private generation failure"), 502, "backend_failure", "ERROR"),
            (
                out_of_memory_error("private GPU allocation failure"),
                507,
                "out_of_memory",
                "WARNING",
            ),
        )

        with patch.object(torch.cuda, "OutOfMemoryError", out_of_memory_error, create=True):
            for error, expected_status, expected_code, log_level in cases:

                async def fail_generation(*_args, failure=error):
                    raise failure

                with self.subTest(code=expected_code):
                    with patch("image_api.owned_generation", new=fail_generation):
                        with self.assertLogs("image_api", level=log_level) as logged:
                            with self.assertRaises(HTTPException) as failed:
                                asyncio.run(generate_image(payload, request))

                    self.assertEqual(failed.exception.status_code, expected_status)
                    self.assertEqual(failed.exception.detail["code"], expected_code)
                    self.assertNotIn("private", json.dumps(failed.exception.detail))
                    self.assertIs(failed.exception.__cause__, error)
                    self.assertTrue(
                        any(
                            record.exc_info and record.exc_info[1] is error
                            for record in logged.records
                        )
                    )

    def test_route_maps_lease_states_and_releases_after_backend_failure(self):
        from diffusion import FLUX2_KLEIN
        import torch

        out_of_memory_error = getattr(
            torch.cuda, "OutOfMemoryError", type("OutOfMemoryError", (Exception,), {})
        )

        class Adapter:
            steps = 8
            guidance = 0.0
            memory_policy = "fixture"

            def __init__(self):
                self.calls = 0
                self.failures = [
                    KeyError("private key detail"),
                    ValueError("private value detail"),
                    RuntimeError("Image runtime is busy"),
                    out_of_memory_error("private memory detail"),
                ]

            def generate(self, _prompt, width, height, _seed, _cancel):
                self.calls += 1
                if self.failures:
                    raise self.failures.pop(0)
                image = SimpleNamespace(size=(width, height))
                image.save = lambda output, format="PNG": output.write(b"private-png")
                return image

        async def exercise_route():
            app = serve.create_app()
            manager = app.state.model_manager
            adapter = Adapter()
            manager.slots["fixture"] = ModelSlot(
                slot_id="fixture",
                model_name="img-model",
                model_path="fixture",
                device="cpu",
                state=SlotState.READY,
                model_type=FLUX2_KLEIN,
                _loaded=LoadedModel(adapter, None, torch.device("cpu"), FLUX2_KLEIN),
            )
            manager.slots["text-fixture"] = ModelSlot(
                slot_id="text-fixture",
                model_name="text-model",
                model_path="fixture",
                device="cpu",
                state=SlotState.READY,
                model_type="safetensors",
                _loaded=LoadedModel(object(), None, torch.device("cpu"), "safetensors"),
            )
            endpoint = next(
                route.endpoint for route in app.routes if route.path == "/api/images/generate"
            )

            async def is_disconnected():
                return False

            request = SimpleNamespace(
                app=SimpleNamespace(state=SimpleNamespace(model_manager=manager)),
                is_disconnected=is_disconnected,
            )
            payload = ImageRequest(
                model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
            )
            unavailable_payload = ImageRequest(
                model_id="missing", prompt="kingfisher", width=64, height=64, seed=3
            )
            unsupported_payload = ImageRequest(
                model_id="text-model", prompt="kingfisher", width=64, height=64, seed=3
            )

            with self.assertRaises(HTTPException) as unavailable:
                await endpoint(unavailable_payload, request)
            self.assertEqual(unavailable.exception.status_code, 503)
            self.assertEqual(unavailable.exception.detail["code"], "model_unavailable")

            with self.assertRaises(HTTPException) as unsupported:
                await endpoint(unsupported_payload, request)
            self.assertEqual(unsupported.exception.status_code, 400)
            self.assertEqual(unsupported.exception.detail["code"], "unsupported_model")

            async with manager.image_lease("img-model"):
                with self.assertRaises(HTTPException) as busy:
                    await endpoint(payload, request)
            self.assertEqual(busy.exception.status_code, 409)
            self.assertEqual(busy.exception.detail["code"], "runtime_busy")

            for expected_status, expected_code in (
                (502, "backend_failure"),
                (502, "backend_failure"),
                (502, "backend_failure"),
                (507, "out_of_memory"),
            ):
                with self.subTest(code=expected_code, call=adapter.calls):
                    original_error = adapter.failures[0]
                    with self.assertLogs("image_api", level="WARNING") as logged:
                        with self.assertRaises(HTTPException) as failed:
                            await endpoint(payload, request)
                    self.assertEqual(failed.exception.status_code, expected_status)
                    self.assertEqual(failed.exception.detail["code"], expected_code)
                    self.assertNotIn("private", json.dumps(failed.exception.detail))
                    self.assertIs(failed.exception.__cause__, original_error)
                    self.assertTrue(any(record.exc_info is not None for record in logged.records))
                    self.assertFalse(manager._get_device_lock("cpu").locked())

            result = await endpoint(payload, request)
            self.assertEqual(base64.b64decode(result["png_base64"]), b"private-png")
            self.assertEqual(adapter.calls, 5)

        with patch.object(torch.cuda, "OutOfMemoryError", out_of_memory_error, create=True):
            asyncio.run(exercise_route())

    def test_route_keeps_actual_device_lock_until_worker_stops(self):
        from diffusion import FLUX2_KLEIN
        import torch

        started = threading.Event()
        release = threading.Event()

        class Adapter:
            steps = 8
            guidance = 0.0
            memory_policy = "fixture"

            def generate(self, _prompt, width, height, _seed, _cancel):
                started.set()
                release.wait()
                image = SimpleNamespace(size=(width, height))
                image.save = lambda output, format="PNG": output.write(b"private-png")
                return image

        async def exercise_route():
            app = serve.create_app()
            manager = app.state.model_manager
            manager.slots["fixture"] = ModelSlot(
                slot_id="fixture",
                model_name="img-model",
                model_path="fixture",
                device="cpu",
                state=SlotState.READY,
                model_type=FLUX2_KLEIN,
                _loaded=LoadedModel(Adapter(), None, torch.device("cpu"), FLUX2_KLEIN),
            )
            endpoint = next(
                route.endpoint for route in app.routes if route.path == "/api/images/generate"
            )

            async def is_disconnected():
                return False

            request = SimpleNamespace(
                app=SimpleNamespace(state=SimpleNamespace(model_manager=manager)),
                is_disconnected=is_disconnected,
            )
            payload = ImageRequest(
                model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
            )
            task = asyncio.create_task(endpoint(payload, request))
            try:
                self.assertTrue(await asyncio.to_thread(started.wait, 3))
                self.assertTrue(manager._get_device_lock("cpu").locked())
                self.assertFalse(task.done())
                with self.assertRaises(HTTPException) as busy:
                    await endpoint(payload, request)
                self.assertEqual(busy.exception.status_code, 409)
                self.assertEqual(busy.exception.detail["code"], "runtime_busy")
                self.assertTrue(manager._get_device_lock("cpu").locked())
            finally:
                release.set()
                first = await asyncio.wait_for(task, 4)
            self.assertEqual(base64.b64decode(first["png_base64"]), b"private-png")
            self.assertFalse(manager._get_device_lock("cpu").locked())
            second = await endpoint(payload, request)
            self.assertEqual(base64.b64decode(second["png_base64"]), b"private-png")

        asyncio.run(exercise_route())

    def test_route_waits_for_blocked_worker_before_mapping_delayed_oom(self):
        from diffusion import FLUX2_KLEIN
        import torch

        out_of_memory_error = getattr(
            torch.cuda, "OutOfMemoryError", type("OutOfMemoryError", (Exception,), {})
        )
        started = threading.Event()
        release = threading.Event()

        class Adapter:
            steps = 8
            guidance = 0.0
            memory_policy = "fixture"

            def __init__(self):
                self.calls = 0

            def generate(self, _prompt, width, height, _seed, _cancel):
                self.calls += 1
                if self.calls == 1:
                    started.set()
                    release.wait()
                    raise out_of_memory_error("private memory detail")
                image = SimpleNamespace(size=(width, height))
                image.save = lambda output, format="PNG": output.write(b"private-png")
                return image

        async def exercise_route():
            app = serve.create_app()
            manager = app.state.model_manager
            adapter = Adapter()
            manager.slots["fixture"] = ModelSlot(
                slot_id="fixture",
                model_name="img-model",
                model_path="fixture",
                device="cpu",
                state=SlotState.READY,
                model_type=FLUX2_KLEIN,
                _loaded=LoadedModel(adapter, None, torch.device("cpu"), FLUX2_KLEIN),
            )
            endpoint = next(
                route.endpoint for route in app.routes if route.path == "/api/images/generate"
            )

            async def is_disconnected():
                return False

            request = SimpleNamespace(
                app=SimpleNamespace(state=SimpleNamespace(model_manager=manager)),
                is_disconnected=is_disconnected,
            )
            payload = ImageRequest(
                model_id="img-model", prompt="kingfisher", width=64, height=64, seed=3
            )
            task = asyncio.create_task(endpoint(payload, request))
            delayed_oom = None
            try:
                self.assertTrue(await asyncio.to_thread(started.wait, 3))
                self.assertTrue(manager._get_device_lock("cpu").locked())
                self.assertFalse(task.done())
                with self.assertRaises(HTTPException) as busy:
                    await endpoint(payload, request)
                self.assertEqual(busy.exception.status_code, 409)
                self.assertEqual(busy.exception.detail["code"], "runtime_busy")
                self.assertTrue(manager._get_device_lock("cpu").locked())
                self.assertEqual(adapter.calls, 1)
            finally:
                release.set()
                try:
                    await asyncio.wait_for(task, 4)
                except HTTPException as error:
                    delayed_oom = error

            self.assertIsNotNone(delayed_oom)
            self.assertEqual(delayed_oom.status_code, 507)
            self.assertEqual(delayed_oom.detail["code"], "out_of_memory")
            self.assertNotIn("private", json.dumps(delayed_oom.detail))
            self.assertFalse(manager._get_device_lock("cpu").locked())
            self.assertEqual(adapter.calls, 1)

            result = await endpoint(payload, request)
            self.assertEqual(base64.b64decode(result["png_base64"]), b"private-png")
            self.assertEqual(adapter.calls, 2)
            self.assertFalse(manager._get_device_lock("cpu").locked())

        with patch.object(torch.cuda, "OutOfMemoryError", out_of_memory_error, create=True):
            asyncio.run(exercise_route())


if __name__ == "__main__":
    unittest.main()
