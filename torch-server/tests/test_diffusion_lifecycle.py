"""Lease evidence only; real GPU and browser acceptance is recorded separately."""

import asyncio
import threading
import unittest

from test_model_manager import _FakeDeviceManager, _TestModelManager
from diffusion import GenerationCancelled, NUNCHAKU_Z_IMAGE


class DiffusionLeaseTests(unittest.IsolatedAsyncioTestCase):
    async def test_busy_generation_rejects_unload_and_second_generation(self):
        manager = _TestModelManager(_FakeDeviceManager())
        slot = await manager.load("/fixture", "image", model_type=NUNCHAKU_Z_IMAGE)
        async with manager.image_lease("image"):
            with self.assertRaisesRegex(RuntimeError, "busy"):
                await manager.unload(slot.slot_id)
            with self.assertRaisesRegex(RuntimeError, "busy"):
                async with manager.image_lease("image"):
                    self.fail("Second request entered the device")
        await manager.unload(slot.slot_id)
        self.assertEqual(manager.list_slots(), [])

    async def test_text_model_cannot_acquire_image_lease(self):
        manager = _TestModelManager(_FakeDeviceManager())
        await manager.load("/fixture", "text", model_type="safetensors")
        with self.assertRaisesRegex(ValueError, "does not support"):
            async with manager.image_lease("text"):
                self.fail("Text model entered the image adapter")

    async def test_repeated_load_cancellation_retains_device_until_worker_returns(self):
        started, release = threading.Event(), threading.Event()
        manager = _TestModelManager(_FakeDeviceManager())

        def load(*_args):
            from model_manager import LoadedModel

            started.set()
            release.wait(5)
            return LoadedModel(object(), None, None, NUNCHAKU_Z_IMAGE)

        manager._load_diffusion = load
        task = asyncio.create_task(
            manager.load(
                "/fixture", "image", model_type=NUNCHAKU_Z_IMAGE, pipeline_path="/components"
            )
        )
        try:
            self.assertTrue(await asyncio.to_thread(started.wait, 3))
            task.cancel()
            await asyncio.sleep(0.02)
            task.cancel()
            await asyncio.sleep(0.02)
            self.assertFalse(task.done())
            self.assertTrue(any(lock.locked() for lock in manager._device_locks.values()))
        finally:
            release.set()
            with self.assertRaises(asyncio.CancelledError):
                await task
        self.assertFalse(any(lock.locked() for lock in manager._device_locks.values()))

    async def test_cancelled_image_task_keeps_lease_until_worker_stops(self):
        from image_api import ImageRequest, owned_generation

        started, release, cancelled = threading.Event(), threading.Event(), threading.Event()

        class Adapter:
            def generate(self, _prompt, _width, _height, _seed, cancel):
                started.set()
                release.wait(5)
                if cancel.is_set():
                    cancelled.set()
                    raise GenerationCancelled()
                raise AssertionError("Expected cancellation")

        class Request:
            async def is_disconnected(self):
                return False

        manager = _TestModelManager(_FakeDeviceManager())
        slot = await manager.load("/fixture", "image", model_type=NUNCHAKU_Z_IMAGE)

        async def run():
            async with manager.image_lease("image"):
                return await owned_generation(
                    Adapter(),
                    ImageRequest(model="image", prompt="x", width=512, height=512),
                    Request(),
                )

        task = asyncio.create_task(run())
        try:
            self.assertTrue(await asyncio.to_thread(started.wait, 3))
            task.cancel()
            await asyncio.sleep(0.05)
            self.assertFalse(task.done())
            with self.assertRaisesRegex(RuntimeError, "busy"):
                await manager.unload(slot.slot_id)
        finally:
            release.set()
            with self.assertRaises(asyncio.CancelledError):
                await task
        self.assertTrue(cancelled.is_set())
        await manager.unload(slot.slot_id)
