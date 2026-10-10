"""Explicit local-attempt plumbing with controlled objects, never real ASR evidence."""

import asyncio
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from test_owned_audio import Devices, HeldInstalledFixtureSource, fixed_installed_native_modules
from test_model_manager import _TestModelManager
from installed_cohere_source import _InstalledCohereSourceOwner, _experimental_policy_for
from owned_audio import OwnedAudioError, _INSTALLED_AUDIO_POLICIES, _InstalledOwnedNativeGate
from private_owned_channel import _create_bootstrap_private_owned_channel


async def controlled_attempt():
    with tempfile.TemporaryDirectory() as folder:
        manager = _TestModelManager(Devices())
        fixture = HeldInstalledFixtureSource(Path(folder), manager)
        with patch.dict(sys.modules, fixed_installed_native_modules(fixture)):
            channel = _create_bootstrap_private_owned_channel(
                manager, fixture.directory, "local/cohere", "selected", experimental=True
            )
            actor, gate = channel.actor, channel.actor._gate
            source = gate._proof._source
            replies = []

            async def reply(*args, **kwargs):
                replies.append(kwargs)

            with patch.object(channel, "_reply", reply):
                await channel._handle(None, {"operation": "hello", "payload": {}}, {})
            assert replies == [
                {
                    "result": {
                        "runtime_instance_id": manager.runtime_instance_id,
                        "production_available": False,
                    }
                }
            ]
            payload = {
                "runtime_instance_id": manager.runtime_instance_id,
                "model_id": source.model_id,
                "source_id": source.source_id,
            }
            for key in ("experimental", "policy", "production_available"):
                try:
                    gate.prepare_from_parent({**payload, key: True}, manager)
                except OwnedAudioError as error:
                    assert error.code == "invalid_load_plan"
                else:
                    raise AssertionError("wire selected execution mode")
            plan = gate.prepare_from_parent(payload, manager)
            status = await actor.load(plan)
            assert status.state == "ready" and not status.production_available
            assert fixture.native_calls[-1] == "model_eval"
            status = await actor.unload(status.slot_ref)
            assert (status.state, status.cleanup) == ("cleanup_unconfirmed", "unconfirmed")
            assert not source._closed and not source._reader._closed
            os.fstat(source._root)
            assert plan._custody.disposal is not None and not plan._custody.disposal._finished
            assert plan._custody.acquisition.objects["model_read_source"] is source._reader
            print("experimental load succeeded; exact child drain required", flush=True)
            await actor._hold_quarantine(actor._active)


@unittest.skipUnless(sys.platform == "linux", "Linux held source and seals")
class ExperimentalCohereTests(unittest.TestCase):
    def test_ordinary_source_cannot_select_experimental_policy(self):
        self.assertEqual(_INSTALLED_AUDIO_POLICIES, ())
        with tempfile.TemporaryDirectory() as folder:
            manager = _TestModelManager(Devices())
            fixture = HeldInstalledFixtureSource(Path(folder), manager)
            source = _InstalledCohereSourceOwner._capture(
                manager, fixture.directory, "model", "part"
            )
            try:
                self.assertIsNone(_experimental_policy_for(source))
                with self.assertRaisesRegex(OwnedAudioError, "native_runtime_unqualified"):
                    _InstalledOwnedNativeGate._from_source_owner(source)
            finally:
                source.close_unclaimed()
                fixture.abort_unclaimed_fixture()

    def test_explicit_attempt_loads_but_never_claims_native_disposal(self):
        child = subprocess.Popen(
            [sys.executable, __file__, "--attempt"],
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
        self.assertIn("experimental load succeeded; exact child drain required", output)
        self.assertNotIn("Traceback", error)


if __name__ == "__main__":
    if sys.argv[1:] == ["--attempt"]:
        asyncio.run(controlled_attempt())
    else:
        unittest.main()
