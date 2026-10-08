"""Private use/unload claim boundaries with controlled workers, not native ASR."""

import asyncio
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _TestModelManager
from test_owned_audio import ControlledGate, Devices, eventually
from test_owned_load_admission import ReplyWriter
from test_owned_model_operations import request_value
from test_speech_operations import Gate
from model_manager import SlotState
from native_speech_result import NativeSpeechResult
from owned_audio import OwnedAudioActor, OwnedAudioError, _CUSTODIANS as AUDIO_CUSTODIANS
from owned_model_operations import OwnedModelOperations
from private_owned_channel import PrivateOwnedChannel
from speech_binding import SpeechBindingError
from speech_operations import SpeechOperationError, _CUSTODIANS as SPEECH_CUSTODIANS


class OperationAdmissionTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.manager = _TestModelManager(Devices())
        self.gate = ControlledGate()
        self.actor = OwnedAudioActor(self.manager, native_gate=self.gate)
        self.gate.actor = self.actor
        self.plan = self.gate.prepare(self.manager)
        self.ref = (await self.actor.load(self.plan)).slot_ref
        await asyncio.sleep(0)  # Retire the load runner's done callback.
        self.entry = self.actor._entries[self.ref]
        self.adapter = Gate(result=NativeSpeechResult("controlled", "stop"))
        self.bridge = OwnedModelOperations(
            self.actor,
            model=self.plan.model_id,
            profile=None,
            slot_ref=self.ref,
            adapter=self.adapter,
        )
        self.owner = self.bridge._native
        self.channel = PrivateOwnedChannel(self.actor)
        self.channel._hello = True
        self.channel._bridge = self.bridge
        self.exchange = 0

    async def asyncTearDown(self):
        self.adapter.release.set()
        self.gate.continue_cleanup.set()
        self.assertTrue((await self.owner.drain(3)).custody_complete)
        await eventually(lambda: self.actor._active is None)
        await asyncio.sleep(0)
        if self.entry.state == "ready":
            await self.actor.unload(self.ref)
        self.actor.close_admission()

    def identity(self):
        return {
            "runtime_instance_id": self.ref.runtime_instance_id,
            "slot": {"slot_id": self.ref.slot_id, "load_generation": self.ref.load_generation},
        }

    async def call(self, operation, payload=None):
        self.exchange += 1
        payload = self.identity() if payload is None else payload
        if operation == "use":
            value = request_value()
            value["model"] = self.plan.model_id
            del value["profile"]
            payload = {**payload, "request": value}
        request = {
            "version": 1,
            "exchange_id": self.exchange,
            "operation": operation,
            "payload": payload,
        }
        writer = ReplyWriter()
        await self.channel._handle(writer, request, {"cancel": False, "load_ref": None})
        return writer.reply()

    def assert_slot_retained(self, borrows=0):
        self.assertEqual((self.entry.state, self.entry.cleanup), ("ready", "retained"))
        self.assertEqual(self.entry.borrows, borrows)
        self.assertIs(self.manager.slots[self.ref.slot_id], self.entry.slot)
        self.assertFalse(self.plan._custody.released)
        self.assertEqual(self.gate.cleanups, 0)

    async def assert_use_reusable(self):
        reply = await self.call("use")
        ref = self.channel._handles[reply["result"]["operation_id"]]._native_ref
        self.adapter.release.set()
        self.assertEqual((await self.owner.wait(ref)).state, "completed")
        self.assertEqual(self.adapter.calls, 1)
        self.assert_slot_retained()

    async def test_closed_use_refuses_before_claim(self):
        self.owner.close_admission()
        reply = await self.call("use")
        self.assertEqual(reply["error"], {"code": "admission_closed", "effect": "not_admitted"})
        self.assertFalse(self.owner._entries)
        self.assertEqual(self.adapter.calls, 0)
        self.assert_slot_retained()

    async def test_busy_use_refusal_preserves_original_custody_and_reuse(self):
        first = await self.call("use")
        await eventually(self.adapter.entered.is_set)
        original = self.owner._active
        refused = await self.call("use")
        self.assertEqual(refused["error"], {"code": "runtime_busy", "effect": "not_admitted"})
        self.assertIs(self.owner._active, original)
        self.assertEqual(len(self.owner._entries), 1)
        self.assert_slot_retained(borrows=1)
        self.adapter.release.set()
        self.assertEqual((await self.owner.wait(original.ref)).state, "completed")
        self.assertEqual(self.adapter.calls, 1)
        again = await self.call("use")
        self.assertNotEqual(first["result"]["operation_id"], again["result"]["operation_id"])
        await self.owner.wait(self.owner._active.ref)
        self.assertEqual(self.adapter.calls, 2)

    async def test_slot_refusal_precedes_borrow_and_channel_stays_reusable(self):
        with patch.object(self.entry.slot, "state", SlotState.UNLOADING):
            refused = await self.call("use")
        self.assertEqual(refused["error"], {"code": "model_unavailable", "effect": "not_admitted"})
        self.assertFalse(self.owner._entries)
        self.assert_slot_retained()
        await self.assert_use_reusable()

    async def test_borrow_refusal_rolls_back_registration_and_allows_reuse(self):
        with patch.object(
            self.actor, "acquire", side_effect=SpeechBindingError("artifact_custody_unavailable")
        ):
            refused = await self.call("use")
        self.assertEqual(
            refused["error"], {"code": "artifact_custody_unavailable", "effect": "not_admitted"}
        )
        self.assertFalse(self.owner._entries)
        self.assertFalse(self.owner._requests)
        self.assertIsNone(self.owner._active)
        self.assertNotIn(self.owner, SPEECH_CUSTODIANS)
        self.assert_slot_retained()
        await self.assert_use_reusable()

    async def test_post_borrow_failure_keeps_unknown_even_after_confirmed_release(self):
        bind = self.manager.bind_speech
        bindings = []

        def fail_after_transfer(binding):
            bind(binding)
            bindings.append(binding)
            self.assertIsNotNone(binding.artifact_use)
            self.assertIn(self.owner, SPEECH_CUSTODIANS)
            raise SpeechBindingError("artifact_custody_unavailable")

        with (
            patch.object(self.manager, "bind_speech", side_effect=fail_after_transfer),
            patch.object(self.owner, "_snapshot", side_effect=SpeechOperationError("runtime_busy")),
        ):
            reply = await self.call("use")
        self.assertEqual(reply["error"], {"code": "runtime_busy", "effect": "unknown"})
        self.assertEqual(len(bindings), 1)
        self.assertTrue(bindings[0].released)
        self.assertEqual(len(self.owner._entries), 1)
        self.assertEqual(next(iter(self.owner._entries.values())).cleanup, "confirmed")
        self.assertEqual(self.adapter.calls, 0)
        self.assert_slot_retained()

    async def test_use_post_launch_snapshot_failure_retains_original_and_never_replays(self):
        with patch.object(self.owner, "_snapshot", side_effect=RuntimeError("status failed")):
            reply = await self.call("use")
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "unknown"})
        original = self.owner._active
        self.assertIsNotNone(original.runner)
        self.assertIn(self.owner, SPEECH_CUSTODIANS)
        self.assertFalse(self.channel._handles)
        self.assert_slot_retained(borrows=1)
        await eventually(self.adapter.entered.is_set)
        self.assertTrue(self.manager._get_device_lock("cpu").locked())
        self.assertEqual((self.adapter.calls, len(self.owner._entries)), (1, 1))
        self.adapter.release.set()
        await self.owner.wait(original.ref)
        self.assertEqual(self.adapter.calls, 1)

    async def test_use_channel_status_failure_keeps_admitted_handle_and_custody(self):
        with patch.object(self.bridge, "status", side_effect=RuntimeError("status failed")):
            reply = await self.call("use")
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "unknown"})
        self.assertEqual(len(self.channel._handles), 1)
        self.assert_slot_retained(borrows=1)
        await eventually(self.adapter.entered.is_set)
        self.assertEqual((self.adapter.calls, len(self.owner._entries)), (1, 1))

    async def test_existing_native_request_marks_admitted_before_status_and_never_replays(self):
        with patch.object(self.owner, "start", wraps=self.owner.start) as start:
            await self.call("use")
        body = start.call_args.args[0]
        marks = []
        with patch.object(self.owner, "_snapshot", side_effect=RuntimeError("status failed")):
            with self.assertRaisesRegex(RuntimeError, "status failed"):
                self.owner.start(body, admission=lambda: marks.append(True))
        self.assertEqual(marks, [True])
        self.assertEqual(len(self.owner._entries), 1)
        self.assert_slot_retained(borrows=1)
        await eventually(self.adapter.entered.is_set)
        self.assertEqual(self.adapter.calls, 1)

    async def test_unload_unknown_slot_refuses_without_claim_and_allows_reuse(self):
        payload = self.identity()
        payload["slot"]["load_generation"] = "missing"
        reply = await self.call("unload", payload)
        self.assertEqual(reply["error"], {"code": "slot_replaced", "effect": "not_admitted"})
        self.assert_slot_retained()
        await self.assert_use_reusable()

    async def test_unload_not_ready_refuses_without_claim_and_allows_reuse(self):
        with patch.object(self.entry.slot, "state", SlotState.LOADING):
            reply = await self.call("unload")
        self.assertEqual(
            reply["error"], {"code": "artifact_custody_unavailable", "effect": "not_admitted"}
        )
        self.assert_slot_retained()
        await self.assert_use_reusable()

    async def test_unload_borrow_refusal_preserves_borrow_then_reuses_exact_slot(self):
        borrow = self.actor.acquire(self.ref)
        try:
            reply = await self.call("unload")
            self.assertEqual(reply["error"], {"code": "runtime_busy", "effect": "not_admitted"})
            self.assert_slot_retained(borrows=1)
        finally:
            borrow.release()
        reply = await self.call("unload")
        self.assertEqual(reply["result"]["state"], "retired")
        self.assertEqual((self.gate.cleanups, self.plan._custody.releases), (1, 1))

    async def test_unload_device_refusal_preserves_slot_then_reuses_channel(self):
        lock = self.manager._get_device_lock("cpu")
        await lock.acquire()
        try:
            reply = await self.call("unload")
            self.assertEqual(reply["error"], {"code": "runtime_busy", "effect": "not_admitted"})
            self.assert_slot_retained()
        finally:
            lock.release()
        await self.assert_use_reusable()

    async def test_unload_launch_failure_after_claim_keeps_unknown_and_custody(self):
        launch = self.actor._launch

        def fail_launch(entry, run):
            self.assertEqual(entry.state, "unloading")
            self.assertIs(self.actor._active, entry)
            raise OwnedAudioError("runtime_busy")

        try:
            with patch.object(self.actor, "_launch", side_effect=fail_launch):
                reply = await self.call("unload")
            self.assertEqual(reply["error"], {"code": "runtime_busy", "effect": "unknown"})
            self.assertIn(self.actor, AUDIO_CUSTODIANS)
            self.assertFalse(self.plan._custody.released)
            self.assertEqual(self.gate.cleanups, 0)
            self.assertIs(self.actor._active, self.entry)
        finally:
            # Test-only recovery of this injected fault drains the retained entry;
            # it does not replay the request or admit another unload.
            if self.actor._active is self.entry:
                launch(self.entry, self.actor._run_unload)
        await eventually(lambda: self.entry.cleanup == "confirmed")
        self.assertEqual((self.gate.cleanups, self.plan._custody.releases), (1, 1))

    async def test_unload_observer_allocation_refuses_before_any_slot_mutation(self):
        with patch.object(
            self.actor._loop, "create_future", side_effect=RuntimeError("allocation")
        ):
            reply = await self.call("unload")
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "not_admitted"})
        self.assertIsNone(self.actor._active)
        self.assert_slot_retained()
        await self.assert_use_reusable()

    async def test_unload_post_launch_observation_failure_retains_original_cleanup(self):
        self.gate.continue_cleanup.clear()
        with patch.object(self.actor, "_observe", side_effect=RuntimeError("observe failed")):
            reply = await self.call("unload")
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "unknown"})
        await eventually(self.gate.cleanup_entered.is_set)
        self.assertEqual(self.gate.cleanups, 1)
        self.assertFalse(self.plan._custody.released)
        self.assertIn(self.actor, AUDIO_CUSTODIANS)
        self.assertTrue(self.manager._get_device_lock("cpu").locked())
        self.gate.continue_cleanup.set()
        await eventually(lambda: self.entry.cleanup == "confirmed")
        self.assertEqual((self.gate.cleanups, self.plan._custody.releases), (1, 1))

    async def test_unload_post_cleanup_snapshot_failure_keeps_unknown_without_replay(self):
        with patch.object(self.actor, "_snapshot", side_effect=RuntimeError("status failed")):
            reply = await self.call("unload")
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "unknown"})
        self.assertEqual((self.entry.state, self.entry.cleanup), ("retired", "confirmed"))
        self.assertEqual((self.gate.cleanups, self.plan._custody.releases), (1, 1))
        self.assertNotIn(self.ref.slot_id, self.manager.slots)


if __name__ == "__main__":
    unittest.main()
