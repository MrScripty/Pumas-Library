"""Torch Inference Server - Entry Point.

Starts a FastAPI server that provides:
  - /v1/*   OpenAI-compatible API endpoints
  - /api/*  Pumas control endpoints (load/unload/status/devices)
  - /health Health check endpoint
"""

import argparse
import logging
import sys

import uvicorn
from fastapi import FastAPI

from control_api import router as control_router
from device_manager import DeviceManager
from model_manager import ModelManager
from openai_api import router as openai_router
from validation import (
    is_loopback_host,
    validate_api_port,
    validate_bind_host,
    validate_max_loaded_models,
)

logger = logging.getLogger(__name__)


def create_app(host: str = "127.0.0.1", port: int = 8400, max_models: int = 4) -> FastAPI:
    """Create and configure the FastAPI application."""
    lan_access = not is_loopback_host(host)
    host = validate_bind_host(host, lan_access=lan_access)
    port = validate_api_port(port)
    max_models = validate_max_loaded_models(max_models)

    device_manager = DeviceManager()
    model_manager = ModelManager(
        device_manager=device_manager,
        max_loaded_models=max_models,
    )
    app = FastAPI(title="Torch Inference Server", version="0.1.0")

    # Store managers in app state for access by route handlers
    app.state.model_manager = model_manager
    app.state.device_manager = device_manager
    app.state.config = {
        "host": host,
        "api_port": port,
        "max_loaded_models": max_models,
        "lan_access": lan_access,
    }

    if lan_access:
        logger.warning("Torch server LAN access enabled on %s:%s", host, port)

    app.include_router(openai_router, prefix="/v1")
    app.include_router(control_router, prefix="/api")

    @app.get("/health")
    async def health_check():
        return {"status": "ok"}

    return app


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Torch Inference Server")
    parser.add_argument("--host", default="127.0.0.1", help="Bind address")
    parser.add_argument("--port", type=int, default=8400, help="Listen port")
    parser.add_argument("--max-models", type=int, default=4, help="Max concurrent loaded models")
    return parser.parse_args()


if __name__ == "__main__":
    args = parse_args()
    try:
        application = create_app(host=args.host, port=args.port, max_models=args.max_models)
    except ValueError as exc:
        print(f"Invalid Torch server configuration: {exc}", file=sys.stderr)
        sys.exit(2)
    uvicorn.run(application, host=args.host, port=args.port, log_level="info")
