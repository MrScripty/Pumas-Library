"""Default private adapter wiring with synthetic tokens, not native qualification."""

import asyncio
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_model_manager import _TestModelManager
from test_cohere_asr_loader import Inputs, fake_torch
from test_owned_audio import ControlledGate, Devices, eventually
from test_owned_load_admission import ReplyWriter
from test_owned_model_operations import request_value
from loaders.cohere_asr_loader import transcribe, transcribe_detailed
from native_speech_result import NativeSpeechResult
from owned_audio import OwnedAudioActor
from owned_model_operations import OwnedModelOperations, OwnedOperationError
from private_owned_channel import PrivateOwnedChannel
from speech_operations import SpeechOperationOwner


class DefaultAdapterTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.manager = _TestModelManager(Devices())
        self.gate = ControlledGate()
        self.actor = OwnedAudioActor(self.manager, native_gate=self.gate)
        self.gate.actor = self.actor
        self.model = Mock(device="cpu", dtype="float32")
        self.model.config = SimpleNamespace(is_encoder_decoder=True)
        self.model.generation_config = SimpleNamespace(
            eos_token_id=0, decoder_start_token_id=7, forced_eos_token_id=None
        )
        self.processor = Mock(return_value=Inputs())
        self.processor.decode.return_value = "  Controlled transcript.  "
        load = self.gate.load

        def controlled_load(plan, device):
            loaded = load(plan, device)
            loaded.model, loaded.tokenizer = self.model, self.processor
            return loaded

        self.enterContext(patch.object(self.gate, "load", side_effect=controlled_load))
        self.gate.prepare_from_parent = lambda payload, manager: self.gate.prepare(manager)
        # Exercise the actual loader wrappers and terminal-token validation;
        # only native packages and model/processor execution are substitutes.
        self.enterContext(
            patch("loaders.cohere_asr_loader._native_api", return_value=(None, None, object, list))
        )
        self.enterContext(patch.dict(sys.modules, {"torch": fake_torch()}))
        self.channel = PrivateOwnedChannel(self.actor, native_gate=self.gate)
        self.channel._hello = True
        self.exchange = 0

    async def asyncTearDown(self):
        if self.manager._speech_owner is not None:
            self.assertTrue((await self.manager._speech_owner.drain(3)).custody_complete)
        await eventually(lambda: self.actor._active is None)
        await asyncio.sleep(0)
        for ref, entry in list(self.actor._entries.items()):
            if entry.state == "ready":
                await self.actor.unload(ref)
        self.actor.close_admission()

    async def call(self, operation, payload):
        self.exchange += 1
        writer = ReplyWriter()
        request = {
            "version": 1,
            "exchange_id": self.exchange,
            "operation": operation,
            "payload": payload,
        }
        await self.channel._handle(writer, request, {"cancel": False, "load_ref": None})
        reply = writer.reply()
        self.assertNotIn("error", reply)
        return reply["result"]

    async def load(self):
        result = await self.call(
            "load",
            {
                "runtime_instance_id": self.manager.runtime_instance_id,
                "model_id": "library/speech",
                "source_id": "controlled",
            },
        )
        self.assertEqual((result["state"], result["cleanup"]), ("ready", "retained"))
        self.assertFalse(self.actor.status(self.channel._bridge._slot_ref).production_available)
        self.identity = {
            "runtime_instance_id": self.manager.runtime_instance_id,
            "slot": result["slot"],
        }

    async def use(self):
        request = request_value()
        request["model"] = "library/speech"
        del request["profile"]
        started = await self.call("use", {**self.identity, "request": request})
        settled = await self.call(
            "status",
            {
                **self.identity,
                "operation_id": started["operation_id"],
                "wait_for_settlement": True,
            },
        )
        self.assertEqual(settled["operation_id"], started["operation_id"])
        return settled

    async def assert_default_result(self, tokens, reason):
        self.model.generate.return_value = [tokens]
        await self.load()
        settled = await self.use()
        self.assertEqual(
            (settled["state"], settled["cleanup"], settled["finish_reason"], settled["text"]),
            ("completed", "confirmed", reason, "Controlled transcript."),
        )
        self.assertIsNone(settled["error_code"])
        self.assertIs(self.manager._speech_owner._adapter, transcribe_detailed)
        self.model.generate.assert_called_once()
        self.assertEqual(self.model.generate.call_args.kwargs["max_new_tokens"], 512)
        self.assertNotIn("audio_chunk_index", self.model.generate.call_args.kwargs)
        self.assertEqual(self.actor.status(self.channel._bridge._slot_ref).outstanding_borrows, 0)

    async def test_default_adapter_preserves_checked_stop_evidence_on_channel(self):
        await self.assert_default_result([7, 3, 0], "stop")

    async def test_default_adapter_preserves_checked_length_evidence_on_channel(self):
        await self.assert_default_result([7, *([1] * 512)], "length")

    async def test_default_adapter_refuses_missing_terminal_evidence_before_decode(self):
        self.model.generate.return_value = [[7, 3]]
        await self.load()
        settled = await self.use()
        self.assertEqual((settled["state"], settled["cleanup"]), ("failed", "confirmed"))
        self.assertEqual(settled["error_code"], "runtime_unsupported")
        self.assertIsNone(settled["text"])
        self.assertIsNone(settled["finish_reason"])
        self.processor.decode.assert_not_called()

    async def test_default_owner_is_reused_across_exact_unload_and_reload(self):
        await self.assert_default_result([7, 3, 0], "stop")
        original = self.manager._speech_owner
        first_ref = self.channel._bridge._slot_ref
        await self.call("unload", self.identity)
        await self.load()
        self.assertIs(self.manager._speech_owner, original)
        self.assertIs(self.channel._bridge._native, original)
        self.assertNotEqual(self.channel._bridge._slot_ref, first_ref)
        self.model.generate.return_value = [[7, *([1] * 512)]]
        self.assertEqual((await self.use())["finish_reason"], "length")
        self.assertEqual(self.model.generate.call_count, 2)

    async def test_existing_plain_owner_refuses_implicit_reuse_without_changing_custody(self):
        original = SpeechOperationOwner(self.manager)
        self.assertIs(original._adapter, transcribe)
        plan = self.gate.prepare(self.manager)
        ready = await self.actor.load(plan)
        with self.assertRaisesRegex(OwnedOperationError, "^capability_unavailable$"):
            OwnedModelOperations(
                self.actor, model=plan.model_id, profile=None, slot_ref=ready.slot_ref
            )
        self.assertIs(self.manager._speech_owner, original)
        self.assertIs(original._adapter, transcribe)
        self.assertFalse(original._entries)
        self.assertEqual(self.actor.status(ready.slot_ref), ready)
        self.assertEqual(ready.outstanding_borrows, 0)
        self.assertFalse(plan._custody.released)
        self.assertEqual((self.gate.loads, self.gate.cleanups), (1, 0))
        self.model.generate.assert_not_called()

    async def test_existing_detailed_owner_accepts_implicit_reuse(self):
        original = SpeechOperationOwner(self.manager, adapter=transcribe_detailed)
        await self.assert_default_result([7, 3, 0], "stop")
        self.assertIs(self.manager._speech_owner, original)
        self.assertIs(self.channel._bridge._native, original)

    async def test_explicit_adapter_identity_reuse_requires_matching_explicit_adapter(self):
        explicit = Mock(return_value=NativeSpeechResult("Explicit transcript.", "stop"))
        self.channel = PrivateOwnedChannel(self.actor, native_gate=self.gate, adapter=explicit)
        self.channel._hello = True
        await self.load()
        original = self.manager._speech_owner
        self.assertIs(original._adapter, explicit)
        with self.assertRaisesRegex(OwnedOperationError, "^capability_unavailable$"):
            OwnedModelOperations(
                self.actor,
                model="library/speech",
                profile=None,
                slot_ref=self.channel._bridge._slot_ref,
            )
        matched = OwnedModelOperations(
            self.actor,
            model="library/speech",
            profile=None,
            slot_ref=self.channel._bridge._slot_ref,
            adapter=explicit,
        )
        self.assertIs(matched._native, original)
        self.assertIs(matched._native._adapter, explicit)
        self.assertEqual((await self.use())["text"], "Explicit transcript.")
        await self.call("unload", self.identity)
        await self.load()
        self.assertIs(self.channel._bridge._native, original)
        self.assertIs(original._adapter, explicit)
        self.assertEqual((await self.use())["finish_reason"], "stop")
        self.assertEqual(explicit.call_count, 2)
        self.model.generate.assert_not_called()

    async def test_mismatched_explicit_adapter_refuses_without_replacing_owner(self):
        await self.load()
        original = self.manager._speech_owner
        other = Mock(return_value=NativeSpeechResult("Unexpected.", "stop"))
        with self.assertRaisesRegex(OwnedOperationError, "^capability_unavailable$"):
            OwnedModelOperations(
                self.actor,
                model="library/speech",
                profile=None,
                slot_ref=self.channel._bridge._slot_ref,
                adapter=other,
            )
        self.assertIs(self.manager._speech_owner, original)
        self.assertIs(self.channel._bridge._native, original)
        self.assertFalse(original._entries)
        other.assert_not_called()


if __name__ == "__main__":
    unittest.main()
