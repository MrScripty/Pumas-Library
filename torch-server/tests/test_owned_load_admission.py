"""Private load refusal and post-claim effect evidence with controlled workers."""

import asyncio
import json
from pathlib import Path
import struct
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _TestModelManager
from test_owned_audio import ControlledGate, Devices, eventually
from owned_audio import OwnedAudioActor, OwnedAudioError, _CUSTODIANS
from private_owned_channel import PrivateOwnedChannel


class ReplyWriter:
    def __init__(self):
        self.data = bytearray()

    def write(self, value):
        self.data.extend(value)

    async def drain(self):
        pass

    def reply(self):
        size = struct.unpack("!I", self.data[:4])[0]
        assert len(self.data) == size + 4
        return json.loads(self.data[4:])


class LoadAdmissionTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.manager = _TestModelManager(Devices())
        self.gate = ControlledGate()
        self.actor = OwnedAudioActor(self.manager, native_gate=self.gate)
        self.gate.actor = self.actor
        self.plans = []

        def prepare(payload, manager):
            plan = self.gate.prepare(manager)
            self.plans.append(plan)
            return plan

        self.gate.prepare_from_parent = prepare
        self.channel = PrivateOwnedChannel(self.actor, native_gate=self.gate)
        self.channel._hello = True
        self.exchange = 0

    async def asyncTearDown(self):
        self.gate.continue_load.set()
        self.gate.continue_cleanup.set()
        await eventually(lambda: self.actor._active is None)
        await asyncio.sleep(0)  # Let the completed runner callback retire first.
        for ref, entry in list(self.actor._entries.items()):
            if entry.state == "ready":
                await self.actor.unload(ref)
        self.actor.close_admission()

    async def load(self):
        self.exchange += 1
        request = {
            "version": 1,
            "exchange_id": self.exchange,
            "operation": "load",
            "payload": {
                "runtime_instance_id": self.manager.runtime_instance_id,
                "model_id": "library/speech",
                "source_id": "controlled",
            },
        }
        writer = ReplyWriter()
        await self.channel._handle(writer, request, {"cancel": False, "load_ref": None})
        return writer.reply()

    async def test_closed_refusal_precedes_claim_and_launch(self):
        self.actor.close_admission()
        reply = await self.load()
        self.assertEqual(reply["error"], {"code": "admission_closed", "effect": "not_admitted"})
        self.assertFalse(self.plans[-1]._claimed)
        self.assertFalse(self.manager.slots)
        self.assertEqual(self.gate.loads, 0)

    async def test_busy_refusal_preserves_existing_slot_and_channel_reuse(self):
        first = await self.load()
        ref = next(iter(self.actor._entries))
        self.assertEqual(first["result"]["state"], "ready")
        refused = await self.load()
        self.assertEqual(refused["error"], {"code": "runtime_busy", "effect": "not_admitted"})
        self.assertFalse(self.plans[-1]._claimed)
        self.assertEqual(self.gate.loads, 1)
        self.assertEqual(self.actor.status(ref).state, "ready")
        await self.actor.unload(ref)
        self.assertEqual((await self.load())["result"]["state"], "ready")
        self.assertEqual(self.gate.loads, 2)

    async def test_device_refusal_precedes_claim_and_native_work(self):
        with patch.object(self.manager.device_manager, "resolve_device", side_effect=ValueError):
            reply = await self.load()
        self.assertEqual(
            reply["error"], {"code": "load_admission_failed", "effect": "not_admitted"}
        )
        self.assertFalse(self.plans[-1]._claimed)
        self.assertFalse(self.manager.slots)
        self.assertEqual(self.gate.loads, 0)

    async def test_failure_during_launch_after_claim_keeps_unknown_effect(self):
        launch = self.actor._launch
        self.gate.continue_load.clear()

        def fail_launch(entry, run):
            # Fault at the real claim boundary, before a runner is available.
            self.assertTrue(entry.plan._claimed)
            raise OwnedAudioError("load_admission_failed")

        with patch.object(self.actor, "_launch", side_effect=fail_launch):
            reply = await self.load()
        entry = self.actor._active
        try:
            self.assertEqual(reply["error"], {"code": "load_admission_failed", "effect": "unknown"})
            self.assertTrue(entry.plan._claimed)
            self.assertFalse(entry.plan._custody.released)
            self.assertIs(self.actor._entries[entry.ref], entry)
            self.assertIn(self.actor, _CUSTODIANS)
            self.assertEqual(self.gate.loads, 0)
        finally:
            # Test-only recovery of the injected launch failure, never replay a
            # channel request or claim a second plan. Drain the retained entry.
            launch(entry, self.actor._run_load)
            self.gate.continue_load.set()

    async def test_status_failure_after_launch_keeps_unknown_and_no_replay(self):
        self.gate.continue_load.clear()
        with patch.object(
            self.actor, "_snapshot", side_effect=RuntimeError("controlled status failure")
        ):
            reply = await self.load()
        entry = self.actor._active
        await eventually(self.gate.entered.is_set)
        self.assertEqual(reply["error"], {"code": "operation_failed", "effect": "unknown"})
        self.assertTrue(entry.plan._claimed)
        self.assertFalse(entry.plan._custody.released)
        self.assertIs(self.actor._entries[entry.ref], entry)
        self.assertIn(self.actor, _CUSTODIANS)
        self.assertEqual(self.gate.loads, 1)
        self.assertEqual(len(self.plans), 1)


if __name__ == "__main__":
    unittest.main()
