"""Record concrete core operations without qualifying optional model adapters."""

import json
import asyncio
import hashlib
import platform
import sys
from pathlib import Path


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
    result = {
        "environment": resolution,
        "context": {
            "python": sys.version,
            "platform": platform.platform(),
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
