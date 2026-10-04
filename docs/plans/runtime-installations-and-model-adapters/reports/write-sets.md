# Proposed write sets and integration boundaries

This is a source-grounded boundary inventory, not authorization to edit the user's repository. Exact new module names below are proposed. Before each slice, inspect current local/branch state, direct callers and generated outputs. A same-owner file discovered within the stated semantic boundary is a documented amendment; a new public/trust/ownership boundary triggers a decision review.

## Prerequisite boundary and shared-file custody

Acquisition's [write-set report](../../artifact-acquisition/reports/write-sets.md) owns Q1/Q2 HTTP/HF/native/package bridges and shared transfer/state changes. Runtime source integration starts only after the relevant [gate](../../artifact-acquisition/reports/dependency-gates.md) is ready. It does not introduce `acquisition/` source changes without an acquisition-owned contract amendment. The integrator serializes overlapping app-manager installer, Torch package integration, public contract/export and UI writes.

## R1 — installation identity and bound executable launch

Existing primary files/families:
- `rust/crates/pumas-app-manager/src/lib.rs`
- `rust/crates/pumas-app-manager/src/version_manager/{mod.rs,state.rs,installer.rs,launcher.rs}`
- `rust/crates/pumas-app-manager/src/version_manager/installer/torch.rs` and its currently linked publication/recovery tests
- `rust/crates/pumas-app-manager/src/process/{mod.rs,factory.rs,traits.rs}` — reconcile public legacy surfaces, not replace custody blindly
- `rust/crates/pumas-core/src/models/runtime_profile.rs`
- `rust/crates/pumas-core/src/runtime_profiles.rs` and `runtime_profiles/{route_config.rs,launch_specs.rs,launch_strategy.rs,process_owner.rs}`
- `rust/crates/pumas-core/src/api/{runtime_profiles.rs,state_runtime_profiles.rs}`
- Directly affected installed-version metadata/path helpers, located through their current callers before migration
- `rust/crates/pumas-rpc/src/handlers/runtime_profiles.rs`, affected `handlers/versions/` operations and launch/stop handlers

Proposed additions, only when their responsibility cannot be kept coherently in the existing owner: app-manager `runtime_installations/` for common identity/lifecycle facade and native/Python detail; a neutral core installed-runtime binding type where consumed by public/persisted profiles. No second mutable catalog and no new crate by default.

Integration-owned adjacent writes: RPC command/schema export, generated consumers, profile/install selection frontend, Electron preload/registry, focused fixtures and active-plan/ADR handoffs. The full output list must come from the exporter. Update actual controls in R1, not only the backend. Native launch prep moves/delegates with its core/embedding/local-IPC callers in the same accepted boundary.

R1 proves real Torch and llama.cpp lifecycle; AQ-HTTP supplies the acquisition boundary. Affected Ollama shared paths must either adopt the same neutral contract or have an explicit supported facade/cutover disposition; no broken third consumer. ONNX/external negative cases remain in the core profile tests.

## R2 — metadata catalog and independent adapter packages (requires AQ-PACKAGES)

Include `torch-server/resolve_runtime.py`, retained-resolution consumers in app-manager `version_manager/{torch_preview.rs,installer/torch.rs}`, local wheel staging/input projection, relevant package integrity tests and acquisition progress consumers. Q2 already provides the existing package-file handoff. R2 adds the independently registered adapter consumer rather than redesigning it. Package solving remains with tooling. Preserve original source provenance while making the installation leg consume exact verified local files. Audit managed-Python provider network/bootstrap hooks separately; do not replace unsupported private APIs or assert that all provider traffic is migrated.

Primary owners: optional app-manager catalog/registration/inspection service; Python adapter host discovery/API modules; installation integration for selected implementation distributions. Proposed `model_adapters/` modules are semantic ownership, not a required directory count.

Direct existing files to reconcile: `torch-server/{model_manager.py,probe_runtime.py,resolve_runtime.py,control_api.py,serve.py}`, existing runtime embedding/integrity inputs, `rust/crates/pumas-app-manager/src/torch_client.rs`, `core/plugins/{schema.rs,loader.rs}` only for genuinely shared/replaced authority, and affected RPC/GUI projection.

A separate adapter SDK/host contract can remain a small Python package/module; package its declaration/API independently only if the actual external package consumer requires that distribution. Establish name, API version, metadata/entry-point ownership and test boundary before publishing. Do not invent a native Rust dylib ABI, per-model provider variant or universal executable install language.

Add controlled adapter fixtures and a real independent package used outside Pumas build inputs. The historical stdlib probe is not production fixture/SDK source to copy wholesale.

## R3 — shared model admission and supported task behavior

Primary existing population:
- `rust/crates/pumas-core/src/model_library/{dependencies.rs,dependency_pins.rs}` and direct core API/DTO/projection consumers selected by the semantic change
- Existing `models/artifact_load_target.rs`, package-fact selection and approved load-target APIs (extend only when an actual missing contract is demonstrated)
- `rust/crates/pumas-core/src/providers/mod.rs` for host/task capability meaning, not model-ID enumeration
- `rust/crates/pumas-rpc/src/handlers/{serving.rs,serving_torch.rs,torch.rs,runtime_profiles.rs,openai_gateway.rs,openai_gateway_images.rs}` and affected native serving handlers discovered from provider dispatch
- Core serving publication/observation and profile ownership APIs needed to carry exact binding values
- `torch-server/{model_manager.py,control_api.py,openai_api.py,image_api.py,diffusion.py,flux2.py}` and `loaders/{__init__.py,safetensors_loader.py,dllm_loader.py,sherry_loader.py}`
- Actual `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx` and related active-slot/profile views

Proposed managed-execution module belongs in app-manager; transport consumes it. Move current adapter assumptions into the adapter host implementations and eliminate duplicated policy from RPC/gateway/probe/UI. Keep approved custom-code authorization on the reachable managed family. Ordinary registered adapters must not require new changes to those shared handlers after this slice.

## R4 — standard Z-Image extension

Primary writes are the separately packaged adapter definition/implementation, its declared dependencies, model/task fixtures and real acceptance procedure. The frozen Pumas build/host must already be sufficient. Any necessary shared-host change is evidence of a missing host contract and must be resolved in the owning earlier boundary, not hidden as an ordinary plugin addition.

Do not embed the external adapter's source into Rust to make the test pass. Tests may reference the independent package as an input; Pumas product source must not require it to build. Preserve the original qualified image implementations and exact output contracts.

## R5 — release/deployment and terminal source cleanup

Only affected public/binding consumers, generated output closure, packaging/embedding inventory, native QA workflows/procedures and current owner docs. Public factory/dead-path removal requires the actual supported consumer/version disposition. No release/tag publication, user-environment deletion, repository history rewrite or external repository write is authorized by this document.

## Shared artifacts and concurrent work

One integrator owns core types, wire/host API definitions, manifests, lockfiles, generated outputs, shared fixtures, active plan and migration decisions. Workers receive a fixed contract and non-overlapping primary writes. Environment/native internals, adapter-host internals and UI may be delegated only after their shared contracts are stable; list permitted adjacent changes and required evidence. Reviewers are read-only. Integrate serially and recheck the current candidate; do not let workers independently redesign a shared DTO or update generated files by hand.

## Before breaking or destructive changes

Read actual call sites of public app-manager/core launch APIs, current metadata helpers/exporters, local dirty changes, supported retained legacy records and independently deployed clients. Inspect exact native archive role/library layout. Missing consumer/state facts block their corresponding cutover, not unrelated reversible implementation. A global source search showing no local caller does not authorize breaking a public API by itself.
