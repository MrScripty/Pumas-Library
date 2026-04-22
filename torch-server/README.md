# Torch Server

## Purpose
This directory contains the Python FastAPI sidecar that exposes Torch-backed model loading and OpenAI-compatible inference routes.

## Ownership
`serve.py` owns application construction and process entrypoints. `control_api.py` and `openai_api.py` own HTTP route registration. `model_manager.py` owns in-process model slot state. `device_manager.py` owns device discovery. `loaders/` owns format-specific model loading.

## Producer Contract
All request models that accept paths, network binding, model identifiers, or slot changes must validate those values at the API boundary before mutating manager state.

## Consumer Contract
The launcher and Rust app-manager clients should treat this service as a local sidecar with explicit bind-host and port policy. LAN exposure requires a documented auth or trusted-network policy.

## Testing Contract
Tests should create a fresh app instance per test and avoid sharing model manager state unless the test explicitly verifies concurrency behavior.

## Non-Goals
Frontend UI behavior is out of scope. Reason: this directory owns the Python service boundary only. Revisit trigger: add an end-to-end sidecar smoke harness.
