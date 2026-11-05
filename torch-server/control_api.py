"""Pumas Control API endpoints.

Provides /api/slots, /api/load, /api/unload, /api/status, /api/devices,
and /api/configure for the Pumas Library frontend to manage the server.
"""

import asyncio
import logging
from typing import Literal, Optional

import torch
from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator

from diffusion import FLUX2_KLEIN, IMAGE_ADAPTERS, NUNCHAKU_Z_IMAGE
from model_manager import SlotConflict
from validation import (
    ModelLoadRefusal,
    is_loopback_host,
    validate_api_port,
    validate_bind_host,
    validate_device,
    validate_max_loaded_models,
    validate_model_name,
    validate_model_path,
    require_lan_policy,
)

logger = logging.getLogger(__name__)

router = APIRouter()

# Model types the loaders and image adapters implement. Omitting model_type
# lets the loader detect a text type from config.json.
SupportedModelType = Literal[
    "text-generation",
    "dllm",
    "sherry",
    "cohere-asr",
    "nunchaku-z-image-turbo",
    "flux2-klein-9b-kv-fp8",
]


def failure(status: int, code: str, message: str) -> HTTPException:
    """A refusal with a stable code and a caller-safe reason."""
    return HTTPException(status_code=status, detail={"code": code, "message": message})


def _internal_error(action: str) -> HTTPException:
    # Raw exception text stays in the log; it can carry resolved paths.
    logger.exception("Failed to %s", action)
    return failure(500, "internal_error", f"Torch runtime failed to {action}; inspect its log")


def _is_cuda_oom(error: BaseException) -> bool:
    oom = getattr(getattr(torch, "cuda", None), "OutOfMemoryError", None)
    return isinstance(oom, type) and isinstance(error, oom)


# --- Request Models ---


class LoadModelRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    model_path: str
    model_name: str
    device: str = "auto"
    model_type: Optional[SupportedModelType] = None
    pipeline_path: Optional[str] = None
    vae_path: Optional[str] = None

    @field_validator("model_path")
    @classmethod
    def _validate_model_path(cls, value: str) -> str:
        return validate_model_path(value)

    @field_validator("pipeline_path", "vae_path")
    @classmethod
    def _validate_component_path(cls, value: Optional[str], info) -> Optional[str]:
        if value is None:
            return None
        return validate_model_path(value, field=info.field_name)

    @model_validator(mode="after")
    def _validate_adapter_components(self) -> "LoadModelRequest":
        image_adapter = self.model_type in IMAGE_ADAPTERS
        if image_adapter and self.pipeline_path is None:
            raise ValueError(f"model_type {self.model_type} requires pipeline_path")
        if self.pipeline_path is not None and not image_adapter:
            raise ValueError(
                f"pipeline_path requires model_type {NUNCHAKU_Z_IMAGE} or {FLUX2_KLEIN}"
            )
        if self.vae_path is not None and self.model_type != FLUX2_KLEIN:
            raise ValueError(f"vae_path requires model_type {FLUX2_KLEIN}")
        return self

    @field_validator("model_name")
    @classmethod
    def _validate_model_name(cls, value: str) -> str:
        return validate_model_name(value)

    @field_validator("device")
    @classmethod
    def _validate_device(cls, value: str) -> str:
        return validate_device(value)


class UnloadModelRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    slot_id: str = Field(min_length=1, max_length=64)


class ConfigureRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")
    host: Optional[str] = None
    api_port: Optional[int] = None
    max_loaded_models: Optional[int] = None
    lan_access: Optional[bool] = None

    @field_validator("api_port")
    @classmethod
    def _validate_api_port(cls, value: Optional[int]) -> Optional[int]:
        if value is None:
            return None
        return validate_api_port(value)

    @field_validator("max_loaded_models")
    @classmethod
    def _validate_max_loaded_models(cls, value: Optional[int]) -> Optional[int]:
        if value is None:
            return None
        return validate_max_loaded_models(value)

    @model_validator(mode="after")
    def _validate_listener_policy(self) -> "ConfigureRequest":
        lan_access = self.lan_access is True
        if self.host is not None:
            self.host = validate_bind_host(self.host, lan_access=lan_access)
        if lan_access:
            require_lan_policy()
        return self


# --- Endpoints ---


@router.get("/slots")
async def list_slots(request: Request):
    """List all model slots."""
    manager = request.app.state.model_manager
    return {"slots": manager.list_slots()}


@router.post("/load")
async def load_model(req: LoadModelRequest, request: Request):
    """Load a model into a new slot."""
    manager = request.app.state.model_manager

    try:
        options = {"pipeline_path": req.pipeline_path} if req.pipeline_path is not None else {}
        if req.vae_path is not None:
            options["vae_path"] = req.vae_path
        load = asyncio.create_task(
            manager.load(
                model_path=req.model_path,
                model_name=req.model_name,
                device_str=req.device,
                model_type=req.model_type,
                **options,
            )
        )
        try:
            while not load.done():
                if await request.is_disconnected():
                    load.cancel()
                    raise failure(499, "cancelled", "Model load cancelled")
                await asyncio.wait({load}, timeout=0.1)
            slot = load.result()
        finally:
            if not load.done():
                load.cancel()
                while not load.done():
                    try:
                        await asyncio.shield(load)
                    except asyncio.CancelledError:
                        continue
                    except Exception:
                        break
            if load.done() and not load.cancelled():
                load.exception()
        return {"success": True, "slot": slot.to_dict()}
    except HTTPException:
        raise
    except SlotConflict as error:
        raise failure(409, error.code, str(error)) from None
    except ModelLoadRefusal as error:
        raise failure(400, "model_rejected", str(error)) from None
    except Exception as error:
        if _is_cuda_oom(error):
            logger.warning("Model load ran out of device memory", exc_info=True)
            torch.cuda.empty_cache()
            raise failure(
                507, "out_of_memory", "Insufficient device memory to load this model"
            ) from None
        if isinstance(error, (ValueError, OSError)):
            # Library refusals (unrecognized config, missing weights) carry
            # resolved paths; the caller gets the stable reason only.
            logger.warning("Model load was refused by its loader", exc_info=True)
            raise failure(
                400,
                "model_unloadable",
                "The Torch runtime could not load these model files; inspect its log",
            ) from None
        raise _internal_error("load model") from None


@router.post("/unload")
async def unload_model(req: UnloadModelRequest, request: Request):
    """Unload a model from a slot."""
    manager = request.app.state.model_manager

    try:
        await manager.unload(req.slot_id)
        return {"success": True}
    except KeyError:
        raise failure(404, "slot_not_found", f"Slot not found: {req.slot_id}") from None
    except SlotConflict as error:
        raise failure(409, error.code, str(error)) from None
    except Exception:
        raise _internal_error("unload model") from None


@router.get("/status")
async def get_status(request: Request):
    """Get server status including slots and device usage."""
    manager = request.app.state.model_manager
    device_manager = request.app.state.device_manager
    config = request.app.state.config

    devices = [
        {
            "device_id": d.device_id,
            "name": d.name,
            "memory_total": d.memory_total,
            "memory_available": d.memory_available,
            "is_available": d.is_available,
        }
        for d in device_manager.list_devices()
    ]

    return {
        "running": True,
        "slots": manager.list_slots(),
        "devices": devices,
        "config": config,
        "api_url": f"http://{config['host']}:{config['api_port']}",
    }


@router.get("/devices")
async def list_devices(request: Request):
    """List available compute devices with memory info."""
    device_manager = request.app.state.device_manager
    devices = device_manager.list_devices()

    return {
        "devices": [
            {
                "device_id": d.device_id,
                "name": d.name,
                "memory_total": d.memory_total,
                "memory_available": d.memory_available,
                "is_available": d.is_available,
            }
            for d in devices
        ]
    }


@router.post("/configure")
async def configure(req: ConfigureRequest, request: Request):
    """Update server configuration (some changes require restart)."""
    config = request.app.state.config
    next_config = dict(config)
    manager = request.app.state.model_manager
    restart_required = False

    if req.host is not None and req.host != next_config["host"]:
        next_config["host"] = req.host
        next_config["lan_access"] = not is_loopback_host(req.host)
        restart_required = True

    if req.api_port is not None and req.api_port != next_config["api_port"]:
        next_config["api_port"] = req.api_port
        restart_required = True

    if req.max_loaded_models is not None:
        try:
            await manager.set_max_loaded_models(req.max_loaded_models)
        except SlotConflict as error:
            raise failure(409, error.code, str(error)) from None
        except Exception:
            raise _internal_error("configure the runtime") from None
        next_config["max_loaded_models"] = req.max_loaded_models

    if req.lan_access is not None:
        next_config["lan_access"] = req.lan_access
        if req.lan_access:
            next_config["host"] = "0.0.0.0"
        else:
            next_config["host"] = "127.0.0.1"
        restart_required = True

    config.update(next_config)

    return {
        "success": True,
        "config": config,
        "restart_required": restart_required,
    }
