import asyncio
import importlib
import os
import sys
import tempfile
import types
import unittest
from pathlib import Path


TORCH_SERVER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TORCH_SERVER_ROOT))


def _install_optional_dependency_stubs() -> None:
    try:
        import fastapi  # noqa: F401
    except ModuleNotFoundError:
        fastapi_module = types.ModuleType("fastapi")
        responses_module = types.ModuleType("fastapi.responses")

        class HTTPException(Exception):
            def __init__(self, status_code: int, detail: str):
                super().__init__(detail)
                self.status_code = status_code
                self.detail = detail

        class Request:
            pass

        class APIRouter:
            def __init__(self):
                self.routes = []

            def get(self, path):
                return self._route(path)

            def post(self, path):
                return self._route(path)

            def _route(self, path):
                def decorator(func):
                    self.routes.append(types.SimpleNamespace(path=path, endpoint=func))
                    return func

                return decorator

        class FastAPI:
            def __init__(self, *args, **kwargs):
                self.routes = []
                self.state = types.SimpleNamespace()

            def include_router(self, router, prefix=""):
                self.routes.extend(
                    types.SimpleNamespace(path=f"{prefix}{route.path}", endpoint=route.endpoint)
                    for route in getattr(router, "routes", [])
                )

            def get(self, path):
                def decorator(func):
                    self.routes.append(types.SimpleNamespace(path=path, endpoint=func))
                    return func

                return decorator

        class StreamingResponse:
            pass

        fastapi_module.APIRouter = APIRouter
        fastapi_module.FastAPI = FastAPI
        fastapi_module.HTTPException = HTTPException
        fastapi_module.Request = Request
        responses_module.StreamingResponse = StreamingResponse
        sys.modules["fastapi"] = fastapi_module
        sys.modules["fastapi.responses"] = responses_module

    try:
        import torch  # noqa: F401
    except ModuleNotFoundError:
        torch_module = types.ModuleType("torch")

        class _Device:
            def __init__(self, value):
                self.type = str(value).split(":", maxsplit=1)[0]
                self.value = value

            def __str__(self):
                return str(self.value)

        torch_module.device = _Device
        torch_module.cuda = types.SimpleNamespace(
            is_available=lambda: False,
            device_count=lambda: 0,
            empty_cache=lambda: None,
            memory_allocated=lambda device: 0,
        )
        torch_module.backends = types.SimpleNamespace(
            mps=types.SimpleNamespace(is_available=lambda: False)
        )
        sys.modules["torch"] = torch_module

    try:
        import uvicorn  # noqa: F401
    except ModuleNotFoundError:
        uvicorn_module = types.ModuleType("uvicorn")
        uvicorn_module.run = lambda *args, **kwargs: None
        sys.modules["uvicorn"] = uvicorn_module

    try:
        import psutil  # noqa: F401
    except ModuleNotFoundError:
        psutil_module = types.ModuleType("psutil")
        psutil_module.virtual_memory = lambda: types.SimpleNamespace(
            total=16 * 1024 * 1024,
            available=8 * 1024 * 1024,
        )
        sys.modules["psutil"] = psutil_module


_install_optional_dependency_stubs()


class TorchValidationTests(unittest.TestCase):
    def setUp(self):
        os.environ.pop("PUMAS_TORCH_ALLOW_LAN", None)
        os.environ.pop("PUMAS_TORCH_MODEL_ROOTS", None)

        self.control_api = importlib.import_module("control_api")
        self.serve = importlib.import_module("serve")

    def test_load_request_canonicalizes_existing_model_path(self):
        with tempfile.TemporaryDirectory() as root:
            request = self.control_api.LoadModelRequest(
                model_path=root,
                model_name="local/test-model",
                device="CPU",
            )

        self.assertEqual(request.model_path, str(Path(root).resolve()))
        self.assertEqual(request.model_name, "local/test-model")
        self.assertEqual(request.device, "cpu")

    def test_load_request_rejects_path_outside_approved_roots(self):
        with tempfile.TemporaryDirectory() as approved_root:
            with tempfile.TemporaryDirectory() as external_root:
                os.environ["PUMAS_TORCH_MODEL_ROOTS"] = approved_root

                with self.assertRaises(ValueError):
                    self.control_api.LoadModelRequest(
                        model_path=external_root,
                        model_name="external-model",
                    )

    def test_configure_rejects_lan_without_explicit_policy(self):
        with self.assertRaises(ValueError):
            self.control_api.ConfigureRequest(host="0.0.0.0", lan_access=True)

    def test_configure_accepts_localhost_without_lan_policy(self):
        request = self.control_api.ConfigureRequest(host="localhost")

        self.assertEqual(request.host, "localhost")

    def test_configure_accepts_lan_with_explicit_policy(self):
        os.environ["PUMAS_TORCH_ALLOW_LAN"] = "1"

        request = self.control_api.ConfigureRequest(host="0.0.0.0", lan_access=True)

        self.assertEqual(request.host, "0.0.0.0")
        self.assertTrue(request.lan_access)

    def test_create_app_returns_fresh_app_instances_without_duplicate_routes(self):
        first = self.serve.create_app()
        second = self.serve.create_app()

        first_paths = [route.path for route in first.routes]
        second_paths = [route.path for route in second.routes]

        self.assertIsNot(first, second)
        self.assertIsNot(first.state.model_manager, second.state.model_manager)
        self.assertEqual(len(first_paths), len(set(first_paths)))
        self.assertEqual(first_paths, second_paths)

    def test_configure_does_not_partially_mutate_config_on_limit_rejection(self):
        class RejectingManager:
            async def set_max_loaded_models(self, max_loaded_models):
                raise RuntimeError("limit too low")

        config = {
            "host": "127.0.0.1",
            "api_port": 8400,
            "max_loaded_models": 4,
            "lan_access": False,
        }
        fake_request = types.SimpleNamespace(
            app=types.SimpleNamespace(
                state=types.SimpleNamespace(
                    config=config,
                    model_manager=RejectingManager(),
                )
            )
        )
        request = self.control_api.ConfigureRequest(host="localhost", max_loaded_models=1)

        with self.assertRaises(self.control_api.HTTPException):
            asyncio.run(self.control_api.configure(request, fake_request))

        self.assertEqual(
            config,
            {
                "host": "127.0.0.1",
                "api_port": 8400,
                "max_loaded_models": 4,
                "lan_access": False,
            },
        )


if __name__ == "__main__":
    unittest.main()
