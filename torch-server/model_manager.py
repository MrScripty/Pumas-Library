"""Model Manager - Slot-based model loading and unloading."""

import asyncio
import logging
import uuid
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional

import torch

from device_manager import DeviceManager

logger = logging.getLogger(__name__)


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

    def _get_device_lock(self, device_str: str) -> asyncio.Lock:
        if device_str not in self._device_locks:
            self._device_locks[device_str] = asyncio.Lock()
        return self._device_locks[device_str]

    def list_slots(self) -> list[dict]:
        return [slot.to_dict() for slot in self.slots.values()]

    def get_slot(self, slot_id: str) -> Optional[ModelSlot]:
        return self.slots.get(slot_id)

    async def load(
        self,
        model_path: str,
        model_name: str,
        device_str: str = "auto",
        model_type: Optional[str] = None,
    ) -> ModelSlot:
        """Load a model into a new slot."""
        active_count = sum(
            1 for s in self.slots.values() if s.state in (SlotState.READY, SlotState.LOADING)
        )
        if active_count >= self.max_loaded_models:
            raise RuntimeError(
                f"Maximum loaded models ({self.max_loaded_models}) reached. "
                "Unload a model first."
            )

        slot_id = str(uuid.uuid4())[:8]
        resolved_device = self.device_manager.resolve_device(device_str)
        device_label = str(resolved_device)

        slot = ModelSlot(
            slot_id=slot_id,
            model_name=model_name,
            model_path=model_path,
            device=device_label,
            state=SlotState.LOADING,
            model_type=model_type,
        )
        self.slots[slot_id] = slot

        try:
            lock = self._get_device_lock(device_label)
            async with lock:
                loaded = await asyncio.get_event_loop().run_in_executor(
                    None, self._load_sync, model_path, resolved_device, model_type
                )
            slot._loaded = loaded
            slot.model_type = loaded.model_type or model_type
            slot.state = SlotState.READY

            # Update memory usage
            if resolved_device.type == "cuda":
                slot.gpu_memory_bytes = self.device_manager.get_device_memory_used(resolved_device)
            else:
                slot.ram_memory_bytes = _estimate_model_ram(loaded.model)

            logger.info("Model loaded: %s on %s (slot %s)", model_name, device_label, slot_id)
        except Exception as e:
            slot.state = SlotState.ERROR
            logger.error("Failed to load model %s: %s", model_name, e)
            raise

        return slot

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

    async def unload(self, slot_id: str) -> None:
        """Unload a model from a slot."""
        slot = self.slots.get(slot_id)
        if slot is None:
            raise KeyError(f"Slot not found: {slot_id}")

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

            del self.slots[slot_id]
            logger.info("Model unloaded: %s (slot %s)", slot.model_name, slot_id)
        except Exception as e:
            slot.state = SlotState.ERROR
            logger.error("Failed to unload slot %s: %s", slot_id, e)
            raise

    def get_model_for_inference(self, model_name: str) -> Optional[LoadedModel]:
        """Get a loaded model by name for inference."""
        for slot in self.slots.values():
            if slot.model_name == model_name and slot.state == SlotState.READY and slot._loaded:
                return slot._loaded
        return None

    def list_model_names(self) -> list[str]:
        """List names of all ready models."""
        return [
            slot.model_name
            for slot in self.slots.values()
            if slot.state == SlotState.READY
        ]


def _estimate_model_ram(model: Any) -> Optional[int]:
    """Estimate RAM usage of a model in bytes."""
    try:
        total = sum(p.nelement() * p.element_size() for p in model.parameters())
        return total
    except Exception:
        return None
