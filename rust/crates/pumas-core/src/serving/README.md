# pumas-core serving

## Purpose
Own backend serving snapshots, request validation, non-critical error shaping, and in-memory status update feeds for user-directed model serving.

## Contents
| File | Description |
| ---- | ----------- |
| `mod.rs` | `ServingService`, serving request validation orchestration, snapshot mutation helpers, and update-feed publication. |
| `gateway_alias.rs` | Gateway model alias validation, effective alias derivation, and duplicate-alias checks. |
| `placement.rs` | Provider-specific placement validation for user-selected serving profiles and per-load placement fields. |

## Design Decisions
- Serving requests are user-directed. The service validates the selected model, runtime profile, provider, and placement instead of choosing another device or evicting models automatically.
- Served-model state is backend-owned. Frontend code may keep form drafts, but loaded/unloaded/error status comes from serving responses, snapshots, or update feeds.
- Update feeds are in-memory invalidation signals. Missed or stale cursors return `snapshot_required` so consumers refresh `get_serving_status` rather than replaying durable history.
- Provider-specific load/unload calls stay behind adapter boundaries. `pumas-core` owns validation and status state, while RPC/provider adapter code may perform operations that depend on crates outside `pumas-core`.
- Placement policy is isolated from validation orchestration so new runtime providers can add capability-specific placement rules without expanding the serving service entrypoint.
- Gateway alias policy is isolated from validation orchestration because aliases are public gateway-facing names and must remain consistent across providers.

## Invariants
- Renderer-supplied model paths are never accepted. Serving validation resolves executable artifacts through `ModelLibrary`.
- Invalid fit, unsupported placement, missing runtime, and provider load failures are non-critical domain errors when existing served state is preserved.
- Endpoint status must report the truth: `not_configured`, `provider_endpoint`, or `pumas_gateway`.
- A Pumas gateway is not implied by provider endpoint status.

## Revisit Triggers
- Serving state becomes durable across backend restarts.
- A Pumas gateway is implemented and serving snapshots need gateway endpoint state.
- Provider adapter inversion moves Ollama and llama.cpp orchestration fully behind core-owned traits.
