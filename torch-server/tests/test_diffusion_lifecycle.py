"""Lease evidence only; real GPU and browser acceptance is recorded separately."""

import asyncio
import contextvars
from concurrent.futures import ThreadPoolExecutor
from functools import partial
import threading
import unittest
from unittest.mock import patch

from test_model_manager import _FakeDeviceManager, _TestModelManager
from model_manager import SlotState
from diffusion import GenerationCancelled, NUNCHAKU_Z_IMAGE


class DiffusionLeaseTests(unittest.IsolatedAsyncioTestCase):
    def _use_owned_thread_executor(self, max_workers):
        executor = self.enterContext(ThreadPoolExecutor(max_workers=max_workers))

        async def owned_to_thread(func, *args, **kwargs):
            context = contextvars.copy_context()
            work = partial(context.run, partial(func, *args, **kwargs))
            return await asyncio.get_running_loop().run_in_executor(executor, work)

        self.enterContext(patch.object(asyncio, "to_thread", new=owned_to_thread))

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

    async def test_load_cancellation_before_device_lock_marks_error_and_reuses_capacity(self):
        manager = _TestModelManager(_FakeDeviceManager(), max_loaded_models=1)
        device_label = str(manager.device_manager.resolve_device("auto"))
        device_lock = manager._get_device_lock(device_label)
        await device_lock.acquire()
        worker_started = threading.Event()

        def load(*_args):
            from model_manager import LoadedModel

            worker_started.set()
            return LoadedModel(object(), object(), None, NUNCHAKU_Z_IMAGE)

        manager._load_diffusion = load
        task = asyncio.create_task(
            manager.load(
                "/fixture", "image", model_type=NUNCHAKU_Z_IMAGE, pipeline_path="/components"
            )
        )
        await asyncio.sleep(0)
        self.assertEqual(len(manager.slots), 1)
        slot = next(iter(manager.slots.values()))
        self.assertEqual(slot.state, SlotState.LOADING)

        task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await task
        self.assertFalse(worker_started.is_set())
        self.assertEqual(slot.state, SlotState.ERROR)
        self.assertTrue(device_lock.locked())

        device_lock.release()
        self.assertFalse(device_lock.locked())
        replacement = await manager.load("/fixture", "replacement")
        self.assertEqual(replacement.state, SlotState.READY)

    async def test_repeated_load_cancellation_retains_device_until_worker_returns(self):
        started, release = threading.Event(), threading.Event()
        manager = _TestModelManager(_FakeDeviceManager(), max_loaded_models=1)
        model_ref, tokenizer_ref = object(), object()
        worker_result = []
        self._use_owned_thread_executor(max_workers=1)

        def load(*_args):
            from model_manager import LoadedModel

            loaded = LoadedModel(model_ref, tokenizer_ref, None, NUNCHAKU_Z_IMAGE)
            worker_result.append(loaded)
            started.set()
            release.wait(5)
            return loaded

        manager._load_diffusion = load
        task = asyncio.create_task(
            manager.load(
                "/fixture", "image", model_type=NUNCHAKU_Z_IMAGE, pipeline_path="/components"
            )
        )
        try:
            for _ in range(300):
                if started.is_set():
                    break
                await asyncio.sleep(0.01)
            self.assertTrue(started.is_set())
            slot = next(iter(manager.slots.values()))
            device_lock = next(iter(manager._device_locks.values()))
            self.assertEqual(slot.state, SlotState.LOADING)
            self.assertTrue(device_lock.locked())

            task.cancel()
            await asyncio.sleep(0.02)
            self.assertFalse(task.done())

            task.cancel()
            await asyncio.sleep(0.02)
            self.assertFalse(task.done())
            self.assertTrue(device_lock.locked())

            release.set()
        finally:
            release.set()
            with self.assertRaises(asyncio.CancelledError):
                await task

        self.assertEqual(slot.state, SlotState.ERROR)
        self.assertIsNone(worker_result[0].model)
        self.assertIsNone(worker_result[0].tokenizer)
        self.assertFalse(device_lock.locked())
        replacement = await manager.load("/fixture", "replacement")
        self.assertEqual(replacement.state, SlotState.READY)

    async def test_cancel_after_worker_completion_waits_for_abandoned_finalization(self):
        started, release = asyncio.Event(), asyncio.Event()
        publication_started = asyncio.Event()
        manager = _TestModelManager(_FakeDeviceManager(), max_loaded_models=1)
        model_ref, tokenizer_ref = object(), object()
        worker_result = []

        async def controlled_to_thread(func, *args, **kwargs):
            loaded = func(*args, **kwargs)
            started.set()
            await release.wait()
            return loaded

        self.enterContext(patch.object(asyncio, "to_thread", new=controlled_to_thread))

        publish_loaded = manager._publish_loaded

        async def observe_publication(*args):
            publication_started.set()
            await publish_loaded(*args)

        manager._publish_loaded = observe_publication

        def load(*_args):
            from model_manager import LoadedModel

            loaded = LoadedModel(model_ref, tokenizer_ref, None, NUNCHAKU_Z_IMAGE)
            worker_result.append(loaded)
            return loaded

        manager._load_diffusion = load
        task = asyncio.create_task(
            manager.load(
                "/fixture", "image", model_type=NUNCHAKU_Z_IMAGE, pipeline_path="/components"
            )
        )
        registry_locked = False
        try:
            await asyncio.wait_for(started.wait(), timeout=3)
            self.assertTrue(started.is_set())
            slot = next(iter(manager.slots.values()))
            device_lock = next(iter(manager._device_locks.values()))
            self.assertEqual(slot.state, SlotState.LOADING)
            self.assertTrue(device_lock.locked())

            self.assertFalse(manager._registry_lock.locked())
            await asyncio.wait_for(manager._registry_lock.acquire(), timeout=3)
            registry_locked = True
            release.set()
            await asyncio.wait_for(publication_started.wait(), timeout=3)
            self.assertFalse(task.done())
            self.assertIsNotNone(worker_result[0].model)
            self.assertEqual(slot.state, SlotState.LOADING)
            self.assertTrue(device_lock.locked())

            task.cancel()
            for _ in range(300):
                if worker_result[0].model is None:
                    break
                await asyncio.sleep(0.01)
            self.assertIsNone(worker_result[0].model)
            self.assertIsNone(worker_result[0].tokenizer)
            self.assertFalse(task.done())
            self.assertEqual(slot.state, SlotState.LOADING)
            self.assertTrue(device_lock.locked())

            task.cancel()
            await asyncio.sleep(0.02)
            self.assertFalse(task.done())
            self.assertTrue(device_lock.locked())
        finally:
            release.set()
            if registry_locked:
                manager._registry_lock.release()
            if not task.done():
                task.cancel()
            with self.assertRaises(asyncio.CancelledError):
                await task

        self.assertEqual(slot.state, SlotState.ERROR)
        self.assertIsNone(worker_result[0].model)
        self.assertIsNone(worker_result[0].tokenizer)
        self.assertFalse(device_lock.locked())
        replacement = await manager.load("/fixture", "replacement")
        self.assertEqual(replacement.state, SlotState.READY)

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
        self._use_owned_thread_executor(max_workers=2)

        async def run():
            async with manager.image_lease("image"):
                return await owned_generation(
                    Adapter(),
                    ImageRequest(model_id="image", prompt="x", width=512, height=512),
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
