# pumas-app-manager

## Purpose
`pumas-app-manager` coordinates external application versions and service clients used by Pumas Library, including ComfyUI, Ollama, and Torch integration points.

## Ownership
This crate owns version installation workflows, external API client adapters, and app-specific lifecycle coordination. It does not own model-library catalog storage or RPC transport.

## Producer Contract
Operations that touch files, archives, or external service APIs must accept typed configuration from callers and report structured errors suitable for RPC projection.

## Consumer Contract
`pumas-rpc` may expose these operations over JSON-RPC after validating request payloads. Domain crates should not depend on this crate for model metadata or catalog behavior.

## Testing Contract
Tests should isolate network and filesystem effects behind temporary directories or test doubles. Long-running external service smoke tests require explicit opt-in.

## Non-Goals
None. Reason: application installation and service-client concerns are both active responsibilities. Revisit trigger: split app installers into per-app crates.
