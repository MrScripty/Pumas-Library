"""Record installed Torch, sidecar, device, and selected adapter evidence."""

import asyncio
import hashlib
import importlib
import importlib.metadata
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ADAPTER_IMPORTS = {
    "flux2": (
        ("diffusers", "transformers"),
        (("diffusers", "Flux2KleinPipeline"), ("transformers", "Qwen3ForCausalLM")),
    ),
    "nunchaku": (
        ("diffusers", "nunchaku"),
        (("diffusers", "ZImagePipeline"), ("nunchaku", "NunchakuZImageTransformer2DModel")),
    ),
}


def runtime_file_hashes(root: Path) -> dict[str, str]:
    """Hash shipped sidecar sources at the runtime root and in loaders/."""
    hashes = {}
    paths = [*root.glob("*.py"), *(root / "loaders").glob("*.py")]
    for path in sorted(paths):
        relative = path.relative_to(root)
        if path.is_symlink() or not path.is_file():
            continue
        hashes[relative.as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return hashes


def driver_versions() -> list[str] | None:
    """Query driver metadata without allocating a CUDA tensor."""
    try:
        completed = subprocess.run(
            ["nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader"],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (FileNotFoundError, OSError, subprocess.TimeoutExpired):
        return None
    if completed.returncode:
        return None
    versions = sorted({line.strip() for line in completed.stdout.splitlines() if line.strip()})
    return versions or None


def all_distribution_versions() -> dict[str, str]:
    """Capture every installed distribution visible to this interpreter."""
    versions = {}
    for distribution in importlib.metadata.distributions():
        name = distribution.metadata.get("Name")
        if name:
            versions[name.lower().replace("_", "-")] = distribution.version
    return dict(sorted(versions.items()))


def adapter_import_probe(modules: tuple[str, ...], symbols: tuple[tuple[str, str], ...]) -> dict:
    """Check imports only; successful imports do not establish model execution."""
    versions = {}
    try:
        for module in modules:
            importlib.import_module(module)
            try:
                versions[module] = importlib.metadata.version(module)
            except importlib.metadata.PackageNotFoundError:
                versions[module] = "unknown"
        for module, symbol in symbols:
            if getattr(importlib.import_module(module), symbol, None) is None:
                raise ImportError(f"{module}.{symbol} is unavailable")
    except Exception as error:
        return {
            "status": "unavailable",
            "scope": "adapter imports",
            "error": str(error),
            "versions": versions,
        }
    return {
        "status": "inconclusive",
        "scope": "adapter imports passed; model not loaded",
        "versions": versions,
    }


def device_probe(torch) -> tuple[dict, dict]:
    hardware = {
        "cuda": getattr(torch.version, "cuda", None),
        "hip": getattr(torch.version, "hip", None),
        "devices": [],
    }
    capability = {
        "status": "unavailable",
        "scope": "CUDA tensor operation",
        "reason": "CUDA is not available",
    }
    try:
        if torch.cuda.is_available():
            for index in range(torch.cuda.device_count()):
                hardware["devices"].append(
                    {
                        "name": torch.cuda.get_device_name(index),
                        "capability": list(torch.cuda.get_device_capability(index)),
                    }
                )
            tensor = torch.ones((2, 2), device="cuda")
            if (tensor @ tensor).cpu().tolist() != [[2.0, 2.0], [2.0, 2.0]]:
                raise RuntimeError("Unexpected CUDA matrix product")
            capability = {"status": "passed", "scope": "CUDA tensor operation"}
    except Exception as error:
        hardware["error"] = str(error)
        capability = {"status": "failed", "scope": "CUDA tensor operation", "error": str(error)}
    return hardware, capability


def probe(root: Path) -> dict:
    resolution_bytes = (root / "resolution.json").read_bytes()
    resolution = json.loads(resolution_bytes)
    capabilities = {}
    installed = {}
    for artifact in resolution.get("artifacts", []):
        name = artifact["name"]
        try:
            installed[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            installed[name] = None
    hardware = {"cuda": None, "hip": None, "devices": []}
    try:
        torch = importlib.import_module("torch")
        if torch.__version__ != resolution["torch"]:
            raise RuntimeError(
                f"Installed Torch {torch.__version__} differs from resolved {resolution['torch']}"
            )
        capabilities["torch_import"] = {"status": "passed", "version": torch.__version__}
    except Exception as error:
        torch = None
        capabilities["torch_import"] = {"status": "failed", "error": str(error)}
    if torch is None:
        capabilities["cpu_tensor"] = {
            "status": "failed",
            "error": "Torch import/version check failed",
        }
        capabilities["cuda_tensor"] = {
            "status": "unavailable",
            "reason": "Torch import/version check failed",
        }
    else:
        try:
            tensor = torch.ones((2, 2))
            if (tensor @ tensor).tolist() != [[2.0, 2.0], [2.0, 2.0]]:
                raise RuntimeError("Unexpected CPU matrix product")
            capabilities["cpu_tensor"] = {"status": "passed"}
        except Exception as error:
            capabilities["cpu_tensor"] = {"status": "failed", "error": str(error)}
        hardware, capabilities["cuda_tensor"] = device_probe(torch)
    try:
        sys.path.insert(0, str(root))
        from serve import create_app

        app = create_app()
        endpoint = next(
            route.endpoint for route in app.routes if getattr(route, "path", None) == "/health"
        )
        health = asyncio.run(endpoint())
        if health.get("status") != "ok" or health.get("protocol") != 3:
            raise RuntimeError(f"Unexpected sidecar protocol: {health}")
        capabilities["sidecar_app"] = {
            "status": "passed",
            "health": health,
            "scope": "in-process app construction and health endpoint",
        }
    except Exception as error:
        capabilities["sidecar_app"] = {
            "status": "failed",
            "error": str(error),
            "scope": "in-process app construction and health endpoint",
        }
    selected = resolution.get("adapter", "none")
    for adapter, (modules, symbols) in ADAPTER_IMPORTS.items():
        key = "nunchaku_z_image" if adapter == "nunchaku" else "flux2_klein"
        capabilities[key] = (
            adapter_import_probe(modules, symbols)
            if selected in (adapter, "bundled")
            else {"status": "not selected", "scope": "optional adapter"}
        )
    image_device = any(device["capability"] == [12, 0] for device in hardware["devices"])
    capabilities["image_device"] = (
        {"status": "passed", "scope": "NVIDIA sm_120 device visible"}
        if image_device
        else {
            "status": "unavailable",
            "scope": "adapter device requirement",
            "reason": "No NVIDIA sm_120 CUDA device detected",
        }
    )
    capabilities["sidecar_startup"] = {
        "status": "not tested",
        "scope": "socket startup requires an explicit trial",
    }
    capabilities["image_generation"] = {
        "status": "not tested",
        "scope": "no model loaded or inference performed",
    }
    core_checks = ("torch_import", "cpu_tensor", "sidecar_app")
    core_status = (
        "passed"
        if all(capabilities[key]["status"] == "passed" for key in core_checks)
        else "failed"
    )
    if selected == "none":
        adapter_status = "not selected"
    else:
        adapter_keys = (
            ("nunchaku_z_image", "flux2_klein")
            if selected == "bundled"
            else ("nunchaku_z_image",)
            if selected == "nunchaku"
            else ("flux2_klein",)
        )
        adapter_status = (
            "unavailable"
            if any(capabilities[key]["status"] == "unavailable" for key in adapter_keys)
            or capabilities["image_device"]["status"] != "passed"
            else "inconclusive"
        )
    status = "failed" if core_status == "failed" else "partial" if selected != "none" else "passed"
    hardware_identity = {
        "cuda": hardware["cuda"],
        "hip": hardware["hip"],
        "device_count": len(hardware["devices"]),
        "devices": hardware["devices"],
    }
    hardware_fingerprint = hashlib.sha256(
        json.dumps(hardware_identity, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    all_installed = all_distribution_versions()
    distributions_sha256 = hashlib.sha256(
        json.dumps(all_installed, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    return {
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "status": status,
        "core_status": core_status,
        "adapter_status": adapter_status,
        "environment": resolution,
        "context": {
            "python": sys.version,
            "executable": sys.executable,
            "platform": platform.platform(),
            "hardware": hardware,
            "hardware_fingerprint": hardware_fingerprint,
            "driver_version": driver_versions(),
            "runtime_files_sha256": runtime_file_hashes(root),
            "interpreter_sha256": hashlib.sha256(
                Path(sys.executable).resolve().read_bytes()
            ).hexdigest(),
            "installed_distributions": installed,
            "all_installed_distributions": all_installed,
            "distributions_sha256": distributions_sha256,
            "resolution_sha256": hashlib.sha256(resolution_bytes).hexdigest(),
            "sidecar_sha256": hashlib.sha256((root / "serve.py").read_bytes()).hexdigest(),
        },
        "capabilities": capabilities,
    }


def main() -> None:
    root = Path(__file__).resolve().parent
    result = probe(root)
    (root / "probe-results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    if result["core_status"] != "passed":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
