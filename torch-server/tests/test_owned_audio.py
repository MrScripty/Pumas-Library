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


# Fixed-source conditional plumbing tests only. This policy is never imported
# or catalogued by shipping code and qualifies neither Torch nor model inference.
class HeldInstalledFixtureSource:
    def __init__(self, root, manager):
        import os

        self.root, self.manager = root, manager
        self.proof = None
        self.released = False
        self.releases = 0
        self.validations = []
        self.native_calls = []
        self.disposals = []
        self.fail_stage = None
        self.hold_model = False
        self.model_entered, self.model_continue = threading.Event(), threading.Event()
        self.model_continue.set()
        self.directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
        self.root_identity = self.identity(os.fstat(self.directory))
        self.members = {}
        bodies = {
            "config.json": json.dumps(
                {"model_type": "cohere_asr", "architectures": ["CohereAsrForConditionalGeneration"]}
            ).encode(),
            "model.safetensors": b"synthetic fixture, no tensors",
            "tokenizer.json": b"{}",
            "tokenizer_config.json": b"{}",
            "preprocessor_config.json": b"{}",
        }
        for name, body in bodies.items():
            (root / name).write_bytes(body)
            fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=self.directory)
            self.members[name] = (fd, self.identity(os.fstat(fd)), body)

    @staticmethod
    def identity(value):
        return value.st_dev, value.st_ino, value.st_size

    def inspect(self):
        import os

        if self.released:
            raise OwnedAudioError("artifact_custody_unavailable")
        # Actual current fixed fixture bytes and descriptor identities, not a
        # metadata hash/locator promoted into installed runtime qualification.
        if self.identity(os.stat(self.root)) != self.identity(os.fstat(self.directory)):
            raise OwnedAudioError("artifact_custody_unavailable")
        for name, (fd, identity, body) in self.members.items():
            if (
                self.identity(os.fstat(fd)) != identity
                or self.identity(os.stat(name, dir_fd=self.directory, follow_symlinks=False))
                != identity
                or os.pread(fd, len(body) + 1, 0) != body
            ):
                raise OwnedAudioError("artifact_custody_unavailable")
        self.validations.append(threading.get_ident())

    def close_model_capabilities(self):
        import os

        assert not self.released
        for fd, _, _ in self.members.values():
            os.close(fd)
        os.close(self.directory)
        self.released = True
        self.releases += 1

    def abort_unclaimed_fixture(self):
        # Explicit test source-owner abort only when no native claim exists.
        if not self.released:
            self.close_model_capabilities()

    def stage(self, name):
        self.native_calls.append(name)
        if name == "model" and self.hold_model:
            self.model_entered.set()
            if not self.model_continue.wait(5):
                raise RuntimeError("Controlled model constructor was not released")
        if self.fail_stage == name:
            raise RuntimeError("controlled native constructor uncertainty")


class FixedInstalledFixturePolicy:
    def retain_source(self, source):
        from owned_audio import _InstalledAudioSourceProof

        if type(source) is not HeldInstalledFixtureSource:
            raise OwnedAudioError("native_runtime_unqualified")
        if source.proof is None:
            return _InstalledAudioSourceProof._from_source_policy(
                self,
                source,
                manager=source.manager,
                model_id="library/speech",
                source_id="fixed-installed-fixture",
            )
        return source.proof

    def bind_retention(self, source, proof):
        if source.proof is not None:
            raise OwnedAudioError("invalid_load_plan")
        source.proof = proof

    def validate_source(self, source):
        source.inspect()

    def model_source(self, source):
        from loaders.owned_cohere_source import HeldCohereReadSource

        return HeldCohereReadSource._from_members(
            source, {name: member[0] for name, member in source.members.items()}
        )

    def dispose_native(self, acquisition, device):
        assert device.type == "cpu" and not acquisition.unknown_allocations
        self.source.disposals.append(tuple(acquisition.objects))
        if self.source.fail_stage == "dispose":
            raise RuntimeError("controlled disposal uncertainty")

    def release_source(self, source):
        assert source is self.source
        source.close_model_capabilities()


_INSTALLED_STAGES = (
    "model_read_source",
    "native_api",
    "processor_classes",
    "feature_extractor",
    "tokenizer_backend",
    "tokenizer_options",
    "tokenizer",
    "processor",
    "model_classes",
    "config",
    "generation_config",
    "weights",
    "model",
    "model_eval",
)


def fixed_installed_native_modules(source):
    import types

    module = types.ModuleType("transformers")
    tokenizers = types.ModuleType("tokenizers")
    safetensors = types.ModuleType("safetensors.torch")

    class NativeObject:
        def __init__(self, kind):
            self.kind = kind
            self.device = Device()

        def eval(self):
            source.stage("model_eval")

    class Feature:
        @staticmethod
        def from_dict(options):
            assert options == {}
            source.stage("feature_extractor")
            return NativeObject("feature_extractor")

    class Tokenizer:
        def __init__(self, *, tokenizer_object, **options):
            assert tokenizer_object.kind == "tokenizer_backend" and options == {}
            source.stage("tokenizer")

    class Backend:
        @staticmethod
        def from_str(raw):
            assert raw == "{}"
            source.stage("tokenizer_backend")
            return NativeObject("tokenizer_backend")

    class Config:
        @staticmethod
        def from_dict(options):
            assert options == {
                "model_type": "cohere_asr",
                "architectures": ["CohereAsrForConditionalGeneration"],
            }
            source.stage("config")
            return NativeObject("config")

    class Generation:
        @staticmethod
        def from_model_config(config):
            assert config.kind == "config"
            source.stage("generation_config")
            return NativeObject("generation_config")

    def weights(filename, *, device):
        import os

        assert device == "cpu" and filename.startswith("/proc/self/fd/")
        # The loader now consumes an immutable copy, retaining the original
        # separately for allocation identity/custody validation.
        import fcntl

        assert source.identity(os.stat(filename)) != source.members["model.safetensors"][1]
        with open(filename, "rb") as sealed:
            required = (
                fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL
            )
            assert fcntl.fcntl(sealed.fileno(), fcntl.F_GET_SEALS) & required == required
        with open(filename, "rb") as reader:
            assert reader.read() == source.members["model.safetensors"][2]
        source.stage("weights")
        return {"owned_fixture_weight": NativeObject("weight")}

    class Processor:
        def __init__(self, *, feature_extractor, tokenizer):
            source.stage("processor")
            self.feature_extractor, self.tokenizer = feature_extractor, tokenizer

    class Model:
        @staticmethod
        def from_pretrained(root, **options):
            assert root is None
            assert options.pop("config").kind == "config"
            assert options.pop("generation_config").kind == "generation_config"
            assert options.pop("state_dict")["owned_fixture_weight"].kind == "weight"
            assert options == {
                "local_files_only": True,
                "trust_remote_code": False,
                "use_safetensors": True,
                "device_map": "cpu",
                "attn_implementation": "eager",
                "dtype": "auto",
                "output_loading_info": True,
            }
            source.stage("model")
            report = {
                "missing_keys": set(),
                "unexpected_keys": set(),
                "mismatched_keys": set(),
                "error_msgs": [],
            }
            if getattr(source, "incomplete_weights", False):
                report["missing_keys"].add("unselected-weight")
            return NativeObject("model"), report

    module.CohereAsrFeatureExtractor = Feature
    module.TokenizersBackend = Tokenizer
    module.CohereAsrProcessor = Processor
    module.CohereAsrForConditionalGeneration = Model
    module.CohereAsrConfig, module.GenerationConfig = Config, Generation
    module.StoppingCriteria, module.StoppingCriteriaList = object, list
    tokenizers.Tokenizer, tokenizers.AddedToken = Backend, object
    safetensors.load_file = weights
    if source.fail_stage == "processor_classes":
        del module.TokenizersBackend
    if source.fail_stage == "model_classes":
        del module.CohereAsrConfig
    return {"transformers": module, "tokenizers": tokenizers, "safetensors.torch": safetensors}


def installed_payload(manager):
    return {
        "runtime_instance_id": manager.runtime_instance_id,
        "model_id": "library/speech",
        "source_id": "fixed-installed-fixture",
    }


class InstalledConditionalPlumbingTests(unittest.IsolatedAsyncioTestCase):
    def fixture(self):
        import tempfile
        from owned_audio import _InstalledOwnedNativeGate

        root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        manager = _TestModelManager(Devices())
        source = HeldInstalledFixtureSource(root, manager)
        policy = FixedInstalledFixturePolicy()
        policy.source = source
        self.enterContext(patch("owned_audio._INSTALLED_AUDIO_POLICIES", (policy,)))
        self.enterContext(patch.dict(sys.modules, fixed_installed_native_modules(source)))
        gate = _InstalledOwnedNativeGate._from_source_owner(source)
        actor = OwnedAudioActor(manager, native_gate=gate)
        self.addAsyncCleanup(self.clean_fixture, actor, source)
        return source, policy, gate, actor, manager

    async def clean_fixture(self, actor, source):
        source.model_continue.set()
        await eventually(lambda: actor._active is None)
        for entry in list(actor._entries.values()):
            if entry.state == "ready":
                await actor.unload(entry.ref)
        actor.close_admission()
        source.abort_unclaimed_fixture()

    async def test_shipping_catalog_and_factory_remain_unqualified(self):
        from owned_audio import _InstalledOwnedNativeGate, _InstalledAudioSourceProof
        from private_owned_channel import (
            _create_installed_private_owned_channel,
            create_private_owned_channel,
        )

        manager = _TestModelManager(Devices())
        for source in ({"qualified": True, "path": "/installed"}, object(), None):
            with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unqualified"):
                _InstalledOwnedNativeGate._from_source_owner(source)
            with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unqualified"):
                _create_installed_private_owned_channel(manager, source)
        with self.assertRaises(TypeError):
            _InstalledAudioSourceProof()
        channel = create_private_owned_channel(manager)
        self.assertIsNone(channel._gate)
        self.assertEqual(type(channel.actor._gate).__name__, "UnavailableOwnedNativeGate")

    async def test_fixed_source_private_provider_load_use_unload_closes_original_capabilities(self):
        import os
        from native_speech_result import NativeSpeechResult
        from private_owned_channel import PrivateOwnedChannel

        source, policy, gate, actor, manager = self.fixture()
        original_fds = [source.directory, *(value[0] for value in source.members.values())]
        channel = PrivateOwnedChannel(
            actor,
            native_gate=gate,
            adapter=lambda *args: NativeSpeechResult("controlled transcript", "stop"),
        )
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        with patch.object(
            policy, "validate_source", side_effect=AssertionError("admit must not scan")
        ):
            self.assertIs(gate.prepare_from_parent(installed_payload(manager), manager), plan)
            gate.admit(plan, manager.runtime_instance_id)
        ready = await channel._dispatch(
            "load", installed_payload(manager), lambda: None, {"cancel": False, "load_ref": None}
        )
        self.assertEqual((ready["state"], ready["cleanup"]), ("ready", "retained"))
        self.assertFalse(actor.status(channel._bridge._slot_ref).production_available)
        self.assertIs(plan._custody.proof._source, source)
        self.assertEqual(
            set(plan._custody.acquisition.objects),
            set(_INSTALLED_STAGES),
        )
        request = {
            "contract_version": 1,
            "request_id": "fixed-source-use",
            "model": "library/speech",
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
        use = await channel._dispatch(
            "use",
            {
                "runtime_instance_id": manager.runtime_instance_id,
                "slot": ready["slot"],
                "request": request,
            },
            lambda: None,
            {},
        )
        settled = await channel._dispatch(
            "status",
            {
                "runtime_instance_id": manager.runtime_instance_id,
                "slot": ready["slot"],
                "operation_id": use["operation_id"],
                "wait_for_settlement": True,
            },
            lambda: None,
            {},
        )
        self.assertEqual(
            (settled["state"], settled["text"], settled["finish_reason"]),
            ("completed", "controlled transcript", "stop"),
        )
        retired = await channel._dispatch(
            "unload",
            {"runtime_instance_id": manager.runtime_instance_id, "slot": ready["slot"]},
            lambda: None,
            {},
        )
        self.assertEqual((retired["state"], retired["cleanup"]), ("retired", "confirmed"))
        self.assertEqual(source.releases, 1)
        self.assertFalse(plan._custody.acquisition.objects)
        self.assertIsNone(plan._custody.loaded)
        for fd in original_fds:
            with self.assertRaises(OSError):
                os.fstat(fd)
        for action in (
            lambda: gate.prepare_from_parent(installed_payload(manager), manager),
            lambda: gate.release_custody(plan),
        ):
            with self.assertRaisesRegex(OwnedAudioError, "artifact_custody_unavailable"):
                action()
        self.assertEqual(source.releases, 1)
        self.assertTrue(any(thread != threading.get_ident() for thread in source.validations))

    async def test_original_manager_same_uuid_and_plan_source_lineage_cannot_be_retargeted(self):
        from owned_audio import _InstalledOwnedNativeGate, _InstalledAudioSourceProof

        source, policy, gate, actor, manager = self.fixture()
        before = len(source.validations)
        for field in ("model_id", "source_id", "runtime_instance_id"):
            payload = installed_payload(manager)
            payload[field] = "wrong"
            with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
                gate.prepare_from_parent(payload, manager)
        other = _TestModelManager(Devices())
        other._runtime_instance_id = manager.runtime_instance_id
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            gate.prepare_from_parent(installed_payload(manager), other)
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            OwnedAudioActor(other, native_gate=gate)
        with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unqualified"):
            _InstalledOwnedNativeGate._from_source_owner(source)
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            _InstalledAudioSourceProof._from_source_policy(
                policy, source, manager=manager, model_id="retarget", source_id="retarget"
            )
        self.assertEqual(
            before + 1, len(source.validations)
        )  # only attempted source-owner re-retention inspected
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        for action in (copy.copy, copy.deepcopy, pickle.dumps):
            with self.assertRaises(TypeError):
                action(plan._custody.proof)
        plan.model_id = "retarget"
        with self.assertRaises(OwnedAudioError):
            await actor.load(plan)
        plan.model_id = "library/speech"
        foreign = OwnedLoadPlan._from_native_gate(
            gate,
            runtime_instance_id=manager.runtime_instance_id,
            model_id=plan.model_id,
            custody=plan._custody,
        )
        for action in (
            lambda: gate.admit(foreign, manager.runtime_instance_id),
            lambda: gate.cleanup(foreign, None, Device()),
            lambda: gate.release_custody(foreign),
        ):
            with self.assertRaises(OwnedAudioError):
                action()
        self.assertFalse(plan._claimed)
        self.assertFalse(manager.slots)
        self.assertFalse(source.native_calls)
        ready = await actor.load(plan)
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            gate.prepare_from_parent(installed_payload(manager), manager)
        with self.assertRaisesRegex(OwnedAudioError, "invalid_load_plan"):
            gate.cleanup(plan, object(), Device())
        self.assertFalse(source.disposals)
        await actor.unload(ready.slot_ref)
        self.assertEqual(source.releases, 1)

    async def test_actual_cpu_mismatch_refuses_before_claim_and_current_source_drift_before_native(
        self,
    ):
        source, policy, gate, actor, manager = self.fixture()
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        original = manager.device_manager.resolve_device
        manager.device_manager.resolve_device = lambda _: Device("cuda")
        with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unsupported"):
            await actor.load(plan)
        self.assertFalse(plan._claimed)
        self.assertIsNone(gate._proof._lineage)
        self.assertFalse(manager.slots)
        manager.device_manager.resolve_device = original
        (source.root / "model.safetensors").write_bytes(b"changed after source preparation")
        failed = await actor.load(plan)
        self.assertEqual(
            (failed.state, failed.cleanup, failed.error_code),
            ("failed", "confirmed", "load_failed"),
        )
        self.assertFalse(source.native_calls)
        self.assertEqual(source.releases, 1)

    async def test_reader_from_another_owner_refuses_before_native_acquisition(self):
        from loaders.owned_cohere_source import HeldCohereReadSource

        source, policy, gate, actor, manager = self.fixture()
        reader = HeldCohereReadSource._from_members(
            object(), {name: value[0] for name, value in source.members.items()}
        )
        self.addCleanup(reader.close)
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        with patch.object(policy, "model_source", return_value=reader):
            failed = await actor.load(plan)
        self.assertEqual((failed.state, failed.cleanup), ("failed", "confirmed"))
        self.assertFalse(source.native_calls)
        self.assertIsNone(reader.source_owner)
        self.assertEqual(source.releases, 1)

    async def test_incomplete_weights_disposes_returned_model_before_source_release(self):
        source, policy, gate, actor, manager = self.fixture()
        source.incomplete_weights = True
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        failed = await actor.load(plan)
        self.assertEqual((failed.state, failed.cleanup), ("failed", "confirmed"))
        self.assertIn("model", source.disposals[0])
        self.assertIn("weights", source.disposals[0])
        self.assertIn("model_read_source", source.disposals[0])
        self.assertNotIn("model_eval", source.native_calls)
        self.assertEqual(source.releases, 1)
        self.assertFalse(plan._custody.acquisition.objects)

    async def test_lost_constructor_caller_retains_each_completed_stage_until_actual_return(self):
        source, policy, gate, actor, manager = self.fixture()
        source.hold_model = True
        source.model_continue.clear()
        plan = gate.prepare_from_parent(installed_payload(manager), manager)
        caller = asyncio.create_task(actor.load(plan))
        await eventually(source.model_entered.is_set)
        caller.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await caller
        entry = actor._active
        self.assertFalse(source.released)
        self.assertEqual(plan._custody.acquisition.in_flight, "model")
        self.assertIn("feature_extractor", plan._custody.acquisition.objects)
        self.assertIn("tokenizer", plan._custody.acquisition.objects)
        self.assertIn("processor", plan._custody.acquisition.objects)
        self.assertTrue(manager._get_device_lock("cpu").locked())
        source.model_continue.set()
        await eventually(lambda: entry.cleanup == "confirmed")
        self.assertEqual((entry.state, entry.error_code), ("failed", "cancelled"))
        self.assertEqual(source.releases, 1)
        self.assertFalse(plan._custody.acquisition.objects)


async def installed_quarantine_fixture(stage):
    import os
    import tempfile
    from owned_audio import _InstalledOwnedNativeGate

    with tempfile.TemporaryDirectory() as folder:
        manager = _TestModelManager(Devices())
        source = HeldInstalledFixtureSource(Path(folder).resolve(), manager)
        source.fail_stage = stage
        policy = FixedInstalledFixturePolicy()
        policy.source = source
        with (
            patch("owned_audio._INSTALLED_AUDIO_POLICIES", (policy,)),
            patch.dict(sys.modules, fixed_installed_native_modules(source)),
        ):
            gate = _InstalledOwnedNativeGate._from_source_owner(source)
            actor = OwnedAudioActor(manager, native_gate=gate)
            plan = gate.prepare_from_parent(installed_payload(manager), manager)
            if stage == "native_api":
                with patch(
                    "loaders.cohere_asr_loader._native_api",
                    side_effect=RuntimeError("controlled native import uncertainty"),
                ):
                    status = await actor.load(plan)
            elif stage == "tokenizer_options":
                with patch(
                    "loaders.owned_cohere_source.tokenizer_options",
                    side_effect=RuntimeError("controlled special token uncertainty"),
                ):
                    status = await actor.load(plan)
            else:
                status = await actor.load(plan)
            if stage == "dispose":
                status = await actor.unload(status.slot_ref)
            assert (status.state, status.cleanup) == ("cleanup_unconfirmed", "unconfirmed")
            assert actor in _CUSTODIANS and not source.released and source.releases == 0
            assert manager._get_device_lock("cpu").locked()
            assert source.proof._source is source
            os.fstat(source.directory)
            reader = plan._custody.acquisition.objects["model_read_source"]
            assert reader.source_owner is source
            assert (
                source.identity(os.stat(reader.weights_filename()))
                != source.members["model.safetensors"][1]
            )
            with open(reader.weights_filename(), "rb") as weights:
                assert weights.read() == source.members["model.safetensors"][2]
            expected = (
                set(_INSTALLED_STAGES)
                if stage == "dispose"
                else set(_INSTALLED_STAGES[: _INSTALLED_STAGES.index(stage)])
            )
            assert set(plan._custody.acquisition.objects) == expected
            assert plan._custody.acquisition.unknown_allocations is (stage != "dispose")
            actor.close_admission()
            print("installed conditional custody retained: " + stage, flush=True)
            # Stay within the source policy's retained lifetime during the
            # attempted orderly shutdown; only the test parent kills the child.
            await actor._hold_quarantine(actor._active)


class InstalledQuarantinePlumbingTests(unittest.TestCase):
    def test_every_uncertain_native_stage_and_disposal_retains_original_source(self):
        for stage in (*_INSTALLED_STAGES[1:], "dispose"):
            with self.subTest(stage=stage):
                child = subprocess.Popen(
                    [sys.executable, __file__, "--installed-quarantine", stage],
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
                self.assertIn("installed conditional custody retained: " + stage, output)
                self.assertNotIn("Traceback", error)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--installed-quarantine":
        asyncio.run(installed_quarantine_fixture(sys.argv[2]))
    elif len(sys.argv) == 3 and sys.argv[1] == "--quarantine":
        asyncio.run(quarantine_fixture(sys.argv[2]))
    else:
        unittest.main()
