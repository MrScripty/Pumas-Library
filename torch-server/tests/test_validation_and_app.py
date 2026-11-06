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
                self.middleware_handlers = []
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

            def middleware(self, middleware_type):
                def decorator(func):
                    self.middleware_handlers.append(
                        types.SimpleNamespace(type=middleware_type, endpoint=func)
                    )
                    return func

                return decorator

        class JSONResponse:
            def __init__(self, status_code, content):
                self.status_code = status_code
                self.content = content

        class StreamingResponse:
            pass

        fastapi_module.APIRouter = APIRouter
        fastapi_module.FastAPI = FastAPI
        fastapi_module.HTTPException = HTTPException
        fastapi_module.Request = Request
        responses_module.JSONResponse = JSONResponse
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
        os.environ.pop("PUMAS_TORCH_API_TOKEN", None)
        os.environ.pop("PUMAS_TORCH_MODEL_ROOTS", None)

        self.control_api = importlib.import_module("control_api")
        self.openai_api = importlib.import_module("openai_api")
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

    def test_configure_rejects_lan_without_api_token(self):
        os.environ["PUMAS_TORCH_ALLOW_LAN"] = "1"

        with self.assertRaises(ValueError):
            self.control_api.ConfigureRequest(host="0.0.0.0", lan_access=True)

    def test_configure_accepts_localhost_without_lan_policy(self):
        request = self.control_api.ConfigureRequest(host="localhost")

        self.assertEqual(request.host, "localhost")

    def test_configure_accepts_lan_with_explicit_policy(self):
        os.environ["PUMAS_TORCH_ALLOW_LAN"] = "1"
        os.environ["PUMAS_TORCH_API_TOKEN"] = "test-token"

        request = self.control_api.ConfigureRequest(host="0.0.0.0", lan_access=True)

        self.assertEqual(request.host, "0.0.0.0")
        self.assertTrue(request.lan_access)

    def test_create_app_rejects_lan_without_api_token(self):
        os.environ["PUMAS_TORCH_ALLOW_LAN"] = "1"

        with self.assertRaises(ValueError):
            self.serve.create_app(host="0.0.0.0")

    def test_create_app_installs_token_auth_when_token_configured(self):
        os.environ["PUMAS_TORCH_API_TOKEN"] = "test-token"

        app = self.serve.create_app()

        middleware_handlers = getattr(app, "middleware_handlers", [])
        if middleware_handlers:
            self.assertEqual(middleware_handlers[0].type, "http")

    def test_token_helpers_accept_header_and_bearer_tokens(self):
        validation = importlib.import_module("validation")

        self.assertEqual(validation.extract_bearer_token("Bearer test-token"), "test-token")
        self.assertIsNone(validation.extract_bearer_token("Basic test-token"))
        self.assertTrue(validation.token_matches("test-token", "test-token"))
        self.assertFalse(validation.token_matches("wrong-token", "test-token"))

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

    def test_openai_text_request_subset_accepts_only_implemented_values(self):
        chat = self.openai_api.ChatCompletionRequest(
            model="local-chat",
            messages=[
                {"role": "system", "content": "Be concise."},
                {"role": "user", "content": "Hello"},
                {"role": "assistant", "content": "Hi"},
            ],
            temperature=0.0,
            top_p=1.0,
            max_tokens=1,
        )
        completion = self.openai_api.CompletionRequest(
            model="local-text",
            prompt="Complete this",
            temperature=2.0,
            top_p=0.5,
            max_tokens=4096,
        )

        self.assertEqual(chat.stream, False)
        self.assertIsNone(chat.stop)
        self.assertEqual(completion.stream, False)
        self.assertIsNone(completion.stop)

    def test_openai_chat_response_allows_empty_assistant_output(self):
        response = self.openai_api.ChatCompletionResponse(
            id="chatcmpl-test",
            created=1,
            model="local-chat",
            choices=[
                self.openai_api.ChatChoice(
                    message=self.openai_api.AssistantMessage(
                        role="assistant",
                        content="",
                    )
                )
            ],
        )

        self.assertEqual(response.choices[0].message.content, "")
        self.assertEqual(response.choices[0].message.role, "assistant")

    def test_openai_text_request_subset_rejects_unknown_fields(self):
        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(
                model="local-text",
                prompt="Hello",
                seed=42,
            )

        with self.assertRaises(ValueError):
            self.openai_api.ChatCompletionRequest(
                model="local-chat",
                messages=[{"role": "user", "content": "Hello", "name": "caller"}],
            )

    def test_openai_text_request_subset_rejects_unsupported_roles_and_values(self):
        invalid_chat_requests = [
            {"model": "local-chat", "messages": []},
            {"model": "local-chat", "messages": [{"role": "tool", "content": "result"}]},
            {"model": "local-chat", "messages": [{"role": "user", "content": ""}]},
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "temperature": -0.1,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "temperature": 2.1,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "top_p": 0.0,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "top_p": 1.1,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "temperature": 0.0,
                "top_p": 0.5,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "max_tokens": 0,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "max_tokens": "12",
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "stream": True,
            },
            {
                "model": "local-chat",
                "messages": [{"role": "user", "content": "Hello"}],
                "stop": ["END"],
            },
        ]

        for request in invalid_chat_requests:
            with self.subTest(request=request), self.assertRaises(ValueError):
                self.openai_api.ChatCompletionRequest(**request)

        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(
                model="local-text",
                prompt="Hello",
                temperature=0.0,
                top_p=0.5,
            )

    def test_openai_text_request_subset_enforces_input_resource_bounds(self):
        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(model="", prompt="Hello")

        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(model="   ", prompt="Hello")

        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(model="m" * 257, prompt="Hello")

        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(model="local-text", prompt="")

        with self.assertRaises(ValueError):
            self.openai_api.CompletionRequest(
                model="local-text",
                prompt="p" * 1_000_001,
            )

        with self.assertRaises(ValueError):
            self.openai_api.ChatCompletionRequest(
                model="local-chat",
                messages=[{"role": "user", "content": "x"}] * 257,
            )

        with self.assertRaises(ValueError):
            self.openai_api.ChatCompletionRequest(
                model="local-chat",
                messages=[
                    {"role": "user", "content": "x" * 600_000},
                    {"role": "assistant", "content": "y" * 400_001},
                ],
            )


if __name__ == "__main__":
    unittest.main()
