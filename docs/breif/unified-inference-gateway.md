# Unified Pumas Inference Gateway

**Status:** Deferred until after the next release, per user decision on
2026-09-12. This brief records direction only; it does not authorize further
feature development or add a requirement for the next release. Revisit scope and
acceptance after that release.

## Intent

Give clients one Pumas API for inference across llama.cpp and future backends.
Pumas owns model discovery, backend selection and model-loading policy. Backend
servers become execution details behind provider adapters.

```text
Clients → Pumas API gateway → Provider adapters → Inference backends
                 ↓
         Library and serving manager
```

## Current Behavior

Pumas's `/v1/models` advertises currently loaded models with usable serving
observations. Requests for unloaded models are rejected; gateway requests do not
currently trigger loading. The llama.cpp router separately advertises the
Pumas-generated GGUF catalog and can load models requested directly by clients.
Pumas observes those external loads and unloads in the library UI.

## Proposed Direction

- Advertise configured, compatible library models that are eligible for serving,
  including unloaded models with a valid backend/profile route. Catalog presence
  must remain distinct from loaded state, resource availability and readiness.
- Route requests through the existing provider registry and serving manager.
  On-demand loading needs owned lifecycle tracking, duplicate-load admission,
  resource checks, explicit eviction policy and observable failures. Merely
  expanding `/v1/models` is insufficient.
- Keep llama.cpp's router behind Pumas initially. Dedicated mode may also be
  supported, but it still uses backend HTTP endpoints; disabling router mode
  does not enforce gateway-only access.
- Define backend access controls. Loopback binding blocks remote access but
  does not prevent other local applications from bypassing Pumas. Enforced
  gateway-only access needs backend authentication or process/network isolation.
  Gateway exposure and client authentication also need an explicit policy.
- Preserve provider capabilities and clear unsupported-operation errors rather
  than implying every engine supports the same inference operations.

## Constraints To Preserve

Backend/library use remains standalone; GUI and inference plugins remain
optional. Uncertain loading outcomes must not trigger automatic mutation retries.
New llama.cpp catalog entries may be added live, while removals and changed paths
remain pending until profile restart, as explicitly approved by the user.

After the next release, settle discovery eligibility, stable model identities,
backend routing, cold-start behavior, resource/eviction policy and the required
access-isolation boundary before implementation. No release version or delivery
date is committed by this brief.
