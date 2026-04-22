import asyncio
import sys
import types
import unittest
from pathlib import Path


TORCH_SERVER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TORCH_SERVER_ROOT))


try:
    import torch  # noqa: F401
except ModuleNotFoundError:
    torch_module = types.ModuleType("torch")

    class _Device:
        def __init__(self, value):
            self.type = str(value).split(":", maxsplit=1)[0]
            self.value = value

        def __str__(self):
            return str(self.value)

    torch_module.device = _Device
    torch_module.cuda = types.SimpleNamespace(
        empty_cache=lambda: None,
        memory_allocated=lambda device: 0,
    )
    sys.modules["torch"] = torch_module


from model_manager import LoadedModel, ModelManager, SlotState  # noqa: E402


class _FakeModel:
    def parameters(self):
        return []


class _FakeDeviceManager:
    def resolve_device(self, device_str):
        return types.SimpleNamespace(type="cpu")

    def get_device_memory_used(self, device):
        return 0


class _TestModelManager(ModelManager):
    async def _load_model(self, model_path, device, model_type):
        return LoadedModel(
            model=_FakeModel(),
            tokenizer=object(),
            device=device,
            model_type=model_type,
        )


class ModelManagerConcurrencyTests(unittest.IsolatedAsyncioTestCase):
    async def test_concurrent_loads_reserve_slots_before_expensive_load(self):
        manager = _TestModelManager(device_manager=_FakeDeviceManager(), max_loaded_models=1)

        results = await asyncio.gather(
            manager.load("/tmp/model-a", "model-a"),
            manager.load("/tmp/model-b", "model-b"),
            return_exceptions=True,
        )

        loaded = [result for result in results if not isinstance(result, Exception)]
        rejected = [result for result in results if isinstance(result, RuntimeError)]

        self.assertEqual(len(loaded), 1)
        self.assertEqual(len(rejected), 1)
        self.assertEqual(len(manager.slots), 1)
        self.assertEqual(next(iter(manager.slots.values())).state, SlotState.READY)

    async def test_max_loaded_models_cannot_drop_below_active_slots(self):
        manager = ModelManager(device_manager=_FakeDeviceManager(), max_loaded_models=2)
        manager.slots["ready"] = types.SimpleNamespace(state=SlotState.READY)

        with self.assertRaises(RuntimeError):
            await manager.set_max_loaded_models(0)

        self.assertEqual(manager.max_loaded_models, 2)


if __name__ == "__main__":
    unittest.main()
