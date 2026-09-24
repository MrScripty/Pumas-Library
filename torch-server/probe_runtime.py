"""Record concrete core operations without qualifying optional model adapters."""

import json
import asyncio
import hashlib
import importlib
import platform
import sys
from pathlib import Path


def adapter_import_probe(modules: tuple[str, ...], symbols: tuple[tuple[str, str], ...]) -> dict:
    """Check importability only; model execution requires a separate trial."""
    try:
        for module in modules:
            importlib.import_module(module)
        for module, symbol in symbols:
            if getattr(importlib.import_module(module), symbol, None) is None:
                raise ImportError(f"{module}.{symbol} is unavailable")
    except Exception as error:
        return {"status": "unavailable", "scope": "adapter imports", "error": str(error)}
    return {"status": "inconclusive", "scope": "adapter imports passed; model not loaded"}


def main() -> None:
    root = Path(__file__).resolve().parent
    resolution = json.loads((root / "resolution.json").read_text())
    import torch

    if torch.__version__ != resolution["torch"]:
        raise RuntimeError(
            f"Installed Torch {torch.__version__} differs from resolved {resolution['torch']}"
        )
    capabilities = {"torch_import": {"status": "passed"}}
    try:
        tensor = torch.ones((2, 2))
        if (tensor @ tensor).tolist() != [[2.0, 2.0], [2.0, 2.0]]:
            raise RuntimeError("Unexpected matrix product")
        capabilities["cpu_tensor"] = {"status": "passed"}
    except Exception as error:
        capabilities["cpu_tensor"] = {"status": "failed", "error": str(error)}
    try:
        sys.path.insert(0, str(root))
        from serve import create_app

        app = create_app()
        endpoint = next(route.endpoint for route in app.routes if route.path == "/health")
        health = asyncio.run(endpoint())
        if health.get("status") != "ok" or health.get("protocol") != 3:
            raise RuntimeError(f"Unexpected sidecar protocol: {health}")
        capabilities["sidecar_app"] = {"status": "passed", "health": health}
    except Exception as error:
        capabilities["sidecar_app"] = {"status": "failed", "error": str(error)}
    hardware = {"cuda": torch.version.cuda, "devices": []}
    try:
        if torch.cuda.is_available():
            hardware["devices"] = [
                {
                    "name": torch.cuda.get_device_name(index),
                    "capability": list(torch.cuda.get_device_capability(index)),
                }
                for index in range(torch.cuda.device_count())
            ]
    except Exception as error:
        hardware["error"] = str(error)
    capabilities["nunchaku_z_image"] = adapter_import_probe(
        ("diffusers", "nunchaku"),
        (("diffusers", "ZImagePipeline"), ("nunchaku", "NunchakuZImageTransformer2DModel")),
    )
    capabilities["flux2_klein"] = adapter_import_probe(
        ("diffusers", "transformers"),
        (("diffusers", "Flux2KleinPipeline"), ("transformers", "Qwen3ForCausalLM")),
    )
    capabilities["image_device"] = (
        {"status": "passed", "scope": "NVIDIA sm_120 device visible"}
        if any(device["capability"] == [12, 0] for device in hardware["devices"])
        else {
            "status": "unavailable",
            "scope": "adapter device requirement",
            "error": "The current image adapters require an NVIDIA sm_120 CUDA device",
        }
    )
    result = {
        "environment": resolution,
        "context": {
            "python": sys.version,
            "platform": platform.platform(),
            "hardware": hardware,
            "resolution_sha256": hashlib.sha256(
                (root / "resolution.json").read_bytes()
            ).hexdigest(),
            "sidecar_sha256": hashlib.sha256((root / "serve.py").read_bytes()).hexdigest(),
        },
        "capabilities": capabilities
        | {
            "sidecar_startup": {"status": "not tested until explicit trial"},
            "image_generation": {"status": "not tested; model adapters are optional"},
        },
    }
    (root / "probe-results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
