"""Synthetic ownership evidence only; no runtime/model/ASR quality qualification."""

import asyncio
import base64
import contextlib
import gc
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time
import types
import unittest
from unittest.mock import Mock, patch
from uuid import uuid4
import weakref

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
# Reuse the minimal device fallback, not native model execution.
from test_model_manager import _FakeDeviceManager, _TestModelManager
from model_manager import ModelSlot, SlotState
from speech_fixtures import SyntheticArtifactAuthority
from loaders.cohere_asr_loader import COHERE_ASR, MAX_AUDIO_SAMPLES, SpeechCleanupUnconfirmed
from speech_operations import (
    MAX_ENVELOPE_BYTES,
    MAX_RECEIPTS,
    MAX_TEXT_BYTES,
    OperationRef,
    SpeechOperationError,
    SpeechOperationOwner,
    _CUSTODIANS,
)


def payload(owner, *, ref=None, request_id=None, pcm=b"\x00\x00", language="en"):
    if ref is None:
        ref = owner._manager.fixture_ref
    return {
        "runtime_instance_id": ref.runtime_instance_id,
        "slot": {"slot_id": ref.slot_id, "load_generation": ref.load_generation},
        "request_id": request_id or str(uuid4()),
        "language": language,
        "audio": {
            "encoding": "pcm_s16le",
            "sample_rate_hz": 16000,
            "channels": 1,
            "sample_count": len(pcm) // 2,
            "data_base64": base64.b64encode(pcm).decode(),
        },
    }


def encode(request):
    return json.dumps(request).encode()


class Manager(_TestModelManager):
    def __init__(self):
        authority = SyntheticArtifactAuthority()
        super().__init__(_FakeDeviceManager(), _speech_artifact_authority=authority)
        self.entered = 0
        self.exited = 0
        self.slot = ModelSlot(
            "fixture",
            "fixture",
            "/fixture",
            "cpu",
            state=SlotState.READY,
            model_type=COHERE_ASR,
            _loaded=types.SimpleNamespace(model=object(), tokenizer=object()),
        )
        self.slots[self.slot.slot_id] = self.slot
        self.fixture_ref = authority.attest_fixture(self.speech_slot_ref(self.slot.slot_id))
        self.lock = self._get_device_lock(self.slot.device)

    @property
    def loaded(self):
        return self.slot._loaded

    @loaded.setter
    def loaded(self, value):
        self.slot._loaded = value

    @contextlib.asynccontextmanager
    async def speech_lease(self, binding):
        async with super().speech_lease(binding) as loaded:
            self.entered += 1
            try:
                yield loaded
            finally:
                self.exited += 1


async def loaded_manager():
    authority = SyntheticArtifactAuthority()
    manager = _TestModelManager(_FakeDeviceManager(), _speech_artifact_authority=authority)
    slot = await manager.load("/fixture", "speech", model_type=COHERE_ASR)
    manager.fixture_ref = authority.attest_fixture(manager.speech_slot_ref(slot.slot_id))
    return manager, slot


class Gate:
    def __init__(self, result="synthetic transcript", error=None):
        self.entered = threading.Event()
        self.release = threading.Event()
        self.result = result
        self.error = error
        self.calls = 0
        self.cancel = None

    def __call__(self, model, processor, audio, language, cancel):
        self.calls += 1
        self.cancel = cancel
        self.entered.set()
        if not self.release.wait(5):
            raise RuntimeError("Synthetic fixture was not released")
        if self.error is not None:
            raise self.error
        return self.result


async def eventually(predicate):
    async with asyncio.timeout(3):
        while not predicate():
            await asyncio.sleep(0.001)


class SpeechOperationTests(unittest.IsolatedAsyncioTestCase):
    def make_owner(self, adapter=None, **kwargs):
        manager = Manager()
        gate = Gate() if adapter is None else adapter
        owner = SpeechOperationOwner(manager, adapter=gate, **kwargs)
        self.addAsyncCleanup(self.finish_owner, owner, gate)
        return owner, manager, gate

    async def finish_owner(self, owner, gate):
        if isinstance(gate, Gate):
            gate.release.set()
        result = await owner.drain(3)
        self.assertTrue(result.custody_complete, result)
        await asyncio.sleep(0)

    def assert_code(self, code, action):
        with self.assertRaises(SpeechOperationError) as result:
            action()
        self.assertEqual(result.exception.code, code)

    async def test_concurrent_duplicates_share_one_operation_and_one_worker(self):
        owner, manager, gate = self.make_owner()
        body = encode(payload(owner))

        async def admit():
            return owner.start(body)

        statuses = await asyncio.gather(*(admit() for _ in range(30)))
        self.assertEqual(len({s.operation_ref for s in statuses}), 1)
        await eventually(gate.entered.is_set)
        self.assertEqual(gate.calls, 1)
        self.assertEqual(manager.entered, 1)
        self.assertEqual(len(owner._entries), 1)
        gate.release.set()
        done = await owner.wait(statuses[0].operation_ref)
        self.assertEqual(done.state, "completed")
        self.assertEqual(done.text, "synthetic transcript")
        self.assertEqual(owner.start(body), done)

    async def test_concurrent_conflicting_requests_refuse_changed_audio_and_language(self):
        owner, _, gate = self.make_owner()
        first = payload(owner)
        second = payload(owner, request_id=first["request_id"], pcm=b"\x01\x00")
        third = payload(owner, request_id=first["request_id"], language="de")

        async def admit(request):
            return owner.start(encode(request))

        outcomes = await asyncio.gather(
            *(admit(p) for p in [first, second, third]), return_exceptions=True
        )
        self.assertNotIsInstance(outcomes[0], Exception)
        for refused in outcomes[1:]:
            self.assertEqual(refused.code, "request_conflict")
        await eventually(gate.entered.is_set)
        self.assertEqual(gate.calls, 1)

    async def test_raw_bound_and_type_precede_json_and_base64_allocation(self):
        owner, manager, _ = self.make_owner()
        with (
            patch("speech_operations.json.loads") as loads,
            patch("speech_operations.base64.b64decode") as decode,
        ):
            for body in (b"x" * (MAX_ENVELOPE_BYTES + 1), "not bytes", bytearray(b"{}")):
                self.assert_code("invalid_envelope", lambda: owner.start(body))
            loads.assert_not_called()
            decode.assert_not_called()
        self.assertEqual(manager.entered, 0)

    async def test_exact_envelope_bound_is_allowed_and_larger_is_refused(self):
        owner, _, gate = self.make_owner()
        base = encode(payload(owner))
        body = base + b" " * (MAX_ENVELOPE_BYTES - len(base))
        status = owner.start(body)
        self.assert_code("invalid_envelope", lambda: owner.start(body + b" "))
        gate.release.set()
        self.assertEqual((await owner.wait(status.operation_ref)).state, "completed")

    async def test_format_and_encoded_length_rejected_before_base64_decode(self):
        owner, _, _ = self.make_owner()
        bad_fields = [
            ("encoding", "wav"),
            ("encoding", None),
            ("sample_rate_hz", 8000),
            ("sample_rate_hz", 16000.0),
            ("channels", 2),
            ("channels", True),
            ("sample_count", 0),
            ("sample_count", -1),
            ("sample_count", True),
            ("sample_count", MAX_AUDIO_SAMPLES + 1),
            ("sample_count", 1.0),
            ("data_base64", "A" * 10000),
            ("data_base64", None),
        ]
        with patch("speech_operations.base64.b64decode") as decode:
            for field, value in bad_fields:
                request = payload(owner)
                request["audio"][field] = value
                with self.assertRaises(SpeechOperationError):
                    owner.start(encode(request))
            decode.assert_not_called()

    async def test_strict_base64_sample_count_language_and_fields(self):
        owner, _, _ = self.make_owner()
        for value in ("!!!=", "AA A", "AAAA", "AA==", "AAB=", "éAA=", "AAA\n"):
            request = payload(owner)
            request["audio"]["data_base64"] = value
            with self.assertRaises(SpeechOperationError, msg=value):
                owner.start(encode(request))
        for language in ("EN", "en-US", "", None, 1, ["en"]):
            request = payload(owner, language=language)
            self.assert_code("invalid_language", lambda: owner.start(encode(request)))
        for key in ("audio", "language", "request_id"):
            request = payload(owner)
            del request[key]
            self.assert_code("invalid_fields", lambda: owner.start(encode(request)))
        request = payload(owner)
        request["audio"]["url"] = "https://invalid.example/audio"
        self.assert_code("invalid_fields", lambda: owner.start(encode(request)))
        request = payload(owner)
        request["unknown"] = 1
        self.assert_code("invalid_fields", lambda: owner.start(encode(request)))
        self.assert_code("duplicate_field", lambda: owner.start(b'{"a":1,"a":2}'))
        self.assert_code("invalid_json", lambda: owner.start(b"\xff"))
        self.assert_code("invalid_json", lambda: owner.start(b"[" * 10000))

    async def test_request_id_requires_canonical_uuid(self):
        owner, _, _ = self.make_owner()
        for value in (None, 1, "bad", str(uuid4()).upper(), uuid4().hex):
            request = payload(owner)
            request["request_id"] = value
            self.assert_code("invalid_request_id", lambda: owner.start(encode(request)))

    async def test_maximum_pcm_and_all_languages_are_accepted(self):
        from loaders.cohere_asr_loader import LANGUAGES

        owner, _, _ = self.make_owner(adapter=lambda *args: "")
        for language in LANGUAGES:
            status = owner.start(
                encode(payload(owner, pcm=b"\x00\x00" * MAX_AUDIO_SAMPLES, language=language))
            )
            done = await owner.wait(status.operation_ref)
            self.assertEqual(done.state, "completed")
            self.assertEqual(done.text, "")

    async def test_busy_no_queue_and_device_lock_remains_held_until_thread_exits(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        self.assert_code("runtime_busy", lambda: owner.start(encode(payload(owner))))
        for _ in range(4):
            cancelled = owner.cancel(status.operation_ref)
            self.assertEqual(cancelled.state, "cancellation_requested")
        self.assertTrue(manager.lock.locked())
        self.assertEqual(manager.exited, 0)
        self.assert_code("runtime_busy", lambda: owner.start(encode(payload(owner))))
        gate.release.set()
        done = await owner.wait(status.operation_ref)
        self.assertEqual(done.state, "cancelled")
        self.assertIsNone(done.text)
        self.assertEqual(manager.exited, 1)
        self.assertFalse(manager.lock.locked())

    async def test_waiter_cancellation_and_lost_ack_do_not_cancel_or_replay_native_work(self):
        owner, manager, gate = self.make_owner()
        body = encode(payload(owner))
        status = owner.start(body)
        waiter = asyncio.create_task(owner.wait(status.operation_ref))
        await eventually(gate.entered.is_set)
        waiter.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await waiter
        self.assertFalse(gate.cancel.is_set())
        self.assertTrue(manager.lock.locked())
        self.assertEqual(owner.start(body).operation_ref, status.operation_ref)
        gate.release.set()
        self.assertEqual((await owner.wait(status.operation_ref)).state, "completed")
        self.assertEqual(gate.calls, 1)

    async def test_cancel_immediately_before_owner_starts_runs_no_worker(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        owner.cancel(status.operation_ref)
        done = await owner.wait(status.operation_ref)
        self.assertEqual(done.state, "cancelled")
        self.assertEqual(gate.calls, 0)
        self.assertEqual(manager.entered, 0)

    async def test_internal_runner_prestart_cancellation_produces_truthful_receipt(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        owner._active.runner.cancel()
        done = await owner.wait(status.operation_ref)
        self.assertEqual(done.state, "cancelled")
        self.assertEqual(manager.entered, 0)
        self.assertEqual(gate.calls, 0)

    async def test_repeated_internal_runner_cancellation_never_cancels_thread_or_releases_lease(
        self,
    ):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        for _ in range(5):
            owner._active.runner.cancel()
            await asyncio.sleep(0)
            self.assertTrue(manager.lock.locked())
            self.assertEqual(manager.exited, 0)
        gate.release.set()
        done = await owner.wait(status.operation_ref)
        self.assertEqual(done.state, "cancelled")
        self.assertIsNone(done.text)

    async def test_cancellation_after_native_return_discards_late_result(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        worker = owner._active.worker
        gate.release.set()
        # Hold the event loop until native thread exit, before its completion
        # callback can settle or release the lease.
        worker.join(3)
        self.assertFalse(worker.is_alive())
        self.assertTrue(manager.lock.locked())
        owner.cancel(status.operation_ref)
        done = await owner.wait(status.operation_ref)
        self.assertEqual(done.state, "cancelled")
        self.assertIsNone(done.text)

    async def test_cancel_after_settlement_preserves_original_completion(self):
        owner, _, _ = self.make_owner(adapter=lambda *args: "done")
        status = owner.start(encode(payload(owner)))
        done = await owner.wait(status.operation_ref)
        self.assertEqual(owner.cancel(status.operation_ref), done)
        waiter = asyncio.create_task(owner.wait(status.operation_ref))
        waiter.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await waiter
        self.assertEqual(owner.status(status.operation_ref), done)

    async def test_invalid_or_oversized_output_fails_without_truncation(self):
        for text in (
            None,
            b"bytes",
            "a" * (MAX_TEXT_BYTES + 1),
            "é" * (MAX_TEXT_BYTES // 2 + 1),
            "\ud800",
        ):
            owner, manager, _ = self.make_owner(adapter=lambda *args: text)
            status = owner.start(encode(payload(owner)))
            done = await owner.wait(status.operation_ref)
            self.assertEqual(done.state, "failed")
            self.assertIsNone(done.text)
            self.assertEqual(done.cleanup, "confirmed")
            self.assertEqual(manager.exited, 1)

    async def test_failed_receipt_does_not_retain_audio_or_exception_tracebacks(self):
        class AudioWitness:
            pass

        witnesses = []

        def fail(model, processor, audio, language, cancel):
            witness = AudioWitness()
            witness.audio = audio
            witnesses.append(weakref.ref(witness))
            raise RuntimeError("synthetic failure")

        owner, _, _ = self.make_owner(adapter=fail)
        status = owner.start(encode(payload(owner)))
        done = await owner.wait(status.operation_ref)
        await asyncio.sleep(0)
        gc.collect()
        self.assertIsNone(witnesses[0]())
        entry = owner._entries[status.operation_ref.operation_id]
        self.assertIsNone(entry.audio)
        self.assertIsNone(entry.worker)
        self.assertIsNone(entry.lease)
        self.assertIsNone(entry.runner)
        self.assertIsNone(entry.quarantine)
        self.assertEqual(done.operation_diagnostic.exception_type, "RuntimeError")
        self.assertEqual(done.operation_diagnostic.message, "Speech inference failed.")
        self.assertNotIn(owner, _CUSTODIANS)

    async def test_count_and_ttl_eviction_unknown_outcome_without_replay_guarantee(self):
        now = [10.0]
        owner, _, _ = self.make_owner(
            adapter=lambda *args: "done", max_receipts=2, receipt_ttl=10, clock=lambda: now[0]
        )
        requests = []
        refs = []
        for _ in range(3):
            body = encode(payload(owner))
            requests.append(body)
            status = owner.start(body)
            refs.append(status.operation_ref)
            await owner.wait(status.operation_ref)
        self.assertEqual(len(owner._settled), 2)
        self.assertEqual(len(owner._entries), 2)
        self.assert_code("unknown_or_expired_operation", lambda: owner.status(refs[0]))
        self.assert_code("unknown_or_expired_operation", lambda: owner.cancel(refs[0]))
        now[0] = 20.0
        self.assert_code("unknown_or_expired_operation", lambda: owner.status(refs[2]))
        self.assertEqual(len(owner._entries), 0)
        replay = owner.start(requests[0])
        self.assertNotEqual(replay.operation_ref, refs[0])
        await owner.wait(replay.operation_ref)

    async def test_timer_expires_idle_receipts_and_releases_text_without_polling(self):
        owner, _, _ = self.make_owner(adapter=lambda *args: "done", receipt_ttl=0.01)
        status = owner.start(encode(payload(owner)))
        await owner.wait(status.operation_ref)
        await eventually(lambda: not owner._entries)
        self.assertFalse(owner._requests)
        self.assertIsNone(owner._expiry)

    async def test_active_custody_is_not_expired_or_counted_as_settled(self):
        now = [1.0]
        owner, manager, gate = self.make_owner(clock=lambda: now[0], max_receipts=1, receipt_ttl=1)
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        now[0] += 10000
        self.assertEqual(owner.status(status.operation_ref).state, "running")
        self.assertEqual(len(owner._settled), 0)
        self.assertTrue(manager.lock.locked())
        self.assertIsNotNone(owner._active.audio)

    async def test_runtime_replacement_and_unknown_operations_never_target_successor(self):
        owner, _, gate = self.make_owner()
        successor, _, _ = self.make_owner()
        body = encode(payload(owner))
        status = owner.start(body)
        for action in (successor.status, successor.cancel):
            self.assert_code("runtime_replaced", lambda: action(status.operation_ref))
        self.assert_code("runtime_replaced", lambda: successor.start(body))
        missing = OperationRef(owner.runtime_instance_id, str(uuid4()), owner._manager.fixture_ref)
        self.assert_code("unknown_or_expired_operation", lambda: owner.status(missing))
        self.assert_code("invalid_operation_ref", lambda: owner.status({}))
        gate.release.set()
        await owner.wait(status.operation_ref)

    async def test_bounded_drain_reports_incomplete_and_preserves_worker_then_finishes(self):
        owner, manager, gate = self.make_owner()
        body = encode(payload(owner))
        status = owner.start(body)
        await eventually(gate.entered.is_set)
        report = await owner.drain(0.01)
        self.assertTrue(report.admission_closed)
        self.assertFalse(report.custody_complete)
        self.assertEqual(report.incomplete[0].operation_ref, status.operation_ref)
        self.assertTrue(manager.lock.locked())
        self.assertEqual(owner.start(body).operation_ref, status.operation_ref)
        self.assert_code("admission_closed", lambda: owner.start(encode(payload(owner))))
        gate.release.set()
        self.assertTrue((await owner.drain(3)).custody_complete)
        self.assertEqual(owner.status(status.operation_ref).state, "cancelled")

    async def test_cancelled_drain_waiter_cannot_release_custody(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        drain = asyncio.create_task(owner.drain(3))
        await asyncio.sleep(0)
        drain.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await drain
        self.assertTrue(manager.lock.locked())
        self.assertFalse(owner.close_admission().custody_complete)
        gate.release.set()
        await owner.wait(status.operation_ref)

    async def test_shutdown_before_owner_starts_prevents_preprocessing(self):
        owner, manager, gate = self.make_owner()
        status = owner.start(encode(payload(owner)))
        report = await owner.drain(3)
        self.assertTrue(report.custody_complete)
        self.assertEqual(owner.status(status.operation_ref).state, "cancelled")
        self.assertEqual(gate.calls, 0)
        self.assertEqual(manager.entered, 0)

    async def test_real_model_manager_lease_blocks_unload_and_load_during_native_work(self):
        manager, slot = await loaded_manager()
        gate = Gate()
        owner = SpeechOperationOwner(manager, adapter=gate)
        self.addAsyncCleanup(self.finish_owner, owner, gate)
        status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        with self.assertRaisesRegex(RuntimeError, "busy"):
            await manager.unload(slot.slot_id)
        with self.assertRaisesRegex(RuntimeError, "busy"):
            async with manager.speech_lease(owner._active.binding):
                self.fail("second speech lease admitted")
        queued_load = asyncio.create_task(manager.load("/other", "other", model_type=COHERE_ASR))
        await asyncio.sleep(0)
        self.assertFalse(queued_load.done())
        queued_load.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await queued_load
        owner.cancel(status.operation_ref)
        self.assertTrue(manager._get_device_lock(slot.device).locked())
        gate.release.set()
        await owner.wait(status.operation_ref)
        await manager.unload(slot.slot_id)

    async def test_default_adapter_shutdown_during_preprocessing_and_generation(self):
        class Inputs(dict):
            def to(self, *args, **kwargs):
                return self

        for phase in ("preprocessing", "generation"):
            with self.subTest(phase=phase):
                manager = Manager()
                entered, release = threading.Event(), threading.Event()

                def block(value):
                    entered.set()
                    if not release.wait(3):
                        raise RuntimeError("Synthetic stage was not released")
                    return value

                processor = Mock(return_value=Inputs(input_features="fixture"))
                model = Mock(device="cpu", dtype="float32")
                model.generate.return_value = [[1]]
                processor.decode.return_value = "late synthetic transcript"
                if phase == "preprocessing":
                    processor.side_effect = lambda *args, **kwargs: block(
                        Inputs(input_features="fixture")
                    )
                else:
                    model.generate.side_effect = lambda **kwargs: block([[1]])
                manager.loaded = types.SimpleNamespace(model=model, tokenizer=processor)
                owner = SpeechOperationOwner(manager)
                with (
                    patch(
                        "loaders.cohere_asr_loader._native_api",
                        return_value=(None, None, object, list),
                    ),
                    patch.dict(
                        sys.modules,
                        {"torch": types.SimpleNamespace(inference_mode=contextlib.nullcontext)},
                    ),
                ):
                    status = owner.start(encode(payload(owner)))
                    try:
                        await eventually(entered.is_set)
                        report = await owner.drain(0.01)
                        self.assertFalse(report.custody_complete)
                        self.assertTrue(manager.lock.locked())
                        self.assertEqual(manager.exited, 0)
                    finally:
                        release.set()
                        done = await owner.wait(status.operation_ref)
                    self.assertEqual(done.state, "cancelled")
                    self.assertIsNone(done.text)
                    self.assertFalse(manager.lock.locked())
                    processor.decode.assert_not_called()
                    if phase == "preprocessing":
                        model.generate.assert_not_called()

    async def test_lease_refusals_are_typed_without_starting_native_work(self):
        for error, code in (
            (KeyError("unavailable"), "model_unavailable"),
            (ValueError("wrong model type"), "model_unsupported"),
            (RuntimeError("device busy"), "runtime_busy"),
        ):

            class RefusingManager(Manager):
                @contextlib.asynccontextmanager
                async def speech_lease(self, binding):
                    raise error
                    yield

            gate = Gate()
            owner = SpeechOperationOwner(RefusingManager(), adapter=gate)
            self.addAsyncCleanup(self.finish_owner, owner, gate)
            started = owner.start(encode(payload(owner)))
            done = await owner.wait(started.operation_ref)
            self.assertEqual(done.state, "failed")
            self.assertEqual(done.operation_diagnostic.code, code)
            self.assertEqual(done.cleanup, "confirmed")
            self.assertEqual(gate.calls, 0)
            self.assertIsNone(owner._entries[started.operation_ref.operation_id].audio)

    async def test_owner_is_retained_independently_of_caller_references(self):
        manager = Manager()
        gate = Gate()
        owner = SpeechOperationOwner(manager, adapter=gate)
        started = owner.start(encode(payload(owner)))
        retained = weakref.ref(owner)
        del owner
        gc.collect()
        self.assertIsNotNone(retained())
        try:
            await eventually(gate.entered.is_set)
            self.assertTrue(manager.lock.locked())
        finally:
            gate.release.set()
            await retained().wait(started.operation_ref)
        self.assertFalse(manager.lock.locked())
        self.assertNotIn(retained(), _CUSTODIANS)

    async def test_resource_options_cannot_weaken_production_limits(self):
        for options in (
            {"max_receipts": MAX_RECEIPTS + 1},
            {"max_receipts": 0},
            {"max_receipts": True},
            {"receipt_ttl": 601},
            {"receipt_ttl": float("inf")},
            {"receipt_ttl": float("nan")},
        ):
            with self.assertRaises(ValueError):
                SpeechOperationOwner(Manager(), **options)
        owner, _, _ = self.make_owner()
        for timeout in (float("inf"), float("nan"), -1, True):
            with self.assertRaises(ValueError):
                await owner.drain(timeout)

    async def test_receipt_diagnostic_does_not_copy_submitted_audio_or_backend_text(self):
        secret = b"PRIVATE-PCM-1234"
        copied = []

        class AudioError(RuntimeError):
            def __str__(self):
                copied.append(True)
                return repr(secret) + " private transcript content"

        AudioError.__name__ = "PRIVATE_BACKEND_TYPE_NAME"

        def fail(*args):
            raise AudioError(secret)

        owner, _, _ = self.make_owner(adapter=fail)
        started = owner.start(encode(payload(owner, pcm=secret)))
        done = await owner.wait(started.operation_ref)
        self.assertEqual(done.state, "failed")
        self.assertFalse(copied, "Diagnostic extraction called backend exception __str__")
        self.assertNotIn("PRIVATE", repr(done))
        self.assertNotIn("private transcript", repr(done))
        self.assertEqual(done.operation_diagnostic.exception_type, "RuntimeError")

    async def test_outcome_notification_is_not_thread_exit(self):
        returned, release_exit = threading.Event(), threading.Event()

        class DelayedExitThread(threading.Thread):
            def run(self):
                super().run()
                returned.set()
                release_exit.wait(3)

        owner, manager, _ = self.make_owner(adapter=lambda *args: "done")
        with patch("speech_operations.Thread", DelayedExitThread):
            started = owner.start(encode(payload(owner)))
            try:
                await eventually(returned.is_set)
                await asyncio.sleep(0.01)
                self.assertTrue(manager.lock.locked())
                self.assertEqual(owner.status(started.operation_ref).state, "running")
                self.assertFalse(owner._active.observed.done())
            finally:
                release_exit.set()
                done = await owner.wait(started.operation_ref)
        self.assertEqual(done.state, "completed")

    async def test_foreign_event_loop_is_rejected(self):
        owner, _, _ = self.make_owner()
        body = encode(payload(owner))

        def foreign():
            async def attempt():
                with self.assertRaisesRegex(RuntimeError, "owning event loop"):
                    owner.start(body)

            asyncio.run(attempt())

        await asyncio.to_thread(foreign)
        self.assertFalse(owner._entries)


async def quarantine_fixture(artifact_failure=False, lease_failure=False):
    """This child must remain alive until its external test supervisor kills it."""
    manager, slot = await loaded_manager()
    retained = np.array([0.25], dtype=np.float32)
    now = [1.0]

    def uncertain(*args):
        raise SpeechCleanupUnconfirmed(
            ValueError("original generation failure"),
            RuntimeError("synthetic device sync failure"),
            retained,
        )

    if artifact_failure:
        manager._speech_artifact_authority.release_error = RuntimeError("private artifact cleanup")
    if lease_failure:
        real_lease = manager.speech_lease

        class UncertainExit:
            def __init__(self, binding):
                self.lease = real_lease(binding)

            async def __aenter__(self):
                return await self.lease.__aenter__()

            async def __aexit__(self, *args):
                raise RuntimeError("private lease cleanup")

        manager.speech_lease = UncertainExit
    owner = SpeechOperationOwner(
        manager,
        adapter=(lambda *args: "done") if artifact_failure or lease_failure else uncertain,
        clock=lambda: now[0],
        max_receipts=1,
        receipt_ttl=1,
    )
    request = encode(payload(owner))
    status = owner.start(request)
    borrow = owner._active.binding.artifact_use
    result = await owner.wait(status.operation_ref)
    assert not borrow.released
    assert result.state == "cleanup_unconfirmed"
    assert result.cleanup == "unconfirmed"
    assert result.text is None
    if artifact_failure or lease_failure:
        assert result.operation_diagnostic is None
        expected = (
            "artifact_cleanup_unconfirmed" if artifact_failure else "lease_cleanup_unconfirmed"
        )
        assert result.cleanup_diagnostic.code == expected
    else:
        assert result.operation_diagnostic.code == "inference_failed"
        assert result.operation_diagnostic.exception_type == "ValueError"
        assert result.cleanup_diagnostic.code == "device_cleanup_unconfirmed"
        assert result.cleanup_diagnostic.exception_type == "RuntimeError"
        assert owner._active.quarantine.retained_audio is retained
        assert retained.tolist() == [0.25]
    assert owner._active.audio is not None
    assert manager._get_device_lock(slot.device).locked()
    assert owner in _CUSTODIANS
    assert owner._active.runner is not None
    for _ in range(5):
        owner.cancel(status.operation_ref)
        owner._active.runner.cancel()
        await asyncio.sleep(0)
        assert manager._get_device_lock(slot.device).locked()
    now[0] = 100000.0
    assert owner.status(status.operation_ref).state == "cleanup_unconfirmed"
    assert len(owner._settled) == 0
    assert len(owner._entries) == 1
    assert owner.start(request).operation_ref == status.operation_ref
    try:
        owner.start(encode(payload(owner)))
    except SpeechOperationError as error:
        assert error.code == "runtime_busy"
    else:
        raise AssertionError("Quarantine admitted new work")
    before = time.monotonic()
    report = await owner.drain(10)
    assert time.monotonic() - before < 1
    assert not report.custody_complete
    assert report.incomplete[0].state == "cleanup_unconfirmed"
    try:
        await manager.unload(slot.slot_id)
    except RuntimeError:
        pass
    else:
        raise AssertionError("Quarantine allowed unload")

    # asyncio.run now cancels all tasks. The retained quarantine task must keep
    # shutdown from reaching shutdown_asyncgens and releasing the real lease.
    # A later loop callback verifies the actual lock after that cancellation.
    def after_shutdown_cancel():
        assert manager._get_device_lock(slot.device).locked()
        assert not owner._active.runner.done()
        assert not borrow.released
        if not artifact_failure and not lease_failure:
            assert owner._active.quarantine.retained_audio is retained
        print("QUARANTINE_SURVIVES_ORDERLY_SHUTDOWN", flush=True)

    asyncio.get_running_loop().call_later(0.05, after_shutdown_cancel)


async def startup_fixture(mode):
    manager = Manager()
    retained = np.array([0.5], dtype=np.float32)
    gate = Gate(
        error=SpeechCleanupUnconfirmed(
            ValueError("private inference content"),
            RuntimeError("private cleanup content"),
            retained,
        )
        if mode == "cleanup"
        else None
    )
    workers = []

    class AmbiguousStartThread(threading.Thread):
        def start(self):
            workers.append(self)
            if mode != "unacknowledged":
                super().start()
            raise RuntimeError("private startup exception content")

    owner = SpeechOperationOwner(manager, adapter=gate)
    with patch("speech_operations.Thread", AmbiguousStartThread):
        started = owner.start(encode(payload(owner)))
        borrow = owner._active.binding.artifact_use
        result = await owner.wait(started.operation_ref)
    assert not borrow.released
    assert result.state == "cleanup_unconfirmed", result.state
    assert result.startup_diagnostic.code == "worker_start_unconfirmed"
    assert result.startup_diagnostic.exception_type == "RuntimeError"
    assert "private startup" not in repr(result)
    assert manager.lock.locked(), "Startup exception released live device custody"
    assert manager.exited == 0
    assert owner in _CUSTODIANS
    assert owner._active.audio is not None
    if mode != "unacknowledged":
        await eventually(gate.entered.is_set)
        assert workers[0].is_alive()
        gate.release.set()
        await eventually(lambda: not workers[0].is_alive())
        await asyncio.sleep(0.01)
        assert manager.lock.locked(), "Native completion alone released startup quarantine"
    else:
        assert not gate.entered.is_set()
    result = owner.status(started.operation_ref)
    assert result.state == "cleanup_unconfirmed"
    assert result.startup_diagnostic.code == "worker_start_unconfirmed"
    assert result.text is None
    if mode == "cleanup":
        assert result.operation_diagnostic.code == "inference_failed"
        assert result.cleanup_diagnostic.code == "device_cleanup_unconfirmed"
        assert owner._active.quarantine[1].retained_audio is retained
        assert "private" not in repr(result)
    else:
        assert result.cleanup_diagnostic is None
    assert not (await owner.drain(0.01)).custody_complete
    owner._active.runner.cancel()
    await asyncio.sleep(0)
    assert manager.lock.locked()
    assert not borrow.released
    if mode == "delayed":
        # Prove the parent waits for evidence rather than killing at two seconds.
        await asyncio.sleep(2.25)
    print("AMBIGUOUS_STARTUP_CUSTODY_RETAINED", flush=True)


async def admission_failure_fixture(mode):
    """Uncertain transfer/launch remains externally supervised until process exit."""
    manager = Manager()
    retained = np.array([0.75], dtype=np.float32)
    gate = Gate(
        error=SpeechCleanupUnconfirmed(
            ValueError("private late inference"), RuntimeError("private late cleanup"), retained
        )
        if mode == "eager_cleanup"
        else None
    )
    owner = SpeechOperationOwner(manager, adapter=gate)
    body = encode(payload(owner))
    loop = asyncio.get_running_loop()
    previous = loop.get_task_factory()
    tasks = []
    if mode == "binding_cleanup":
        bind = manager.bind_speech

        def fail_after_transfer(binding):
            bind(binding)
            raise RuntimeError("private post-transfer failure")

        manager.bind_speech = fail_after_transfer
        manager._speech_artifact_authority.release_error = RuntimeError("private release failure")
    else:

        def factory(loop, coroutine, **kwargs):
            if mode != "before":
                task = asyncio.Task(
                    coroutine, loop=loop, eager_start=mode.startswith("eager"), **kwargs
                )
                tasks.append(task)
            raise RuntimeError("private factory startup failure")

        loop.set_task_factory(factory)

    class AmbiguousThreadStart(threading.Thread):
        def start(self):
            super().start()
            raise RuntimeError("private native startup failure")

    try:
        if mode == "eager_worker":
            with patch("speech_operations.Thread", AmbiguousThreadStart):
                started = owner.start(body)
        else:
            started = owner.start(body)
    finally:
        loop.set_task_factory(previous)
    assert started.state == "cleanup_unconfirmed"
    assert started.cleanup == "unconfirmed"
    entry = owner._active
    borrow = entry.binding.artifact_use
    assert not borrow.released
    assert entry.audio is not None
    assert entry.guard is not None
    assert owner in _CUSTODIANS
    assert owner.start(body).operation_ref == started.operation_ref
    assert not (await owner.drain(0.01)).custody_complete
    if mode == "binding_cleanup":
        assert started.operation_diagnostic.code == "binding_failed"
        assert started.cleanup_diagnostic.code == "artifact_cleanup_unconfirmed"
        assert started.owner_startup_diagnostic is None
        assert entry.launch is None
        assert gate.calls == 0
    else:
        assert started.owner_startup_diagnostic.code == "owner_start_unconfirmed"
        assert started.owner_startup_diagnostic.exception_type == "RuntimeError"
        assert entry.launch is not None
        if mode.startswith("eager"):
            await eventually(gate.entered.is_set)
            assert manager.lock.locked()
            assert manager.exited == 0
            assert entry.runner is tasks[0]
            assert entry.lease is not None
            gate.release.set()
            await eventually(lambda: not entry.worker.is_alive())
            await asyncio.sleep(0.01)
            assert not tasks[0].done()
            assert manager.lock.locked()
            assert manager.exited == 0
        else:
            await asyncio.sleep(0)
            assert gate.calls == 0
    result = owner.status(started.operation_ref)
    assert result.state == "cleanup_unconfirmed"
    assert result.text is None
    assert "private" not in repr(result).lower()
    assert entry.binding is not None and not borrow.released
    if mode == "eager_worker":
        assert result.startup_diagnostic.code == "worker_start_unconfirmed"
        assert result.owner_startup_diagnostic.code == "owner_start_unconfirmed"
    if mode == "eager_cleanup":
        assert result.operation_diagnostic.code == "inference_failed"
        assert result.cleanup_diagnostic.code == "device_cleanup_unconfirmed"
        assert entry.quarantine[1].retained_audio is retained
    for _ in range(3):
        owner.cancel(started.operation_ref)
        entry.guard.cancel()
        if entry.runner is not None:
            entry.runner.cancel()
        await asyncio.sleep(0)
        assert owner.status(started.operation_ref).state == "cleanup_unconfirmed"
        assert entry.binding is not None and not borrow.released
        assert entry.audio is not None
    assert not (await owner.drain(0.01)).custody_complete

    def after_shutdown_cancel():
        assert owner in _CUSTODIANS
        assert entry.binding is not None and not borrow.released
        assert entry.audio is not None
        assert not entry.guard.done()
        if mode.startswith("eager"):
            assert manager.lock.locked()
            assert entry.lease is not None
        print("ADMISSION_FAILURE_CUSTODY_RETAINED", flush=True)

    loop.call_later(0.05, after_shutdown_cancel)


async def lock_handoff_fixture(cancel_runner):
    """Exercise actual asyncio.Lock's unlocked-but-waiter-owned handoff window."""
    manager = Manager()
    assert type(manager.lock) is asyncio.Lock
    gate = Gate()
    owner = SpeechOperationOwner(manager, adapter=gate)
    body = encode(payload(owner))
    await manager.lock.acquire()
    prior_entered, release_prior = asyncio.Event(), asyncio.Event()

    async def prior_waiter():
        async with manager.lock:
            prior_entered.set()
            await release_prior.wait()

    prior = asyncio.create_task(prior_waiter())
    await asyncio.sleep(0)  # The actual earlier waiter now queues on the lock.
    assert not prior_entered.is_set()
    manager.lock.release()
    assert not manager.lock.locked()
    assert not prior_entered.is_set()  # Awakened, but it has not resumed yet.
    loop = asyncio.get_running_loop()
    previous = loop.get_task_factory()
    tasks = []

    def factory(loop, coroutine, **kwargs):
        tasks.append(asyncio.Task(coroutine, loop=loop, eager_start=True, **kwargs))
        raise RuntimeError("private factory failure during actual lock handoff")

    loop.set_task_factory(factory)
    try:
        started = owner.start(body)
    finally:
        loop.set_task_factory(previous)
    entry = owner._active
    binding, lease = entry.binding, entry.lease
    borrow = binding.artifact_use
    assert entry.runner is tasks[0]
    assert lease is not None
    assert entry.worker is None
    assert started.owner_startup_diagnostic.code == "owner_start_unconfirmed"
    if cancel_runner:
        entry.runner.cancel()
    await prior_entered.wait()
    await asyncio.sleep(0)
    release_prior.set()
    await prior
    await asyncio.sleep(0)
    result = owner.status(started.operation_ref)
    assert result.state == "cleanup_unconfirmed", result.state
    assert result.cleanup == "unconfirmed"
    assert owner._active is entry
    assert owner in _CUSTODIANS
    assert entry.binding is binding and not borrow.released
    assert entry.lease is lease and entry.audio is not None
    assert not entry.runner.done() and not entry.guard.done()
    assert gate.calls == 0 and entry.worker is None
    assert owner.start(body).operation_ref == started.operation_ref
    assert not (await owner.drain(0.01)).custody_complete
    if cancel_runner:
        assert result.operation_diagnostic.code == "cancelled"
        assert not manager.lock.locked()  # Cancellation never owned this lock.
    else:
        assert manager.lock.locked()  # Resumed acquisition retains its exact lease.
    for _ in range(3):
        entry.runner.cancel()
        entry.guard.cancel()
        await asyncio.sleep(0)
        assert entry.binding is binding and not borrow.released
        assert owner.status(started.operation_ref).state == "cleanup_unconfirmed"

    def after_shutdown_cancel():
        assert owner._active is entry and owner in _CUSTODIANS
        assert entry.binding is binding and not borrow.released
        assert entry.lease is lease and entry.audio is not None
        assert not entry.guard.done() and not entry.runner.done()
        print("REAL_LOCK_HANDOFF_CUSTODY_RETAINED", flush=True)

    loop.call_later(0.05, after_shutdown_cancel)


class QuarantineProcessTests(unittest.TestCase):
    def assert_retained_child(self, fixture_args, marker):
        """Bound marker readiness separately from post-marker custody observation."""
        with tempfile.TemporaryDirectory() as directory:
            stdout_path = Path(directory) / "stdout.log"
            stderr_path = Path(directory) / "stderr.log"

            def read_log(path):
                with path.open("rb") as source:
                    data = source.read(65_537)
                self.assertLessEqual(len(data), 65_536, "Synthetic fixture log overflow")
                return data

            # argv contains only this test file, the current interpreter and
            # fixed fixture selectors below. No request data or shell is used.
            # File-backed output avoids unowned reader threads and pipe stalls;
            # observations use separate file descriptions from the child writer.
            with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
                process = subprocess.Popen(
                    [sys.executable, str(Path(__file__).resolve()), *fixture_args],
                    stdout=stdout,
                    stderr=stderr,
                    env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
                )
            try:
                deadline = time.monotonic() + 30
                while marker not in read_log(stdout_path):
                    self.assertIsNone(
                        process.poll(),
                        f"Fixture exited before custody marker: {read_log(stderr_path)!r}",
                    )
                    self.assertLess(
                        time.monotonic(),
                        deadline,
                        f"Fixture custody marker deadline expired: {read_log(stderr_path)!r}",
                    )
                    time.sleep(0.01)
                # Readiness is established before checking that quarantine still
                # prevents orderly process exit. This is an observation bound,
                # not a claim that a delay proves indefinite resource custody.
                with self.assertRaises(subprocess.TimeoutExpired):
                    process.wait(timeout=0.25)
            finally:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=5)
                self.assertIsNotNone(process.returncode, "Fixture child exit was not observed")
            self.assertIn(marker, read_log(stdout_path), read_log(stderr_path).decode())
            self.assertNotIn(b"Traceback", read_log(stderr_path))

    def test_real_lock_handoff_preserves_factory_latch_on_cancellation_or_acquisition(self):
        for mode in ("cancel", "acquire"):
            with self.subTest(mode=mode):
                self.assert_retained_child(
                    ["--lock-handoff-fixture", mode], b"REAL_LOCK_HANDOFF_CUSTODY_RETAINED"
                )

    def test_admission_transfer_and_task_factory_failure_retain_observable_custody(self):
        for mode in (
            "binding_cleanup",
            "before",
            "scheduled",
            "eager",
            "eager_cleanup",
            "eager_worker",
        ):
            with self.subTest(mode=mode):
                self.assert_retained_child(
                    ["--admission-fixture", mode], b"ADMISSION_FAILURE_CUSTODY_RETAINED"
                )

    def test_exceptional_start_retains_running_or_unknown_native_custody(self):
        for mode in ("started", "unacknowledged", "cleanup", "delayed"):
            with self.subTest(mode=mode):
                self.assert_retained_child(
                    ["--startup-fixture", mode], b"AMBIGUOUS_STARTUP_CUSTODY_RETAINED"
                )

    def test_lease_cleanup_uncertainty_retains_artifact_borrow_through_shutdown(self):
        self.assert_retained_child(["--lease-fixture"], b"QUARANTINE_SURVIVES_ORDERLY_SHUTDOWN")

    def test_artifact_release_uncertainty_retains_device_and_borrow_through_shutdown(self):
        self.assert_retained_child(["--artifact-fixture"], b"QUARANTINE_SURVIVES_ORDERLY_SHUTDOWN")

    def test_cleanup_unconfirmed_survives_cancellation_expiry_drain_and_orderly_loop_shutdown(self):
        self.assert_retained_child(
            ["--quarantine-fixture"], b"QUARANTINE_SURVIVES_ORDERLY_SHUTDOWN"
        )


if __name__ == "__main__":
    if "--lock-handoff-fixture" in sys.argv:
        asyncio.run(lock_handoff_fixture(cancel_runner=sys.argv[-1] == "cancel"))
        raise AssertionError(
            "Lock handoff quarantine unexpectedly allowed orderly runtime shutdown"
        )
    if "--admission-fixture" in sys.argv:
        asyncio.run(admission_failure_fixture(sys.argv[-1]))
        raise AssertionError("Admission quarantine unexpectedly allowed orderly runtime shutdown")
    if "--startup-fixture" in sys.argv:
        asyncio.run(startup_fixture(sys.argv[-1]))
        raise AssertionError("Startup quarantine unexpectedly allowed runtime shutdown")
    if "--lease-fixture" in sys.argv:
        asyncio.run(quarantine_fixture(lease_failure=True))
        raise AssertionError("Lease quarantine unexpectedly allowed orderly runtime shutdown")
    if "--artifact-fixture" in sys.argv:
        asyncio.run(quarantine_fixture(artifact_failure=True))
        raise AssertionError("Artifact quarantine unexpectedly allowed orderly runtime shutdown")
    if "--quarantine-fixture" in sys.argv:
        asyncio.run(quarantine_fixture())
        raise AssertionError("Quarantine unexpectedly allowed orderly runtime shutdown")
    unittest.main()
