"""Qualify a staged runtime without acquiring or loading model assets."""

import json
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path


def validate_recipe_handshake(recipe: dict, health: dict) -> None:
    """Require the staged recipe and its bundled live sidecar to agree."""
    if recipe.get("protocol") != 3:
        raise RuntimeError("Runtime recipe does not declare Torch protocol 3")
    if recipe.get("capabilities") != ["image_generation"]:
        raise RuntimeError("Runtime recipe capabilities are not the qualified set")
    if health.get("status") != "ok":
        raise RuntimeError("Sidecar health status is not ready")
    if health.get("protocol") != recipe["protocol"]:
        raise RuntimeError("Sidecar protocol does not match runtime recipe")
    if health.get("capabilities") != recipe["capabilities"]:
        raise RuntimeError("Sidecar capabilities do not match runtime recipe")


def validate() -> None:
    if sys.version_info[:2] != (3, 12) or sys.platform != "linux":
        raise RuntimeError("This runtime requires CPython 3.12 on Linux")

    import torch
    from diffusers import Flux2KleinPipeline, ZImagePipeline
    from nunchaku import NunchakuZImageTransformer2DModel

    # Imports must resolve the concrete adapters; importing only torch is not
    # evidence that its independently compiled Nunchaku extension is usable.
    if any(
        item is None
        for item in (Flux2KleinPipeline, ZImagePipeline, NunchakuZImageTransformer2DModel)
    ):
        raise RuntimeError("Required diffusion adapter is unavailable")
    if torch.__version__ != "2.9.1+cu130" or torch.version.cuda != "13.0":
        raise RuntimeError("Runtime Torch/CUDA version does not match the recipe")
    if not torch.cuda.is_available() or torch.cuda.get_device_capability() != (12, 0):
        raise RuntimeError("This recipe is qualified only for an sm_120 NVIDIA GPU")
    value = torch.ones((16, 16), device="cuda", dtype=torch.bfloat16)
    if float((value @ value).sum().item()) != 4096:
        raise RuntimeError("CUDA execution validation failed")
    torch.cuda.synchronize()
    del value
    torch.cuda.empty_cache()

    root = Path(__file__).resolve().parent
    # This is the validated bundle directory, explicitly admitted under -I.
    sys.path.insert(0, str(root))
    from nunchaku_compat import PumasNunchakuZImageTransformer

    if not issubclass(PumasNunchakuZImageTransformer, NunchakuZImageTransformer2DModel):
        raise RuntimeError("Nunchaku adapter compatibility boundary is unavailable")
    recipe = json.loads((root / "runtime.json").read_text())
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    process = subprocess.Popen(
        [sys.executable, str(root / "serve.py"), "--port", str(port)], cwd=root
    )
    try:
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            if process.poll() is not None:
                raise RuntimeError("Sidecar exited during runtime validation")
            try:
                with urllib.request.urlopen(
                    f"http://127.0.0.1:{port}/health", timeout=1
                ) as response:
                    health = json.load(response)
                validate_recipe_handshake(recipe, health)
                print(
                    json.dumps(
                        {
                            "recipe": recipe["recipe_id"],
                            "torch": torch.__version__,
                            "cuda": torch.version.cuda,
                            "gpu": torch.cuda.get_device_name(),
                            "health": health,
                        }
                    ),
                    flush=True,
                )
                return
            except (urllib.error.URLError, TimeoutError):
                time.sleep(0.1)
        raise RuntimeError("Sidecar health validation timed out")
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


if __name__ == "__main__":
    validate()
