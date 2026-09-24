"""Ordinary-load custody evidence with real executor threads and controlled models."""

import asyncio
from concurrent.futures import ThreadPoolExecutor
import threading
import unittest

from test_model_manager import _FakeDeviceManager, _FakeModel
from model_manager import LoadedModel, ModelManager, SlotState


class _ControlledLoadManager(ModelManager):
    """Expose lifecycle barriers while retaining the production executor and locks."""

    def __init__(self):
        super().__init__(_FakeDeviceManager(), max_loaded_models=1)
        self.loop = asyncio.get_running_loop()
        self.worker_started = asyncio.Event()
        self.publication_started = asyncio.Event()
        self.cleanup_started = asyncio.Event()
        self.finalization_started = asyncio.Event()
        self.release_worker = threading.Event()
        self.worker_error = None
        self.publication_error = None
        self.loaded_result = None

    def _load_sync(self, model_path, device, model_type):
        """Block only the executor thread until the test releases its work."""
        self.loop.call_soon_threadsafe(self.worker_started.set)
        self.release_worker.wait()
        if self.worker_error is not None:
            raise self.worker_error
        self.loaded_result = LoadedModel(_FakeModel(), object(), device, model_type)
        return self.loaded_result

    async def _publish_loaded(self, slot, loaded, model_type, resolved_device):
        """Signal entry before acquiring the real publication lock."""
        self.publication_started.set()
        if self.publication_error is not None:
            raise self.publication_error
        await super()._publish_loaded(slot, loaded, model_type, resolved_device)

    async def _finish_abandoned_load(self, slot_id, worker, loaded):
        """Signal cancellation handling before running production cleanup."""
        self.cleanup_started.set()
        return await super()._finish_abandoned_load(slot_id, worker, loaded)

    async def _mark_slot_error(self, slot_id):
        """Signal entry before acquiring the real terminalization lock."""
        self.finalization_started.set()
        await super()._mark_slot_error(slot_id)


class ModelLoadLifecycleTests(unittest.IsolatedAsyncioTestCase):
    """Prove slot recovery and device custody at each ordinary-load suspension."""

    async def asyncSetUp(self):
        """Give each test its own worker, barriers, registry, and device lock."""
        loop = asyncio.get_running_loop()
        self.worker_executor = ThreadPoolExecutor(max_workers=2)
        self.addCleanup(self.worker_executor.shutdown, wait=True)
        self.original_run_in_executor = loop.run_in_executor
        self.addCleanup(setattr, loop, "run_in_executor", self.original_run_in_executor)

        def run_in_owned_executor(executor, func, *args):
            if executor is not None:
                return self.original_run_in_executor(executor, func, *args)
            worker = self.worker_executor.submit(func, *args)

            async def observe_worker():
                # Poll the real worker from the event loop; this fixture cannot
                # depend on the default executor's cross-thread wakeup at teardown.
                while not worker.done():
                    await asyncio.sleep(0.01)
                return worker.result()

            return loop.create_task(observe_worker())

        loop.run_in_executor = run_in_owned_executor
        self.manager = _ControlledLoadManager()
        device = self.manager.device_manager.resolve_device("auto")
        self.device_lock = self.manager._get_device_lock(str(device))
        self.task = None
        self.registry_held = False
        self.device_held = False

    async def asyncTearDown(self):
        """Release test-owned barriers and observe the load even after an assertion fails."""
        if self.registry_held:
            self.manager._registry_lock.release()
        if self.device_held:
            self.device_lock.release()
        self.manager.release_worker.set()
        if self.task is not None:
            if not self.task.done():
                self.task.cancel()
            await asyncio.gather(self.task, return_exceptions=True)

    async def _start_load(self):
        """Wait for an actual executor worker, rather than elapsed time, to start."""
        self.task = asyncio.create_task(self.manager.load("/fixture", "text"))
        await asyncio.wait_for(self.manager.worker_started.wait(), timeout=3)
        return next(iter(self.manager.slots.values()))

    async def _hold_registry(self):
        """Keep publication or terminalization pending at its real lock boundary."""
        await self.manager._registry_lock.acquire()
        self.registry_held = True

    def _release_registry(self):
        """Release only the registry acquisition owned by this test."""
        self.manager._registry_lock.release()
        self.registry_held = False

    async def _assert_capacity_reusable(self, slot):
        """Prove ERROR is observable, unloadable, and no longer consumes capacity."""
        self.assertEqual(slot.state, SlotState.ERROR)
        self.assertIsNone(slot._loaded)
        self.assertIsNone(self.manager.get_model_for_inference("text"))
        self.assertFalse(self.device_lock.locked())
        self.manager.worker_error = None
        self.manager.publication_error = None
        self.manager.release_worker.set()
        replacement = await self.manager.load("/fixture", "replacement")
        self.assertEqual(replacement.state, SlotState.READY)
        await self.manager.unload(slot.slot_id)
        await self.manager.unload(replacement.slot_id)
        self.assertEqual(self.manager.list_slots(), [])

    async def test_cancel_before_device_acquisition_terminalizes_under_repeated_cancel(self):
        """A reserved slot recovers without starting work or releasing another owner's lock."""
        await self.device_lock.acquire()
        self.device_held = True
        self.task = asyncio.create_task(self.manager.load("/fixture", "text"))
        await asyncio.sleep(0)
        slot = next(iter(self.manager.slots.values()))
        await self._hold_registry()

        self.task.cancel("original cancellation")
        await asyncio.wait_for(self.manager.finalization_started.wait(), timeout=3)
        self.task.cancel("repeated cancellation")
        await asyncio.sleep(0)
        self.assertFalse(self.task.done())
        self.assertFalse(self.manager.worker_started.is_set())
        self.assertEqual(slot.state, SlotState.LOADING)
        self._release_registry()
        with self.assertRaisesRegex(asyncio.CancelledError, "original cancellation"):
            await self.task
        self.assertTrue(self.device_lock.locked())
        self.device_lock.release()
        self.device_held = False
        await self._assert_capacity_reusable(slot)

    async def test_cancel_during_executor_load_retains_device_until_worker_returns(self):
        """Repeated cancellation preserves custody, disposes the result, and frees capacity."""
        slot = await self._start_load()
        self.task.cancel("original cancellation")
        await asyncio.wait_for(self.manager.cleanup_started.wait(), timeout=3)
        self.task.cancel("repeated cancellation")
        await asyncio.sleep(0)
        self.assertFalse(self.task.done())
        self.assertTrue(self.device_lock.locked())
        self.assertEqual(slot.state, SlotState.LOADING)
        with self.assertRaisesRegex(RuntimeError, "Maximum loaded models"):
            await self.manager.load("/fixture", "too-early")

        self.manager.release_worker.set()
        with self.assertRaisesRegex(asyncio.CancelledError, "original cancellation"):
            await self.task
        self.assertIsNone(self.manager.loaded_result.model)
        self.assertIsNone(self.manager.loaded_result.tokenizer)
        await self._assert_capacity_reusable(slot)

    async def test_cancel_during_publication_retains_device_through_terminalization(self):
        """A completed model is disposed before ERROR, despite cancellation at the registry."""
        slot = await self._start_load()
        await self._hold_registry()
        self.manager.release_worker.set()
        await asyncio.wait_for(self.manager.publication_started.wait(), timeout=3)
        self.assertTrue(self.device_lock.locked())
        self.task.cancel("original cancellation")
        await asyncio.wait_for(self.manager.finalization_started.wait(), timeout=3)
        self.assertIsNone(self.manager.loaded_result.model)
        self.assertIsNone(self.manager.loaded_result.tokenizer)
        self.task.cancel("repeated cancellation")
        await asyncio.sleep(0)
        self.assertFalse(self.task.done())
        self.assertTrue(self.device_lock.locked())
        self.assertEqual(slot.state, SlotState.LOADING)

        self._release_registry()
        with self.assertRaisesRegex(asyncio.CancelledError, "original cancellation"):
            await self.task
        await self._assert_capacity_reusable(slot)

    async def test_success_retains_device_until_ready_is_published(self):
        """A successful load keeps its model and releases custody only after READY."""
        slot = await self._start_load()
        await self._hold_registry()
        self.manager.release_worker.set()
        await asyncio.wait_for(self.manager.publication_started.wait(), timeout=3)
        self.assertTrue(self.device_lock.locked())
        self.assertEqual(slot.state, SlotState.LOADING)
        self.assertIsNone(slot._loaded)

        self._release_registry()
        self.assertIs(await self.task, slot)
        self.assertEqual(slot.state, SlotState.READY)
        self.assertIs(slot._loaded, self.manager.loaded_result)
        self.assertIsNotNone(slot._loaded.model)
        self.assertFalse(self.device_lock.locked())
        await self.manager.unload(slot.slot_id)

    async def test_worker_failure_preserves_exception_and_recovers_capacity(self):
        """An executor failure is observed and the failed slot remains unloadable."""
        error = RuntimeError("controlled worker failure")
        self.manager.worker_error = error
        slot = await self._start_load()
        with self.assertLogs("model_manager", level="ERROR"):
            self.manager.release_worker.set()
            with self.assertRaises(RuntimeError) as result:
                await self.task
        self.assertIs(result.exception, error)
        await self._assert_capacity_reusable(slot)

    async def test_publication_failure_disposes_loaded_result(self):
        """Unexpected publication failure retains its exception while disposing the model."""
        error = TypeError("controlled publication failure")
        self.manager.publication_error = error
        slot = await self._start_load()
        with self.assertLogs("model_manager", level="ERROR"):
            self.manager.release_worker.set()
            with self.assertRaises(TypeError) as result:
                await self.task
        self.assertIs(result.exception, error)
        self.assertIsNone(self.manager.loaded_result.model)
        self.assertIsNone(self.manager.loaded_result.tokenizer)
        await self._assert_capacity_reusable(slot)

    async def test_cancel_during_failure_finalization_preserves_cancellation(self):
        """Cancellation supersedes a load failure only after terminalization finishes."""
        self.manager.worker_error = RuntimeError("controlled worker failure")
        slot = await self._start_load()
        await self._hold_registry()
        self.manager.release_worker.set()
        await asyncio.wait_for(self.manager.finalization_started.wait(), timeout=3)
        self.task.cancel("original cancellation")
        await asyncio.sleep(0)
        self.task.cancel("repeated cancellation")
        await asyncio.sleep(0)
        self.assertFalse(self.task.done())
        self.assertTrue(self.device_lock.locked())
        self.assertEqual(slot.state, SlotState.LOADING)

        with self.assertLogs("model_manager", level="ERROR"):
            self._release_registry()
            with self.assertRaisesRegex(asyncio.CancelledError, "original cancellation"):
                await self.task
        await self._assert_capacity_reusable(slot)
