# Torch Server

## Purpose
This directory contains the Python FastAPI sidecar that exposes Torch-backed model loading and OpenAI-compatible inference routes.

## Ownership
`serve.py` owns application construction and process entrypoints. `control_api.py` and `openai_api.py` own HTTP route registration. `model_manager.py` owns in-process model slot state. `device_manager.py` owns device discovery. `loaders/` owns format-specific model loading.

## Producer Contract
All request models that accept paths, network binding, model identifiers, or slot changes must validate those values at the API boundary before mutating manager state.

## Consumer Contract
The launcher and Rust app-manager clients should treat this service as a local sidecar with explicit bind-host and port policy. LAN exposure requires a documented auth or trusted-network policy.

## Boundary Validation
`validation.py` owns shared validation for model paths, model names, device selectors, listener host, API ports, and model-slot limits.

- `PUMAS_TORCH_MODEL_ROOTS` may contain an `os.pathsep`-separated list of approved model roots. When set, `/api/load` rejects model paths outside those roots.
- `PUMAS_TORCH_ALLOW_LAN=1` is required before the service accepts a non-loopback bind host or `lan_access=true`.
- API ports must be unprivileged ports in the range `1024..65535`.
- `max_loaded_models` is bounded to `1..16`.

## Testing Contract
Tests should create a fresh app instance per test and avoid sharing model manager state unless the test explicitly verifies concurrency behavior.

## Non-Goals
Frontend UI behavior is out of scope. Reason: this directory owns the Python service boundary only. Revisit trigger: add an end-to-end sidecar smoke harness.
