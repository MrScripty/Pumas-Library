"""Model Manager - Slot-based model loading and unloading."""

import asyncio
import logging
import uuid
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional

import torch

from device_manager import DeviceManager

logger = logging.getLogger(__name__)

EXPECTED_LOAD_ERRORS = (OSError, RuntimeError, ValueError, KeyError)
EXPECTED_UNLOAD_ERRORS = (OSError, RuntimeError, AttributeError)


class SlotState(str, Enum):
    UNLOADED = "unloaded"
    LOADING = "loading"
    READY = "ready"
    UNLOADING = "unloading"
    ERROR = "error"


@dataclass
class LoadedModel:
    model: Any
    tokenizer: Any
    device: torch.device
    model_type: Optional[str] = None


@dataclass
class ModelSlot:
    slot_id: str
    model_name: str
    model_path: str
    device: str
    state: SlotState = SlotState.UNLOADED
    gpu_memory_bytes: Optional[int] = None
    ram_memory_bytes: Optional[int] = None
    model_type: Optional[str] = None
    _loaded: Optional[LoadedModel] = field(default=None, repr=False)

    def to_dict(self) -> dict:
        return {
            "slot_id": self.slot_id,
            "model_name": self.model_name,
            "model_path": self.model_path,
            "device": self.device,
            "state": self.state.value,
            "gpu_memory_bytes": self.gpu_memory_bytes,
            "ram_memory_bytes": self.ram_memory_bytes,
            "model_type": self.model_type,
        }


class ModelManager:
    """Manages model loading/unloading with slot-based multi-model support."""

    def __init__(self, device_manager: DeviceManager, max_loaded_models: int = 4):
        self.device_manager = device_manager
        self.max_loaded_models = max_loaded_models
        self.slots: dict[str, ModelSlot] = {}
        self._device_locks: dict[str, asyncio.Lock] = {}
        self._registry_lock = asyncio.Lock()

    def _get_device_lock(self, device_str: str) -> asyncio.Lock:
        if device_str not in self._device_locks:
            self._device_locks[device_str] = asyncio.Lock()
        return self._device_locks[device_str]

    def list_slots(self) -> list[dict]:
        return [slot.to_dict() for slot in self.slots.values()]

    def get_slot(self, slot_id: str) -> Optional[ModelSlot]:
        return self.slots.get(slot_id)

    async def set_max_loaded_models(self, max_loaded_models: int) -> None:
        """Update slot limit without invalidating active or loading slots."""
        async with self._registry_lock:
            active_count = self._active_slot_count()
            if max_loaded_models < active_count:
                raise RuntimeError(
                    f"Cannot reduce max_loaded_models to {max_loaded_models}; "
                    f"{active_count} models are active or loading."
                )
            self.max_loaded_models = max_loaded_models

    async def load(
        self,
        model_path: str,
        model_name: str,
        device_str: str = "auto",
        model_type: Optional[str] = None,
        pipeline_path: Optional[str] = None,
        vae_path: Optional[str] = None,
    ) -> ModelSlot:
        """Load a model into a new slot."""
        resolved_device = self.device_manager.resolve_device(device_str)
        device_label = str(resolved_device)

        async with self._registry_lock:
            active_count = self._active_slot_count()
            if active_count >= self.max_loaded_models:
                raise RuntimeError(
                    f"Maximum loaded models ({self.max_loaded_models}) reached. "
                    "Unload a model first."
                )

            slot_id = str(uuid.uuid4())[:8]
            slot = ModelSlot(
                slot_id=slot_id,
                model_name=model_name,
                model_path=model_path,
                device=device_label,
                state=SlotState.LOADING,
                model_type=model_type,
            )
            self.slots[slot_id] = slot
            lock = self._get_device_lock(device_label)

        try:
            async with lock:
                if pipeline_path is not None:
                    task = asyncio.create_task(
                        asyncio.to_thread(
                            self._load_diffusion,
                            model_path,
                            pipeline_path,
                            resolved_device,
                            model_type,
                            vae_path,
                        )
                    )
                    try:
                        loaded = await asyncio.shield(task)
                    except asyncio.CancelledError:
                        # A disconnected load cannot release the GPU while its
                        # executor thread still allocates tensors.
                        try:
                            while not task.done():
                                try:
                                    await asyncio.shield(task)
                                except asyncio.CancelledError:
                                    continue
                                except Exception:
                                    break
                            if not task.cancelled() and task.exception() is None:
                                abandoned = task.result()
                                abandoned.model = None
                                abandoned.tokenizer = None
                        finally:
                            await self._mark_slot_error(slot_id)
                        raise
                else:
                    loaded = await self._load_model(model_path, resolved_device, model_type)

            async with self._registry_lock:
                slot._loaded = loaded
                slot.model_type = loaded.model_type or model_type
                slot.state = SlotState.READY

                # Update memory usage
                if resolved_device.type == "cuda":
                    slot.gpu_memory_bytes = self.device_manager.get_device_memory_used(
                        resolved_device
                    )
                else:
                    slot.ram_memory_bytes = _estimate_model_ram(loaded.model)

            logger.info("Model loaded: %s on %s (slot %s)", model_name, device_label, slot_id)
        except EXPECTED_LOAD_ERRORS:
            await self._mark_slot_error(slot_id)
            logger.exception("Failed to load model %s", model_name)
            raise
        except Exception:
            await self._mark_slot_error(slot_id)
            logger.exception("Unexpected failure while loading model %s", model_name)
            raise

        return slot

    @staticmethod
    def _load_diffusion(
        model_path, pipeline_path, device, model_type, vae_path=None
    ) -> LoadedModel:
        from diffusion import FLUX2_KLEIN, NUNCHAKU_Z_IMAGE, NunchakuZImage

        try:
            if model_type == NUNCHAKU_Z_IMAGE:
                adapter = NunchakuZImage(model_path, pipeline_path, device)
            elif model_type == FLUX2_KLEIN:
                from flux2 import Flux2Klein

                adapter = Flux2Klein(model_path, pipeline_path, vae_path, device)
            else:
                raise ValueError("Unsupported diffusion adapter")
            return LoadedModel(adapter, None, device, model_type)
        finally:
            torch.cuda.synchronize(device)

    @asynccontextmanager
    async def image_lease(self, model_name: str):
        """Reject queued work and retain the same device lease used by loading."""
        from diffusion import IMAGE_ADAPTERS

        slot = next((s for s in self.slots.values() if s.model_name == model_name), None)
        if slot is None or slot.state != SlotState.READY or slot._loaded is None:
            raise KeyError("Image model is unavailable")
        if slot.model_type not in IMAGE_ADAPTERS:
            raise ValueError("Selected model does not support image generation")
        lock = self._get_device_lock(slot.device)
        if lock.locked():
            raise RuntimeError("Image runtime is busy")
        async with lock:
            yield slot._loaded.model

    def _load_sync(
        self, model_path: str, device: torch.device, model_type: Optional[str]
    ) -> LoadedModel:
        """Synchronous model loading (runs in executor)."""
        from loaders import load_model

        model, tokenizer, detected_type = load_model(model_path, device, model_type)
        return LoadedModel(
            model=model,
            tokenizer=tokenizer,
            device=device,
            model_type=detected_type,
        )

    async def _load_model(
        self, model_path: str, device: torch.device, model_type: Optional[str]
    ) -> LoadedModel:
        """Load a model without blocking the event loop."""
        return await asyncio.get_event_loop().run_in_executor(
            None, self._load_sync, model_path, device, model_type
        )

    async def unload(self, slot_id: str) -> None:
        """Unload a model from a slot."""
        async with self._registry_lock:
            slot = self.slots.get(slot_id)
            if slot is None:
                raise KeyError(f"Slot not found: {slot_id}")
            if slot.state == SlotState.LOADING:
                raise RuntimeError(f"Cannot unload loading slot: {slot_id}")
            if self._get_device_lock(slot.device).locked():
                raise RuntimeError("Cannot unload a model while its device is busy")
            slot.state = SlotState.UNLOADING

        try:
            if slot._loaded is not None:
                device = slot._loaded.device
                # Free model and tokenizer
                del slot._loaded.model
                del slot._loaded.tokenizer
                slot._loaded = None

                # Clear CUDA cache if applicable
                if device.type == "cuda":
                    torch.cuda.empty_cache()

            async with self._registry_lock:
                if self.slots.get(slot_id) is slot:
                    del self.slots[slot_id]
            logger.info("Model unloaded: %s (slot %s)", slot.model_name, slot_id)
        except EXPECTED_UNLOAD_ERRORS:
            await self._mark_slot_error(slot_id)
            logger.exception("Failed to unload slot %s", slot_id)
            raise
        except Exception:
            await self._mark_slot_error(slot_id)
            logger.exception("Unexpected failure while unloading slot %s", slot_id)
            raise

    def get_model_for_inference(self, model_name: str) -> Optional[LoadedModel]:
        """Get a loaded model by name for inference."""
        for slot in self.slots.values():
            if slot.model_name == model_name and slot.state == SlotState.READY and slot._loaded:
                return slot._loaded
        return None

    def list_model_names(self) -> list[str]:
        """List names of all ready models."""
        return [slot.model_name for slot in self.slots.values() if slot.state == SlotState.READY]

    def _active_slot_count(self) -> int:
        return sum(
            1 for slot in self.slots.values() if slot.state in (SlotState.READY, SlotState.LOADING)
        )

    async def _mark_slot_error(self, slot_id: str) -> None:
        async with self._registry_lock:
            slot = self.slots.get(slot_id)
            if slot is not None:
                slot.state = SlotState.ERROR


def _estimate_model_ram(model: Any) -> Optional[int]:
    """Estimate RAM usage of a model in bytes."""
    try:
        total = sum(p.nelement() * p.element_size() for p in model.parameters())
        return total
    except (AttributeError, TypeError, RuntimeError):
        logger.debug("Failed to estimate model RAM usage", exc_info=True)
        return None
    except Exception:
        logger.exception("Unexpected error while estimating model RAM usage")
        return None
