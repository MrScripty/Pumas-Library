"""Pumas Control API endpoints.

Provides /api/slots, /api/load, /api/unload, /api/status, /api/devices,
and /api/configure for the Pumas Library frontend to manage the server.
"""

import logging
from typing import Optional

from fastapi import APIRouter, HTTPException, Request
from pydantic import BaseModel, Field, field_validator, model_validator

from validation import (
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


def _log_and_raise_internal_error(action: str, error: Exception) -> None:
    logger.exception("Failed to %s", action)
    raise HTTPException(status_code=500, detail=str(error))


# --- Request Models ---

class LoadModelRequest(BaseModel):
    model_path: str
    model_name: str
    device: str = "auto"
    model_type: Optional[str] = None

    @field_validator("model_path")
    @classmethod
    def _validate_model_path(cls, value: str) -> str:
        return validate_model_path(value)

    @field_validator("model_name")
    @classmethod
    def _validate_model_name(cls, value: str) -> str:
        return validate_model_name(value)

    @field_validator("device")
    @classmethod
    def _validate_device(cls, value: str) -> str:
        return validate_device(value)


class UnloadModelRequest(BaseModel):
    slot_id: str = Field(min_length=1, max_length=64)


class ConfigureRequest(BaseModel):
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
        slot = await manager.load(
            model_path=req.model_path,
            model_name=req.model_name,
            device_str=req.device,
            model_type=req.model_type,
        )
        return {"success": True, "slot": slot.to_dict()}
    except RuntimeError as e:
        raise HTTPException(status_code=409, detail=str(e))
    except ValueError as e:
        raise HTTPException(status_code=400, detail=str(e))
    except Exception as e:
        _log_and_raise_internal_error("load model", e)


@router.post("/unload")
async def unload_model(req: UnloadModelRequest, request: Request):
    """Unload a model from a slot."""
    manager = request.app.state.model_manager

    try:
        await manager.unload(req.slot_id)
        return {"success": True}
    except KeyError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except RuntimeError as e:
        raise HTTPException(status_code=409, detail=str(e))
    except Exception as e:
        _log_and_raise_internal_error("unload model", e)


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
        except RuntimeError as e:
            raise HTTPException(status_code=409, detail=str(e))
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
