"""Exact-generation and retained-authority contract tests, using no real custody."""

import asyncio
from dataclasses import replace
from pathlib import Path
import sys
import unittest
from unittest.mock import AsyncMock, Mock, patch
from uuid import UUID, uuid4

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _FakeDeviceManager, _TestModelManager
from test_speech_operations import Gate, Manager, encode, eventually, loaded_manager, payload
from loaders.cohere_asr_loader import COHERE_ASR
from model_manager import LoadedModel, SlotState
from speech_binding import SpeechBindingError, SpeechSlotRef
from speech_fixtures import bind_fixture
from speech_operations import SpeechOperationError, SpeechOperationOwner


class SlotBindingTests(unittest.IsolatedAsyncioTestCase):
    async def finish_owner(self, owner, gate):
        gate.release.set()
        self.assertTrue((await owner.drain(3)).custody_complete)
        await asyncio.sleep(0)

    def owner(self, manager):
        gate = Gate()
        owner = SpeechOperationOwner(manager, adapter=gate)
        self.addAsyncCleanup(self.finish_owner, owner, gate)
        return owner, gate

    def assert_code(self, code, action):
        with self.assertRaises(SpeechOperationError) as raised:
            action()
        self.assertEqual(raised.exception.code, code)

    async def test_admission_construction_failures_transfer_no_borrow(self):
        for target in (
            "speech_operations.uuid4",
            "speech_operations._Entry",
            "model_manager._BoundSpeechSlot",
        ):
            with self.subTest(target=target):
                manager = Manager()
                owner, gate = self.owner(manager)
                body = encode(payload(owner))
                with patch(target, side_effect=MemoryError("private preparation failure")):
                    self.assert_code("admission_failed", lambda: owner.start(body))
                self.assertFalse(manager._speech_artifact_authority.borrows)
                self.assertFalse(owner._entries)
                self.assertIsNone(owner._active)
                self.assertEqual(gate.calls, 0)
                self.assertTrue(owner.close_admission().custody_complete)
        manager = Manager()
        owner, gate = self.owner(manager)
        body = encode(payload(owner))
        with patch.object(
            owner._loop, "create_future", side_effect=MemoryError("private future failure")
        ):
            self.assert_code("admission_failed", lambda: owner.start(body))
        self.assertFalse(manager._speech_artifact_authority.borrows)
        self.assertTrue(owner.close_admission().custody_complete)

    async def test_post_transfer_binding_failure_releases_only_proven_nonstarted_borrow(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        acquire = manager.bind_speech

        def fail_after_transfer(binding):
            self.assertIs(owner._active.binding, binding)
            self.assertIs(owner._entries[owner._active.ref.operation_id], owner._active)
            acquire(binding)
            raise RuntimeError("private binding failure")

        manager.bind_speech = fail_after_transfer
        with patch("speech_operations.Thread") as thread:
            started = owner.start(encode(payload(owner)))
            self.assertEqual(started.state, "failed")
            self.assertEqual(started.operation_diagnostic.code, "binding_failed")
            self.assertEqual(started.cleanup, "confirmed")
            self.assertNotIn("private binding failure", repr(started))
            thread.assert_not_called()
        self.assertEqual(len(manager._speech_artifact_authority.borrows), 1)
        self.assertTrue(manager._speech_artifact_authority.borrows[0].released)
        self.assertTrue(owner.close_admission().custody_complete)
        self.assertEqual(gate.calls, 0)
        self.assertIsNone(owner._entries[started.operation_ref.operation_id].binding)

    async def test_eager_completed_refusal_preserves_exact_cleanup_after_factory_error(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        # A device-busy refusal can settle without an actual suspension or worker.
        await manager.lock.acquire()
        loop = asyncio.get_running_loop()
        previous = loop.get_task_factory()
        tasks = []

        def factory(loop, coroutine, **kwargs):
            tasks.append(asyncio.Task(coroutine, loop=loop, eager_start=True, **kwargs))
            raise RuntimeError("private task factory failure")

        body = encode(payload(owner))
        loop.set_task_factory(factory)
        try:
            started = owner.start(body)
        finally:
            loop.set_task_factory(previous)
            manager.lock.release()
        self.assertTrue(tasks[0].done())
        self.assertEqual(started.state, "failed")
        self.assertEqual(started.operation_diagnostic.code, "runtime_busy")
        self.assertEqual(started.owner_startup_diagnostic.code, "owner_start_failed")
        self.assertEqual(started.cleanup, "confirmed")
        self.assertEqual(owner.start(body), started)
        self.assertTrue(manager._speech_artifact_authority.borrows[0].released)
        self.assertEqual(gate.calls, 0)
        self.assertNotIn("private task factory failure", repr(started))

    async def test_production_default_refuses_admission_and_native_work(self):
        manager = _TestModelManager(_FakeDeviceManager())
        slot = await manager.load("/fixture", "speech", model_type=COHERE_ASR)
        ref = manager.speech_slot_ref(slot.slot_id)
        owner, gate = self.owner(manager)
        with patch("speech_operations.Thread") as thread:
            self.assert_code(
                "artifact_authority_unavailable",
                lambda: owner.start(encode(payload(owner, ref=ref))),
            )
            await asyncio.sleep(0)
            thread.assert_not_called()
        self.assertFalse(owner._entries)
        self.assertIsNone(owner._active)
        self.assertEqual(gate.calls, 0)
        self.assertFalse(manager._get_device_lock(slot.device).locked())
        # Private identities are not a new readiness field in public schemas.
        self.assertNotIn("load_generation", slot.to_dict())
        self.assertNotIn("runtime_instance_id", slot.to_dict())
        with self.assertRaisesRegex(SpeechBindingError, "invalid_slot_ref"):
            bind_fixture(manager, "speech")

    async def test_one_owner_is_runtime_wide_across_models_and_devices(self):
        manager, first = await loaded_manager()
        other = await manager.load("/second", "another", model_type=COHERE_ASR)
        # Distinct device lock is insufficient to bypass the runtime-wide limit.
        other.device = "other-test-device"
        ref = manager._speech_artifact_authority.attest_fixture(
            manager.speech_slot_ref(other.slot_id)
        )
        owner, gate = self.owner(manager)
        with self.assertRaisesRegex(RuntimeError, "already has"):
            SpeechOperationOwner(manager)
        first_status = owner.start(encode(payload(owner)))
        await eventually(gate.entered.is_set)
        self.assert_code("runtime_busy", lambda: owner.start(encode(payload(owner, ref=ref))))
        gate.release.set()
        await owner.wait(first_status.operation_ref)
        second = owner.start(encode(payload(owner, ref=ref)))
        self.assertEqual(second.operation_ref.slot_ref, ref)
        self.assertEqual((await owner.wait(second.operation_ref)).state, "completed")
        owner.close_admission()
        with self.assertRaisesRegex(RuntimeError, "already has"):
            SpeechOperationOwner(manager)

    async def test_generation_digest_conflicts_even_when_request_audio_are_identical(self):
        manager, old_slot = await loaded_manager()
        owner, gate = self.owner(manager)
        body = encode(payload(owner))
        started = owner.start(body)
        gate.release.set()
        done = await owner.wait(started.operation_ref)
        old_ref = started.operation_ref.slot_ref
        await manager.unload(old_slot.slot_id)
        replacement = await manager.load("/fixture", "speech", model_type=COHERE_ASR)
        new_ref = manager._speech_artifact_authority.attest_fixture(
            manager.speech_slot_ref(replacement.slot_id)
        )
        self.assertNotEqual(old_ref.load_generation, new_ref.load_generation)
        self.assertEqual(owner.start(body), done)
        changed = payload(owner, ref=new_ref, request_id=done.request_id)
        self.assert_code("request_conflict", lambda: owner.start(encode(changed)))
        self.assert_code("slot_replaced", lambda: owner.start(encode(payload(owner, ref=old_ref))))
        self.assertEqual(owner.status(started.operation_ref).operation_ref.slot_ref, old_ref)
        forged = replace(started.operation_ref, slot_ref=new_ref)
        for action in (owner.status, owner.cancel):
            self.assert_code("invalid_operation_ref", lambda: action(forged))
        fresh = owner.start(encode(payload(owner, ref=new_ref)))
        self.assertEqual((await owner.wait(fresh.operation_ref)).state, "completed")
        self.assertEqual(gate.calls, 2)

    async def test_unload_reload_in_admission_to_worker_gap_never_uses_successor(self):
        manager, old = await loaded_manager()
        owner, gate = self.owner(manager)
        request = encode(payload(owner))
        begin_native = asyncio.Event()
        run = owner._run

        async def delayed_run(entry):
            await begin_native.wait()
            await run(entry)

        owner._run = delayed_run
        started = owner.start(request)
        borrow = owner._active.binding.artifact_use
        # Hold the admitted runner while real manager unload/reload completes.
        # A name-only lookup here would subsequently acquire the READY successor.
        await manager.unload(old.slot_id)
        replacement_task = asyncio.create_task(
            manager.load("/fixture", "speech", model_type=COHERE_ASR)
        )
        replacement = await replacement_task
        self.assertEqual(replacement.state, SlotState.READY)
        self.assertEqual(replacement.model_name, old.model_name)
        begin_native.set()
        done = await owner.wait(started.operation_ref)
        self.assertEqual(done.state, "failed")
        self.assertEqual(done.operation_diagnostic.code, "slot_replaced")
        self.assertEqual(gate.calls, 0)
        self.assertTrue(borrow.released)
        self.assertEqual(owner.start(request), done)
        self.assertNotEqual(
            replacement.load_generation, started.operation_ref.slot_ref.load_generation
        )

    async def test_same_slot_id_replacement_in_worker_gap_cannot_hijack_old_ref(self):
        for same_generation in (False, True):
            with self.subTest(same_generation=same_generation):
                manager = Manager()
                owner, gate = self.owner(manager)
                started = owner.start(encode(payload(owner)))
                original = manager.slot
                replacement = replace(
                    original,
                    load_generation=original.load_generation if same_generation else str(uuid4()),
                    _loaded=LoadedModel(object(), object(), "cpu", COHERE_ASR),
                )
                manager.slots[original.slot_id] = replacement
                done = await owner.wait(started.operation_ref)
                self.assertEqual(done.operation_diagnostic.code, "slot_replaced")
                self.assertEqual(gate.calls, 0)
                self.assertIs(manager.slots[original.slot_id], replacement)

    async def test_authority_receipt_and_slot_are_revalidated_under_device_lock(self):
        for change in ("authority", "state", "loaded", "device", "generation"):
            with self.subTest(change=change):
                manager = Manager()
                owner, gate = self.owner(manager)
                started = owner.start(encode(payload(owner)))
                borrow = owner._active.binding.artifact_use
                validate = borrow.validate

                def assert_locked(ref):
                    self.assertTrue(manager.lock.locked())
                    validate(ref)

                borrow.validate = Mock(side_effect=assert_locked)
                if change == "authority":
                    manager._speech_artifact_authority.attested.clear()
                elif change == "state":
                    manager.slot.state = SlotState.UNLOADING
                elif change == "loaded":
                    manager.slot._loaded = object()
                elif change == "device":
                    manager.slot.device = "changed"
                else:
                    manager.slot.load_generation = str(uuid4())
                done = await owner.wait(started.operation_ref)
                self.assertEqual(done.state, "failed")
                self.assertEqual(gate.calls, 0)
                self.assertTrue(borrow.released)
                if change == "authority":
                    borrow.validate.assert_called_once_with(started.operation_ref.slot_ref)
                    self.assertEqual(done.operation_diagnostic.code, "artifact_custody_unavailable")

    async def test_slot_replacement_during_device_lock_acquisition_is_rechecked(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        original = manager.slot

        class ReplacingLock(asyncio.Lock):
            async def acquire(self):
                await super().acquire()
                manager.slots[original.slot_id] = replace(original, load_generation=str(uuid4()))
                return True

        manager._device_locks[original.device] = ReplacingLock()
        started = owner.start(encode(payload(owner)))
        done = await owner.wait(started.operation_ref)
        self.assertEqual(done.operation_diagnostic.code, "slot_replaced")
        self.assertEqual(gate.calls, 0)
        self.assertFalse(manager._device_locks[original.device].locked())

    async def test_released_or_foreign_binding_cannot_acquire_a_device(self):
        manager = Manager()
        unused = manager.prepare_speech(manager.fixture_ref)
        unused.release()
        with self.assertRaisesRegex(SpeechBindingError, "invalid_slot_ref"):
            manager.bind_speech(unused)
        binding = bind_fixture(manager, manager.fixture_ref)
        binding.release()
        with self.assertRaisesRegex(SpeechBindingError, "slot_replaced"):
            async with manager.speech_lease(binding):
                self.fail("released binding reused")
        other = Manager()
        with self.assertRaisesRegex(SpeechBindingError, "invalid_slot_ref"):
            async with other.speech_lease(binding):
                self.fail("foreign binding accepted")
        self.assertFalse(manager.lock.locked())
        self.assertFalse(other.lock.locked())

    async def test_slot_envelope_requires_exact_fields_and_canonical_generation(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        for field, value in (
            ("slot_id", None),
            ("slot_id", []),
            ("slot_id", ""),
            ("slot_id", "x" * 129),
            ("slot_id", "é"),
            ("load_generation", None),
            ("load_generation", "bad"),
            ("load_generation", str(uuid4()).upper()),
            ("load_generation", uuid4().hex),
        ):
            request = payload(owner)
            request["slot"][field] = value
            self.assert_code("invalid_slot_ref", lambda: owner.start(encode(request)))
        for slot in (None, [], {"slot_id": "fixture"}, {"model_name": "fixture"}):
            request = payload(owner)
            request["slot"] = slot
            self.assert_code("invalid_fields", lambda: owner.start(encode(request)))
        self.assertEqual(gate.calls, 0)
        self.assertFalse(manager._speech_artifact_authority.borrows)

    async def test_cancellation_retains_artifact_borrow_until_worker_exit(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        started = owner.start(encode(payload(owner)))
        borrow = owner._active.binding.artifact_use
        await eventually(gate.entered.is_set)
        self.assertEqual(borrow.validations, 1)
        for _ in range(3):
            owner.cancel(started.operation_ref)
            owner._active.runner.cancel()
            await asyncio.sleep(0)
            self.assertFalse(borrow.released)
            self.assertTrue(manager.lock.locked())
        gate.release.set()
        self.assertEqual((await owner.wait(started.operation_ref)).state, "cancelled")
        self.assertTrue(borrow.released)

    async def test_generation_never_reused_and_colliding_slot_ids_do_not_overwrite(self):
        manager = _TestModelManager(_FakeDeviceManager())
        first = await manager.load("/first", "first")
        # A live public slot-ID collision retries without overwriting the slot.
        with patch(
            "model_manager.uuid.uuid4", side_effect=[UUID(first.slot_id + "0" * 24), uuid4()]
        ):
            second = await manager.load("/second", "second")
        self.assertIs(manager.slots[first.slot_id], first)
        self.assertNotEqual(first.slot_id, second.slot_id)
        self.assertEqual(UUID(first.load_generation).int, 1)
        self.assertEqual(UUID(second.load_generation).int, 2)
        await manager.unload(first.slot_id)
        third = await manager.load("/third", "third")
        self.assertEqual(UUID(third.load_generation).int, 3)
        self.assertEqual(manager.get_model_for_inference("second"), second._loaded)
        self.assertEqual(set(manager.list_model_names()), {"second", "third"})
        self.assertFalse(hasattr(manager, "_load_generations"))

    async def test_generation_exhaustion_refuses_without_wrapping_or_reserving(self):
        manager = _TestModelManager(_FakeDeviceManager())
        manager._last_load_generation = (1 << 128) - 2
        final = await manager.load("/final", "final")
        self.assertEqual(UUID(final.load_generation).int, (1 << 128) - 1)
        await manager.unload(final.slot_id)
        with self.assertRaisesRegex(RuntimeError, "generation space is exhausted"):
            await manager.load("/no-wrap", "no-wrap")
        self.assertEqual(manager._last_load_generation, (1 << 128) - 1)
        self.assertFalse(manager.slots)

    async def test_failed_load_reservation_also_consumes_its_generation(self):
        manager = _TestModelManager(_FakeDeviceManager())
        with patch.object(manager, "_load_model", AsyncMock(side_effect=RuntimeError("fixture"))):
            with patch("model_manager.logger"), self.assertRaises(RuntimeError):
                await manager.load("/failed", "failed")
        failed = next(iter(manager.slots.values()))
        self.assertEqual(UUID(failed.load_generation).int, 1)
        await manager.unload(failed.slot_id)
        loaded = await manager.load("/next", "next")
        self.assertEqual(UUID(loaded.load_generation).int, 2)

    async def test_authority_codes_are_clamped_at_admission_and_native_boundaries(self):
        for secret_code in ("PRIVATE-PCM-transcript", ["private", "unhashable"]):
            with self.subTest(code=secret_code):
                manager = Manager()
                owner, gate = self.owner(manager)
                manager._speech_artifact_authority.acquire = Mock(
                    side_effect=SpeechBindingError(secret_code)
                )
                self.assert_code(
                    "artifact_custody_unavailable", lambda: owner.start(encode(payload(owner)))
                )
                self.assertFalse(owner._entries)
                self.assertEqual(gate.calls, 0)

                manager = Manager()
                owner, gate = self.owner(manager)
                started = owner.start(encode(payload(owner)))
                owner._active.binding.artifact_use.validate = Mock(
                    side_effect=SpeechBindingError(secret_code)
                )
                done = await owner.wait(started.operation_ref)
                self.assertEqual(done.operation_diagnostic.code, "artifact_custody_unavailable")
                self.assertNotIn("private", repr(done).lower())
                self.assertEqual(gate.calls, 0)

    async def test_foreign_runtime_and_unattested_exact_slot_refuse_before_admission(self):
        manager = Manager()
        owner, gate = self.owner(manager)
        foreign = replace(manager.fixture_ref, runtime_instance_id=str(uuid4()))
        self.assert_code(
            "runtime_replaced", lambda: owner.start(encode(payload(owner, ref=foreign)))
        )
        manager._speech_artifact_authority.attested.clear()
        self.assert_code(
            "artifact_custody_unavailable", lambda: owner.start(encode(payload(owner)))
        )
        self.assertEqual(gate.calls, 0)
        self.assertFalse(owner._entries)
        with self.assertRaisesRegex(SpeechBindingError, "runtime_replaced"):
            bind_fixture(manager, foreign)
        invalid = SpeechSlotRef(manager.runtime_instance_id, "missing", str(uuid4()))
        with self.assertRaisesRegex(SpeechBindingError, "slot_replaced"):
            bind_fixture(manager, invalid)


if __name__ == "__main__":
    unittest.main()
