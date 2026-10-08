"""Controlled owning-worker contracts; no models or installed ASR qualification."""

import asyncio
import base64
import copy
import json
import pickle
from pathlib import Path
import subprocess
import sys
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _FakeDeviceManager, _TestModelManager
from model_manager import LoadedModel, SlotState
from owned_audio import (
    OwnedAudioActor,
    OwnedAudioError,
    OwnedLoadPlan,
    _CUSTODIANS,
)
from owned_model_operations import OwnedModelOperations
from speech_binding import SpeechBindingError, SpeechSlotRef


class Device:
    def __init__(self, kind="cpu"):
        self.type = kind

    def __str__(self):
        return self.type


class Devices(_FakeDeviceManager):
    def resolve_device(self, value):
        return Device(value)


class Custody:
    def __init__(self):
        self.released = False
        self.releases = 0
        self.partial = None


class ControlledGate:
    def __init__(self):
        self.entered = threading.Event()
        self.continue_load = threading.Event()
        self.continue_load.set()
        self.cleanup_entered = threading.Event()
        self.continue_cleanup = threading.Event()
        self.continue_cleanup.set()
        self.loads = 0
        self.cleanups = 0
        self.load_error = None
        self.cleanup_error = None
        self.release_error = None
        self.actor = None
        self.worker_threads = []

    def prepare(self, manager, model_id="library/speech", device="cpu"):
        return OwnedLoadPlan._from_native_gate(
            self,
            runtime_instance_id=manager.runtime_instance_id,
            model_id=model_id,
            custody=Custody(),
            device=device,
        )

    def admit(self, plan, runtime_instance_id):
        assert plan._gate is self and plan.runtime_instance_id == runtime_instance_id
        assert not plan._custody.released

    def load(self, plan, device):
        self.loads += 1
        self.worker_threads.append(threading.get_ident())
        assert plan._claimed and self.actor in _CUSTODIANS
        assert self.actor.manager._get_device_lock(str(device)).locked()
        plan._custody.partial = object()
        self.entered.set()
        if not self.continue_load.wait(5):
            raise RuntimeError("Controlled load fixture was not released")
        if self.load_error is not None:
            raise self.load_error
        return LoadedModel(object(), object(), device, "cohere-asr")

    def cleanup(self, plan, loaded, device):
        self.cleanups += 1
        self.worker_threads.append(threading.get_ident())
        assert self.actor.manager._get_device_lock(str(device)).locked()
        assert not plan._custody.released
        self.cleanup_entered.set()
        if not self.continue_cleanup.wait(5):
            raise RuntimeError("Controlled cleanup fixture was not released")
        if self.cleanup_error is not None:
            raise self.cleanup_error
        plan._custody.partial = None

    def release_custody(self, plan):
        self.worker_threads.append(threading.get_ident())
        if self.release_error is not None:
            raise self.release_error
        assert not plan._custody.released
        assert plan._custody.partial is None
        plan._custody.released = True
        plan._custody.releases += 1


async def eventually(predicate):
    async with asyncio.timeout(3):
        while not predicate():
            await asyncio.sleep(0)


class OwnedAudioTests(unittest.IsolatedAsyncioTestCase):
    def make_actor(self, **kwargs):
        manager = _TestModelManager(Devices())
        gate = ControlledGate()
        actor = OwnedAudioActor(manager, native_gate=gate, **kwargs)
        gate.actor = actor
        self.addAsyncCleanup(self.cleanup_actor, actor, gate)
        return actor, manager, gate

    async def cleanup_actor(self, actor, gate):
        gate.continue_load.set()
        gate.continue_cleanup.set()
        await eventually(lambda: actor._active is None)
        for ref, entry in list(actor._entries.items()):
            if entry.state == "ready":
                await actor.unload(ref)
        actor.close_admission()

    async def test_default_gate_refuses_and_plain_values_cannot_install_authority(self):
        manager = _TestModelManager(Devices())
        actor = OwnedAudioActor(manager)
        for value in ("/path", {"qualified": True, "path": "/path"}, None):
            with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
                await actor.load(value)
        plan = OwnedLoadPlan._from_native_gate(
            actor._gate,
            runtime_instance_id=manager.runtime_instance_id,
            model_id="library/speech",
            custody=Custody(),
        )
        with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unqualified"):
            await actor.load(plan)
        self.assertFalse(plan._claimed)
        self.assertFalse(manager.slots)
        with self.assertRaisesRegex(RuntimeError, "already configured"):
            OwnedAudioActor(manager)
        for action in (copy.copy, copy.deepcopy, pickle.dumps):
            with self.assertRaises(TypeError):
                action(plan)

    async def test_ready_exact_borrows_and_clean_unload_preserve_legacy_slots(self):
        actor, manager, gate = self.make_actor()
        legacy = await manager.load("/legacy", "legacy", model_type="cohere-asr")
        with self.assertRaises(SpeechBindingError):
            actor.acquire(manager.speech_slot_ref(legacy.slot_id))
        plan = gate.prepare(manager)
        ready = await actor.load(plan)
        self.assertEqual(ready.state, "ready")
        self.assertFalse(ready.production_available)
        self.assertIs(actor.manager, manager)
        self.assertIs(actor.authority, actor)
        self.assertIsNone(manager.get_model_for_inference(plan.model_id))
        self.assertNotIn(plan.model_id, manager.list_model_names())
        self.assertIsNotNone(manager.get_model_for_inference("legacy"))
        with self.assertRaisesRegex(RuntimeError, "exact owner"):
            await manager.unload(ready.slot_ref.slot_id)
        borrow = actor.acquire(ready.slot_ref)
        borrow.validate(ready.slot_ref)
        with self.assertRaises(SpeechBindingError):
            actor.acquire(ready.slot_ref)
        with self.assertRaisesRegex(OwnedAudioError, "runtime_busy"):
            await actor.unload(ready.slot_ref)
        other = gate.prepare(manager)
        with self.assertRaisesRegex(OwnedAudioError, "runtime_busy"):
            await actor.load(other)
        self.assertFalse(other._claimed)
        borrow.release()
        borrow.release()
        retired = await actor.unload(ready.slot_ref)
        self.assertEqual((retired.state, retired.cleanup), ("retired", "confirmed"))
        self.assertEqual(plan._custody.releases, 1)
        self.assertNotIn(ready.slot_ref.slot_id, manager.slots)
        self.assertTrue(all(t != threading.get_ident() for t in gate.worker_threads))
        with self.assertRaises(SpeechBindingError):
            borrow.validate(ready.slot_ref)
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            await actor.load(plan)
        await manager.unload(legacy.slot_id)

    async def test_lost_load_caller_and_repeated_runner_cancel_retain_until_cleanup(self):
        actor, manager, gate = self.make_actor()
        gate.continue_load.clear()
        gate.continue_cleanup.clear()
        plan = gate.prepare(manager)
        caller = asyncio.create_task(actor.load(plan))
        await eventually(gate.entered.is_set)
        entry = actor._active
        caller.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await caller
        for _ in range(4):
            entry.runner.cancel()
            await asyncio.sleep(0)
        self.assertFalse(plan._custody.released)
        self.assertTrue(manager._get_device_lock("cpu").locked())
        self.assertEqual(entry.slot.state, SlotState.LOADING)
        gate.continue_load.set()
        await eventually(gate.cleanup_entered.is_set)
        self.assertEqual(gate.loads, 1)
        self.assertFalse(plan._custody.released)
        gate.continue_cleanup.set()
        await eventually(lambda: entry.cleanup == "confirmed")
        self.assertEqual(entry.error_code, "cancelled")
        self.assertEqual(entry.state, "failed")
        self.assertEqual(plan._custody.releases, 1)
        self.assertFalse(manager._get_device_lock("cpu").locked())

    async def test_cancel_before_runner_poll_has_no_native_load_effect(self):
        actor, manager, gate = self.make_actor()
        caller = asyncio.create_task(actor.load(gate.prepare(manager)))
        await asyncio.sleep(0)
        entry = actor._active
        actor.close_admission()
        await caller
        self.assertEqual(gate.loads, 0)
        self.assertEqual(entry.cleanup, "confirmed")
        self.assertFalse(manager.slots)

    async def test_unload_caller_loss_finishes_original_cleanup_once(self):
        actor, manager, gate = self.make_actor()
        plan = gate.prepare(manager)
        ready = await actor.load(plan)
        gate.continue_cleanup.clear()
        caller = asyncio.create_task(actor.unload(ready.slot_ref))
        await eventually(gate.cleanup_entered.is_set)
        entry = actor._active
        caller.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await caller
        for _ in range(3):
            entry.runner.cancel()
            await asyncio.sleep(0)
        self.assertFalse(plan._custody.released)
        self.assertTrue(manager._get_device_lock("cpu").locked())
        with self.assertRaises(SpeechBindingError):
            actor.acquire(ready.slot_ref)
        gate.continue_cleanup.set()
        await eventually(lambda: entry.cleanup == "confirmed")
        self.assertEqual(gate.cleanups, 1)
        self.assertEqual(plan._custody.releases, 1)
        self.assertEqual(entry.state, "retired")

    async def test_load_failure_cleans_owned_partial_state_before_release(self):
        actor, manager, gate = self.make_actor()
        gate.load_error = RuntimeError("private loader diagnostic")
        plan = gate.prepare(manager)
        failed = await actor.load(plan)
        self.assertEqual((failed.state, failed.error_code), ("failed", "load_failed"))
        self.assertEqual(failed.cleanup, "confirmed")
        self.assertNotIn("private", repr(failed))
        self.assertIsNone(plan._custody.partial)
        self.assertEqual((gate.loads, gate.cleanups, plan._custody.releases), (1, 1, 1))

    async def test_cuda_completion_before_ready_and_both_sides_of_disposal(self):
        actor, manager, gate = self.make_actor()
        plan = gate.prepare(manager, device="cuda")
        calls = []

        def sync(device):
            self.assertTrue(manager._get_device_lock("cuda").locked())
            self.assertFalse(plan._custody.released)
            calls.append((gate.loads, gate.cleanups, threading.get_ident()))

        with patch("owned_audio.torch.cuda.synchronize", sync, create=True):
            ready = await actor.load(plan)
            self.assertEqual(len(calls), 1)
            await actor.unload(ready.slot_ref)
        self.assertEqual([c[:2] for c in calls], [(1, 0), (1, 0), (1, 1)])
        self.assertTrue(all(c[2] != threading.get_ident() for c in calls))

    async def test_exact_generation_and_bounded_settled_receipts(self):
        actor, manager, gate = self.make_actor(max_settled_receipts=1)
        first = await actor.load(gate.prepare(manager))
        await actor.unload(first.slot_ref)
        second = await actor.load(gate.prepare(manager))
        self.assertNotEqual(first.slot_ref.load_generation, second.slot_ref.load_generation)
        with self.assertRaises(SpeechBindingError):
            actor.acquire(first.slot_ref)
        wrong = SpeechSlotRef(manager.runtime_instance_id, second.slot_ref.slot_id, "wrong")
        with self.assertRaisesRegex(OwnedAudioError, "slot_replaced"):
            await actor.unload(wrong)
        await actor.unload(second.slot_ref)
        with self.assertRaisesRegex(OwnedAudioError, "slot_replaced"):
            actor.status(first.slot_ref)
        self.assertEqual(len(actor._entries), 1)
        self.assertEqual(len(actor._settled), 1)
        self.assertIsNone(actor._entries[second.slot_ref].plan)

    async def test_default_receipt_capacity_evicts_only_settled_entries(self):
        actor, manager, gate = self.make_actor()
        refs = []
        for _ in range(65):
            ready = await actor.load(gate.prepare(manager))
            refs.append(ready.slot_ref)
            await actor.unload(ready.slot_ref)
        self.assertEqual(len(actor._entries), 64)
        with self.assertRaisesRegex(OwnedAudioError, "slot_replaced"):
            actor.status(refs[0])
        self.assertEqual(actor.status(refs[1]).cleanup, "confirmed")
        ready = await actor.load(gate.prepare(manager))
        self.assertEqual(len(actor._entries), 65)
        self.assertEqual(actor.status(ready.slot_ref).state, "ready")
        with self.assertRaisesRegex(ValueError, "between 1 and 64"):
            OwnedAudioActor(_TestModelManager(Devices()), max_settled_receipts=65)

    async def test_same_child_successor_bridge_reuses_runtime_owner_and_refuses_old_slot(self):
        from owned_model_operations import OwnedOperationError

        actor, manager, gate = self.make_actor()
        calls = []

        def native(*args):
            calls.append(args)
            return "controlled transcript"

        def body(model):
            return json.dumps(
                {
                    "contract_version": 1,
                    "request_id": "same-caller-id",
                    "model": model,
                    "capability": "audio_transcription",
                    "input": {
                        "kind": "audio",
                        "encoding": "pcm_s16le",
                        "sample_rate_hz": 16000,
                        "channels": 1,
                        "sample_count": 1,
                        "data_base64": "AAA=",
                    },
                    "output": "text",
                    "options": {"kind": "audio"},
                }
            ).encode()

        first_plan = gate.prepare(manager)
        first = await actor.load(first_plan)
        old = OwnedModelOperations(
            actor,
            model=first_plan.model_id,
            profile="owned",
            slot_ref=first.slot_ref,
            adapter=native,
        )
        first_handle = old.start(body(first_plan.model_id))
        self.assertEqual((await old.wait(first_handle)).state, "completed")
        runtime_owner = manager._speech_owner
        await actor.unload(first.slot_ref)
        second_plan = gate.prepare(manager)
        second = await actor.load(second_plan)
        successor = OwnedModelOperations(
            actor,
            model=second_plan.model_id,
            profile="owned",
            slot_ref=second.slot_ref,
            adapter=native,
        )
        self.assertIs(successor._native, runtime_owner)
        self.assertIs(old._native, runtime_owner)
        with self.assertRaises(OwnedOperationError):
            old.start(body(first_plan.model_id))
        old.close_admission()
        self.assertFalse(runtime_owner._closed)
        handle = successor.start(body(second_plan.model_id))
        self.assertNotEqual(first_handle._native_ref, handle._native_ref)
        self.assertEqual((await successor.wait(handle)).state, "completed")
        self.assertEqual(len(calls), 2)
        successor.close_admission()
        await actor.unload(second.slot_ref)
        self.assertEqual(first_plan._custody.releases, 1)
        self.assertEqual(second_plan._custody.releases, 1)

    async def test_retired_missing_bridge_refuses_before_claiming_native_owner(self):
        from owned_model_operations import OwnedOperationError

        actor, manager, gate = self.make_actor()
        ready = await actor.load(gate.prepare(manager))
        await actor.unload(ready.slot_ref)
        missing = SpeechSlotRef(manager.runtime_instance_id, "missing", "missing")
        for ref in (ready.slot_ref, missing):
            with self.assertRaisesRegex(OwnedOperationError, "capability_unavailable"):
                OwnedModelOperations(actor, model="speech", profile="owned", slot_ref=ref)
            self.assertIsNone(manager._speech_owner)

    async def test_pretransfer_native_gate_refusal_projects_no_private_diagnostic(self):
        actor, manager, gate = self.make_actor()
        plan = gate.prepare(manager)
        with patch.object(gate, "admit", side_effect=RuntimeError("private recipe evidence")):
            with self.assertRaisesRegex(OwnedAudioError, "^load_admission_failed$"):
                await actor.load(plan)
        self.assertFalse(plan._claimed)
        self.assertFalse(plan._custody.released)
        self.assertFalse(manager.slots)
        self.assertEqual(gate.loads, 0)

    async def test_actual_actor_bridge_use_cancel_then_exact_unload(self):
        actor, manager, gate = self.make_actor()
        plan = gate.prepare(manager)
        ready = await actor.load(plan)
        entered, finish = threading.Event(), threading.Event()
        calls = []

        def native(model, processor, pcm, language, cancel):
            calls.append((pcm, language, cancel))
            entered.set()
            if not finish.wait(5):
                raise RuntimeError("Controlled inference fixture was not released")
            return "controlled transcript"

        bridge = OwnedModelOperations(
            actor,
            model=plan.model_id,
            profile="owned",
            slot_ref=ready.slot_ref,
            adapter=native,
        )
        request = {
            "contract_version": 1,
            "request_id": "caller-1",
            "model": plan.model_id,
            "capability": "audio_transcription",
            "input": {
                "kind": "audio",
                "encoding": "pcm_s16le",
                "sample_rate_hz": 16000,
                "channels": 2,
                "sample_count": 1,
                "data_base64": base64.b64encode(b"\x00\x40\x00\x00").decode(),
            },
            "output": "text",
            "options": {"kind": "audio", "language": "en"},
        }
        handle = bridge.start(json.dumps(request).encode())
        await eventually(entered.is_set)
        waiter = asyncio.create_task(bridge.wait(handle))
        await asyncio.sleep(0)
        waiter.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await waiter
        self.assertTrue(calls[0][2].is_set())
        self.assertEqual(calls[0][:2], (b"\x00\x20", "en"))
        self.assertEqual(actor.status(ready.slot_ref).outstanding_borrows, 1)
        with self.assertRaisesRegex(OwnedAudioError, "runtime_busy"):
            await actor.unload(ready.slot_ref)
        self.assertFalse(plan._custody.released)
        self.assertTrue(manager._get_device_lock("cpu").locked())
        finish.set()
        settled = await bridge.wait(handle)
        self.assertEqual((settled.state, settled.cleanup), ("cancelled", "confirmed"))
        self.assertEqual(len(calls), 1)
        self.assertEqual(actor.status(ready.slot_ref).outstanding_borrows, 0)
        await actor.unload(ready.slot_ref)
        self.assertEqual(plan._custody.releases, 1)


async def quarantine_fixture(mode):
    manager = _TestModelManager(Devices())
    gate = ControlledGate()
    actor = OwnedAudioActor(manager, native_gate=gate)
    gate.actor = actor
    plan = gate.prepare(manager, device="cuda" if mode == "cuda" else "cpu")
    if mode == "owner_start":

        def failing_factory(loop, coroutine, **kwargs):
            raise RuntimeError("private task factory startup")

        asyncio.get_running_loop().set_task_factory(failing_factory)
        status = await actor.load(plan)
        asyncio.get_running_loop().set_task_factory(None)
    elif mode == "thread_start":
        with patch("owned_audio.Thread.start", side_effect=RuntimeError("private thread startup")):
            status = await actor.load(plan)
    elif mode == "cuda":
        with patch(
            "owned_audio.torch.cuda.synchronize",
            side_effect=RuntimeError("private CUDA"),
            create=True,
        ):
            status = await actor.load(plan)
    else:
        ready = await actor.load(plan)
        if mode == "cleanup":
            gate.cleanup_error = RuntimeError("private cleanup")
        else:
            gate.release_error = RuntimeError("private artifact cleanup")
        status = await actor.unload(ready.slot_ref)
    assert status.state == "cleanup_unconfirmed" and status.cleanup == "unconfirmed"
    assert not plan._custody.released and actor in _CUSTODIANS
    assert not status.production_available and "private" not in repr(status)
    actor.close_admission()
    for _ in range(4):
        if actor._active.runner is not None:
            actor._active.runner.cancel()
        actor._active.guard.cancel()
        await asyncio.sleep(0)
        assert actor.status(status.slot_ref).cleanup == "unconfirmed"
    assert not plan._custody.released
    if mode != "owner_start":
        assert manager._get_device_lock(plan.device).locked()
    print("custody retained before orderly shutdown", flush=True)


class QuarantineTests(unittest.TestCase):
    def test_unknown_start_cleanup_and_cuda_retain_through_orderly_shutdown(self):
        for mode in ("owner_start", "thread_start", "cleanup", "release", "cuda"):
            with self.subTest(mode=mode):
                child = subprocess.Popen(
                    [sys.executable, __file__, "--quarantine", mode],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                try:
                    with self.assertRaises(subprocess.TimeoutExpired):
                        child.communicate(timeout=3)
                finally:
                    child.kill()
                    output, error = child.communicate(timeout=3)
                self.assertIn("custody retained before orderly shutdown", output)
                self.assertNotIn("Traceback", error)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--quarantine":
        asyncio.run(quarantine_fixture(sys.argv[2]))
    else:
        unittest.main()
