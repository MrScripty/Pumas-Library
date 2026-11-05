"""Sidecar boundary hardening: text leases, auth, path confinement, admission.

Fake models and tokenizers stand in for transformers; no GPU or model files.
"""

import asyncio
import contextvars
import json
import os
import subprocess
import sys
import tempfile
import threading
import types
import unittest
from concurrent.futures import ThreadPoolExecutor
from contextlib import nullcontext
from functools import partial
from pathlib import Path
from unittest.mock import Mock, patch

TORCH_SERVER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TORCH_SERVER_ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from test_validation_and_app import _install_optional_dependency_stubs  # noqa: E402

_install_optional_dependency_stubs()

import control_api  # noqa: E402
import diffusion  # noqa: E402
import image_api  # noqa: E402
import openai_api  # noqa: E402
import psutil  # noqa: E402
import serve  # noqa: E402
import validation  # noqa: E402
from fastapi import HTTPException  # noqa: E402
from model_manager import LoadedModel, ModelManager, ModelSlot, SlotConflict, SlotState  # noqa: E402
from test_model_manager import _FakeDeviceManager  # noqa: E402


class _FakeOutOfMemory(RuntimeError):
    pass


def _fake_torch():
    return types.SimpleNamespace(
        no_grad=nullcontext,
        full=lambda shape, value, dtype=None, device=None: [value] * shape[0],
        bool=bool,
        cuda=types.SimpleNamespace(OutOfMemoryError=_FakeOutOfMemory, empty_cache=Mock()),
    )


def _fake_transformers():
    module = types.ModuleType("transformers")
    module.StoppingCriteria = object
    module.StoppingCriteriaList = list
    return module


class _Encoding(dict):
    def to(self, _device):
        return self


class _Tokenizer:
    def __init__(self, prompt_tokens):
        self.prompt_tokens = prompt_tokens

    def __call__(self, _prompt, return_tensors):
        ids = types.SimpleNamespace(shape=(1, self.prompt_tokens), device="cpu")
        return _Encoding(input_ids=ids)

    def decode(self, tokens, skip_special_tokens):
        return "".join(tokens)


class _ControlledModel:
    """Blocks in generate until released; honors stopping criteria like transformers."""

    def __init__(self, *, positions=None, error=None):
        self.config = types.SimpleNamespace(max_position_embeddings=positions)
        self.started = threading.Event()
        self.release = threading.Event()
        self.stopped_by_criteria = threading.Event()
        self.error = error
        self.calls = 0

    def generate(self, input_ids, max_new_tokens, stopping_criteria, **_kwargs):
        self.calls += 1
        self.started.set()
        if self.error is not None:
            raise self.error
        while not self.release.wait(0.01):
            if any(criterion(input_ids, None)[0] for criterion in stopping_criteria):
                self.stopped_by_criteria.set()
                break
        return [["p"] * input_ids.shape[1] + ["o", "k"]]


def _manager_with_text_model(model, *, prompt_tokens=3, model_type="text-generation"):
    manager = ModelManager(_FakeDeviceManager())
    manager.slots["text"] = ModelSlot(
        slot_id="text",
        model_name="local-text",
        model_path="/fixture",
        device="cpu",
        state=SlotState.READY,
        model_type=model_type,
        _loaded=LoadedModel(model, _Tokenizer(prompt_tokens), "cpu", model_type),
    )
    return manager


def _request(manager, disconnected=lambda: False):
    async def is_disconnected():
        return disconnected()

    return types.SimpleNamespace(
        app=types.SimpleNamespace(state=types.SimpleNamespace(model_manager=manager)),
        is_disconnected=is_disconnected,
    )


def _completion(max_tokens=2):
    return openai_api.CompletionRequest(model="local-text", prompt="Hi", max_tokens=max_tokens)


class TextGenerationLeaseTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        executor = self.enterContext(ThreadPoolExecutor(max_workers=2))

        async def owned_to_thread(func, *args, **kwargs):
            context = contextvars.copy_context()
            work = partial(context.run, partial(func, *args, **kwargs))
            return await asyncio.get_running_loop().run_in_executor(executor, work)

        self.enterContext(patch.object(asyncio, "to_thread", new=owned_to_thread))
        self.torch = _fake_torch()
        self.enterContext(patch.object(openai_api, "torch", self.torch))
        self.enterContext(patch.dict(sys.modules, {"transformers": _fake_transformers()}))

    async def test_health_answers_and_queued_work_is_refused_during_long_generation(self):
        model = _ControlledModel()
        manager = _manager_with_text_model(model)
        request = _request(manager)
        health = next(r.endpoint for r in serve.create_app().routes if r.path == "/health")

        task = asyncio.create_task(openai_api.completions(_completion(), request))
        try:
            self.assertTrue(
                await asyncio.get_running_loop().run_in_executor(None, model.started.wait, 3)
            )
            reply = await asyncio.wait_for(health(), 1)
            self.assertEqual(reply["status"], "ok")
            self.assertFalse(task.done())
            with self.assertRaises(HTTPException) as busy:
                await openai_api.completions(_completion(), request)
            self.assertEqual(busy.exception.status_code, 409)
            self.assertEqual(busy.exception.detail["code"], "runtime_busy")
            with self.assertRaisesRegex(SlotConflict, "busy"):
                await manager.unload("text")
        finally:
            model.release.set()
        response = await task
        self.assertEqual(response.choices[0].text, "ok")
        self.assertFalse(manager._get_device_lock("cpu").locked())

    async def test_disconnect_stops_decoding_and_holds_lease_until_worker_returns(self):
        model = _ControlledModel()
        manager = _manager_with_text_model(model)
        disconnected = threading.Event()
        request = _request(manager, disconnected.is_set)

        task = asyncio.create_task(openai_api.completions(_completion(), request))
        self.assertTrue(
            await asyncio.get_running_loop().run_in_executor(None, model.started.wait, 3)
        )
        disconnected.set()
        with self.assertRaises(HTTPException) as caught:
            await asyncio.wait_for(task, 3)
        self.assertEqual(caught.exception.status_code, 499)
        self.assertEqual(caught.exception.detail["code"], "cancelled")
        self.assertTrue(model.stopped_by_criteria.is_set())
        self.assertFalse(manager._get_device_lock("cpu").locked())

    async def test_owner_cancellation_waits_for_the_worker_before_releasing(self):
        model = _ControlledModel()
        manager = _manager_with_text_model(model)
        task = asyncio.create_task(openai_api.completions(_completion(), _request(manager)))
        self.assertTrue(
            await asyncio.get_running_loop().run_in_executor(None, model.started.wait, 3)
        )
        task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await asyncio.wait_for(task, 3)
        self.assertTrue(model.stopped_by_criteria.is_set())
        self.assertFalse(manager._get_device_lock("cpu").locked())

    async def test_out_of_memory_frees_cache_and_refuses_with_reason(self):
        model = _ControlledModel(error=_FakeOutOfMemory("CUDA out of memory at /private/path"))
        manager = _manager_with_text_model(model)
        with self.assertRaises(HTTPException) as caught:
            await openai_api.completions(_completion(), _request(manager))
        self.assertEqual(caught.exception.status_code, 507)
        self.assertEqual(caught.exception.detail["code"], "out_of_memory")
        self.assertNotIn("/private/path", caught.exception.detail["message"])
        self.torch.cuda.empty_cache.assert_called_once_with()

    async def test_unexpected_failure_keeps_raw_text_out_of_the_response(self):
        model = _ControlledModel(error=RuntimeError("secret /home/user/model"))
        manager = _manager_with_text_model(model)
        with self.assertRaises(HTTPException) as caught:
            await openai_api.completions(_completion(), _request(manager))
        self.assertEqual(caught.exception.status_code, 500)
        self.assertEqual(caught.exception.detail["code"], "backend_failure")
        self.assertNotIn("secret", caught.exception.detail["message"])

    async def test_prompt_plus_output_beyond_model_positions_is_refused_before_generation(self):
        model = _ControlledModel(positions=8)
        manager = _manager_with_text_model(model, prompt_tokens=6)
        with self.assertRaises(HTTPException) as caught:
            await openai_api.completions(_completion(max_tokens=3), _request(manager))
        self.assertEqual(caught.exception.status_code, 400)
        self.assertEqual(caught.exception.detail["code"], "context_length_exceeded")
        message = caught.exception.detail["message"]
        self.assertIn("6 tokens", message)
        self.assertIn("8-token context", message)
        self.assertIn("max_position_embeddings", message)
        self.assertEqual(model.calls, 0)

    async def test_request_that_fits_the_model_context_generates(self):
        model = _ControlledModel(positions=8)
        model.release.set()
        manager = _manager_with_text_model(model, prompt_tokens=6)
        response = await openai_api.completions(_completion(max_tokens=2), _request(manager))
        self.assertEqual(response.choices[0].text, "ok")

    async def test_non_text_models_and_unknown_names_are_refused_with_reasons(self):
        manager = _manager_with_text_model(_ControlledModel(), model_type="flux2-klein-9b-kv-fp8")
        with self.assertRaises(HTTPException) as unsupported:
            await openai_api.completions(_completion(), _request(manager))
        self.assertEqual(unsupported.exception.status_code, 400)
        self.assertEqual(unsupported.exception.detail["code"], "unsupported_model")

        request = openai_api.CompletionRequest(model="missing", prompt="Hi")
        with self.assertRaises(HTTPException) as missing:
            await openai_api.completions(request, _request(manager))
        self.assertEqual(missing.exception.status_code, 404)

    def test_max_tokens_has_no_flat_ceiling(self):
        request = openai_api.CompletionRequest(model="m", prompt="p", max_tokens=1_000_000)
        self.assertEqual(request.max_tokens, 1_000_000)


class TokenAuthTests(unittest.IsolatedAsyncioTestCase):
    def middleware(self):
        captured = []
        app = types.SimpleNamespace(
            middleware=lambda _kind: lambda handler: captured.append(handler) or handler
        )
        serve.install_token_auth(app, "launch-token")
        return captured[0]

    async def test_unauthenticated_request_is_refused_with_a_reason(self):
        middleware = self.middleware()
        call_next = Mock()
        for headers in ({}, {"x-pumas-torch-token": "wrong"}):
            request = types.SimpleNamespace(
                url=types.SimpleNamespace(path="/api/slots"), headers=headers
            )
            response = await middleware(request, call_next)
            self.assertEqual(response.status_code, 401)
        call_next.assert_not_called()

    async def test_token_header_and_health_pass(self):
        middleware = self.middleware()

        async def call_next(_request):
            return "passed"

        for path, headers in (
            ("/api/slots", {"x-pumas-torch-token": "launch-token"}),
            ("/v1/completions", {"authorization": "Bearer launch-token"}),
            ("/health", {}),
        ):
            request = types.SimpleNamespace(url=types.SimpleNamespace(path=path), headers=headers)
            self.assertEqual(await middleware(request, call_next), "passed")


class ControlBoundaryTests(unittest.TestCase):
    def setUp(self):
        os.environ.pop(validation.MODEL_ROOTS_ENV, None)
        self.addCleanup(os.environ.pop, validation.MODEL_ROOTS_ENV, None)

    def test_component_paths_outside_model_roots_are_refused(self):
        with tempfile.TemporaryDirectory() as root, tempfile.TemporaryDirectory() as outside:
            os.environ[validation.MODEL_ROOTS_ENV] = root
            model = Path(root) / "checkpoint.safetensors"
            model.write_bytes(b"")
            for field in ("pipeline_path", "vae_path"):
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, field):
                    control_api.LoadModelRequest(
                        model_path=str(model),
                        model_name="image",
                        model_type="flux2-klein-9b-kv-fp8",
                        pipeline_path=outside if field == "pipeline_path" else root,
                        vae_path=outside if field == "vae_path" else None,
                    )
            accepted = control_api.LoadModelRequest(
                model_path=str(model),
                model_name="image",
                model_type="flux2-klein-9b-kv-fp8",
                pipeline_path=root,
                vae_path=str(model),
            )
            self.assertEqual(accepted.pipeline_path, str(Path(root).resolve()))

    def test_model_type_is_closed_and_components_match_the_adapter(self):
        with tempfile.TemporaryDirectory() as root:
            for options in (
                {"model_type": "arbitrary"},
                {"model_type": "nunchaku-z-image-turbo"},
                {"pipeline_path": root},
                {"model_type": "nunchaku-z-image-turbo", "pipeline_path": root, "vae_path": root},
            ):
                with self.subTest(options=options), self.assertRaises(ValueError):
                    control_api.LoadModelRequest(model_path=root, model_name="m", **options)

    def test_unload_and_configure_reject_unknown_fields(self):
        with self.assertRaises(ValueError):
            control_api.UnloadModelRequest(slot_id="a", force=True)
        with self.assertRaises(ValueError):
            control_api.ConfigureRequest(api_port=8400, unknown=True)

    def test_load_failures_map_to_stable_codes_without_raw_library_text(self):
        cases = (
            (
                SlotConflict("slot_limit_reached", "Maximum loaded models (1) reached."),
                409,
                "slot_limit_reached",
                "Maximum",
            ),
            (
                validation.ModelLoadRefusal("Pipeline component is missing: vae"),
                400,
                "model_rejected",
                "vae",
            ),
            (ValueError("Unrecognized model in /secret/path"), 400, "model_unloadable", None),
            (OSError("no file in /secret/path"), 400, "model_unloadable", None),
            (KeyError("/secret/path"), 500, "internal_error", None),
        )
        with tempfile.TemporaryDirectory() as root:
            for error, status, code, visible in cases:
                with self.subTest(code=code, error=type(error).__name__):
                    manager = types.SimpleNamespace(load=Mock(side_effect=error))

                    async def connected():
                        return False

                    request = types.SimpleNamespace(
                        app=types.SimpleNamespace(
                            state=types.SimpleNamespace(model_manager=manager)
                        ),
                        is_disconnected=connected,
                    )

                    async def load(*_args, **_kwargs):
                        raise error

                    manager.load = load
                    payload = control_api.LoadModelRequest(model_path=root, model_name="m")
                    with self.assertRaises(HTTPException) as caught:
                        asyncio.run(control_api.load_model(payload, request))
                    detail = caught.exception.detail
                    self.assertEqual((caught.exception.status_code, detail["code"]), (status, code))
                    self.assertNotIn("/secret/path", detail["message"])
                    if visible is not None:
                        self.assertIn(visible, detail["message"])


class DeviceSynchronizeTests(unittest.TestCase):
    def test_synchronize_failure_never_masks_the_original_error(self):
        cuda = types.SimpleNamespace(
            is_available=lambda: True, synchronize=Mock(side_effect=RuntimeError("sync"))
        )
        with patch.object(diffusion.torch, "cuda", cuda):
            diffusion.finish_device_work("cuda:0", RuntimeError("original"))
            with self.assertRaisesRegex(RuntimeError, "sync"):
                diffusion.finish_device_work("cuda:0", None)

    def test_non_cuda_runs_do_not_synchronize(self):
        cuda = types.SimpleNamespace(is_available=lambda: False, synchronize=Mock())
        with patch.object(diffusion.torch, "cuda", cuda):
            diffusion.finish_device_work(None, None)
        cuda.synchronize.assert_not_called()


class PipelineIndexAllowlistTests(unittest.TestCase):
    def index(self, payload):
        directory = self.enterContext(tempfile.TemporaryDirectory())
        path = Path(directory) / "model_index.json"
        path.write_text(json.dumps(payload))
        return path

    def z_image_index(self, **overrides):
        payload = {
            "_class_name": "ZImagePipeline",
            "_diffusers_version": "0.36.0.dev0",
            "scheduler": ["diffusers", "FlowMatchEulerDiscreteScheduler"],
            "text_encoder": ["transformers", "Qwen3Model"],
            "tokenizer": ["transformers", "Qwen2Tokenizer"],
            "transformer": ["diffusers", "ZImageTransformer2DModel"],
            "vae": ["diffusers", "AutoencoderKL"],
        }
        payload.update(overrides)
        return payload

    def validate(self, payload):
        diffusion.validate_pipeline_index(
            self.index(payload), diffusion.Z_IMAGE_PIPELINE_CLASS, diffusion.Z_IMAGE_COMPONENTS
        )

    def test_published_z_image_index_is_accepted(self):
        self.validate(self.z_image_index())

    def test_unlisted_library_class_or_component_is_refused(self):
        for overrides in (
            {"text_encoder": ["os", "system"]},
            {"vae": ["diffusers", "SomeOtherAutoencoder"]},
            {"safety_checker": ["transformers", "CLIPModel"]},
            {"scheduler": None},
            {"_class_name": "OtherPipeline"},
        ):
            with self.subTest(overrides=overrides):
                with self.assertRaises(validation.ModelLoadRefusal):
                    self.validate(self.z_image_index(**overrides))
        payload = self.z_image_index()
        del payload["vae"]
        with self.assertRaisesRegex(validation.ModelLoadRefusal, "missing: vae"):
            self.validate(payload)


class ImageAdmissionTests(unittest.TestCase):
    def adapter(self, **facts):
        return types.SimpleNamespace(
            max_pixels=2048 * 2048, peak_bytes_per_pixel=1024, device="cuda:0", **facts
        )

    def test_dimensions_beyond_the_adapter_limit_are_refused_with_the_limit(self):
        with self.assertRaises(HTTPException) as caught:
            image_api.admit_dimensions(self.adapter(), 4096, 2048)
        self.assertEqual(caught.exception.status_code, 400)
        self.assertEqual(caught.exception.detail["code"], "dimensions_exceed_limit")
        self.assertIn(str(2048 * 2048), caught.exception.detail["message"])
        self.assertEqual(caught.exception.detail["max_pixels"], 2048 * 2048)

    def test_pixel_budget_follows_free_device_memory(self):
        one_gib = 1024 * 1024 * 1024
        with patch.object(image_api, "_free_device_memory", return_value=one_gib):
            image_api.admit_dimensions(self.adapter(), 1024, 1024)
            with self.assertRaises(HTTPException) as caught:
                image_api.admit_dimensions(self.adapter(), 1024, 1025)
        self.assertEqual(caught.exception.status_code, 507)
        self.assertEqual(caught.exception.detail["code"], "out_of_memory")
        self.assertIn("1024 MiB is free", caught.exception.detail["message"])

    def test_adapters_declare_their_admission_facts(self):
        safetensors = types.ModuleType("safetensors")
        safetensors.safe_open = None
        with patch.dict(sys.modules, {"safetensors": sys.modules.get("safetensors", safetensors)}):
            from flux2 import Flux2Klein

        for adapter in (diffusion.NunchakuZImage, Flux2Klein):
            with self.subTest(adapter=adapter.__name__):
                self.assertGreater(adapter.max_pixels, 0)
                self.assertGreater(adapter.peak_bytes_per_pixel, 0)


class TextWeightFormatTests(unittest.TestCase):
    def test_text_models_without_safetensors_weights_are_refused_with_a_reason(self):
        from loaders import _require_safetensors_weights

        with tempfile.TemporaryDirectory() as root:
            (Path(root) / "pytorch_model.bin").write_bytes(b"")
            with self.assertRaisesRegex(validation.ModelLoadRefusal, "safetensors"):
                _require_safetensors_weights(Path(root))
            (Path(root) / "model.safetensors").write_bytes(b"")
            _require_safetensors_weights(Path(root))


@unittest.skipUnless(hasattr(psutil, "Process"), "requires psutil process inspection")
class ParentWatchdogTests(unittest.TestCase):
    def test_watchdog_stops_the_server_once_its_launcher_exits(self):
        launcher = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
        self.addCleanup(launcher.kill)
        orphaned = threading.Event()
        serve.start_parent_watchdog(launcher.pid, interval=0.02, on_orphaned=orphaned.set)
        self.assertFalse(orphaned.wait(0.2))
        launcher.kill()
        launcher.wait(5)
        self.assertTrue(orphaned.wait(5))

    def test_a_missing_launcher_stops_the_server_immediately(self):
        launcher = subprocess.Popen([sys.executable, "-c", "pass"])
        launcher.wait(5)
        orphaned = threading.Event()
        serve.start_parent_watchdog(launcher.pid, interval=0.02, on_orphaned=orphaned.set)
        self.assertTrue(orphaned.wait(5))

    def test_parent_pid_configuration_is_validated(self):
        self.addCleanup(os.environ.pop, validation.PARENT_PID_ENV, None)
        os.environ.pop(validation.PARENT_PID_ENV, None)
        self.assertIsNone(validation.configured_parent_pid())
        os.environ[validation.PARENT_PID_ENV] = "4242"
        self.assertEqual(validation.configured_parent_pid(), 4242)
        for value in ("1", "-5", "abc"):
            os.environ[validation.PARENT_PID_ENV] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                validation.configured_parent_pid()


if __name__ == "__main__":
    unittest.main()
