"""Private audio transcription operation: mounting, refusals and leasing."""

import asyncio
import base64
import sys
import unittest
from contextlib import asynccontextmanager
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from test_validation_and_app import (  # noqa: E402
    _install_optional_dependency_stubs as _install_dependency_stubs,
)

_install_dependency_stubs()

import serve  # noqa: E402
from audio_api import TranscriptionRequest, transcribe_audio  # noqa: E402
from fastapi import HTTPException  # noqa: E402
from model_manager import LoadedModel, ModelManager, ModelSlot, SlotConflict, SlotState  # noqa: E402
from test_model_manager import _FakeDeviceManager  # noqa: E402


def pcm(samples=1600):
    return {
        "encoding": "pcm_s16le",
        "sample_rate_hz": 16000,
        "channels": 1,
        "sample_count": samples,
        "data_base64": base64.b64encode(b"\x00\x00" * samples).decode("ascii"),
    }


def request(lease, disconnected=False):
    async def is_disconnected():
        return disconnected

    return SimpleNamespace(
        app=SimpleNamespace(
            state=SimpleNamespace(model_manager=SimpleNamespace(audio_lease=lease))
        ),
        is_disconnected=is_disconnected,
    )


def loaded():
    return SimpleNamespace(model="model", tokenizer="processor")


@asynccontextmanager
async def ready_lease(_model_id):
    yield loaded()


class AudioRouteTests(unittest.TestCase):
    def test_route_is_private_and_advertised(self):
        app = serve.create_app()
        self.assertIn("/api/audio/transcriptions", [route.path for route in app.routes])
        self.assertIn("audio_transcription", serve.TORCH_CAPABILITIES)

    def test_transcribes_with_the_leased_model(self):
        seen = {}

        def fake_transcribe(model, processor, data, language, cancel):
            seen.update(model=model, processor=processor, bytes=len(data), language=language)
            return "ask not", "stop"

        payload = TranscriptionRequest(model_id="asr", audio=pcm(), language="en")
        with patch("loaders.cohere_asr_loader.transcribe", fake_transcribe):
            result = asyncio.run(transcribe_audio(payload, request(ready_lease)))
        self.assertEqual(result, {"text": "ask not", "finish_reason": "stop"})
        self.assertEqual(
            seen, {"model": "model", "processor": "processor", "bytes": 3200, "language": "en"}
        )

    def test_refusals_carry_codes_and_reasons(self):
        @asynccontextmanager
        async def missing(_):
            raise KeyError("asr")
            yield  # pragma: no cover

        @asynccontextmanager
        async def wrong(_):
            raise ValueError("not audio")
            yield  # pragma: no cover

        @asynccontextmanager
        async def busy(_):
            raise SlotConflict("runtime_busy", "Audio runtime is busy")
            yield  # pragma: no cover

        cases = (
            (
                TranscriptionRequest(model_id="asr", audio=pcm(), language="xx"),
                ready_lease,
                400,
                "invalid_request",
            ),
            (
                TranscriptionRequest(model_id="asr", audio={"encoding": "mp3"}),
                ready_lease,
                400,
                "invalid_request",
            ),
            (TranscriptionRequest(model_id="asr", audio=pcm()), missing, 503, "model_unavailable"),
            (TranscriptionRequest(model_id="asr", audio=pcm()), wrong, 400, "unsupported_model"),
            (TranscriptionRequest(model_id="asr", audio=pcm()), busy, 409, "runtime_busy"),
        )
        for payload, lease, status, code in cases:
            with self.subTest(code=code, status=status), self.assertRaises(HTTPException) as failed:
                asyncio.run(transcribe_audio(payload, request(lease)))
            self.assertEqual(failed.exception.status_code, status)
            self.assertEqual(failed.exception.detail["code"], code)
            self.assertTrue(failed.exception.detail["message"])

    def test_disconnect_cancels_and_discards_the_late_result(self):
        def slow(model, processor, data, language, cancel):
            cancel.wait(5)
            return "late", "stop"

        payload = TranscriptionRequest(model_id="asr", audio=pcm())
        with patch("loaders.cohere_asr_loader.transcribe", slow):
            with self.assertRaises(HTTPException) as failed:
                asyncio.run(transcribe_audio(payload, request(ready_lease, disconnected=True)))
        self.assertEqual(failed.exception.status_code, 499)


class AudioLeaseTests(unittest.IsolatedAsyncioTestCase):
    def manager_with(self, model_type):
        manager = ModelManager(device_manager=_FakeDeviceManager())
        slot = ModelSlot(
            "s1", "asr", "/models/asr", "cpu", state=SlotState.READY, model_type=model_type
        )
        slot._loaded = LoadedModel("model", "processor", "cpu", model_type)
        manager.slots[slot.slot_id] = slot
        return manager

    async def test_lease_admits_only_ready_cohere_slots_and_refuses_queued_work(self):
        manager = self.manager_with("cohere-asr")
        async with manager.audio_lease("asr") as held:
            self.assertEqual(held.tokenizer, "processor")
            with self.assertRaises(SlotConflict):
                async with manager.audio_lease("asr"):
                    pass
        with self.assertRaises(KeyError):
            async with manager.audio_lease("other"):
                pass
        with self.assertRaises(ValueError):
            async with self.manager_with("text-generation").audio_lease("asr"):
                pass
        with self.assertRaises(ValueError):
            async with self.manager_with("cohere-asr").text_lease("asr"):
                pass


if __name__ == "__main__":
    unittest.main()
