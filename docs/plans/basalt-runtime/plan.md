# Managed Basalt runtime and inference defaults

- **Status** Accepted
- **Acceptance** Satisfied
- **Delivery** Pull request to `main`

## Objective

Install Basalt through Pumas's GitHub runtime installer, serve an Inkling 2 Mini pack through a managed runtime profile, and set compatible generation defaults through the Ctrl+click model dialog and the Pumas endpoint. Deliver the verified branch as a pull request to main.

## Decisions

- The existing acquisition and importer owners verify and publish pack bytes. The library identifies the Basalt v2 container from bounded content evidence. The upstream engine owns architecture and tensor compatibility.
- The shared GitHub installer owns release selection, trusted extraction, provenance and publication. Pumas owns the spawned runtime and its process lifecycle. Basalt declares its own build feature and starts only on a supported Linux x64 host with usable CUDA 13 and an NVIDIA GPU of compute capability 12.0.
- The runtime profile selects a dedicated managed Basalt process. Pumas publishes readiness only after the owned process reports that the selected model is loaded. A service's TCP address or model list alone is insufficient.
- The existing model metadata settings schema owns per-model values. The Ctrl+click editor and JSON-RPC reuse it. Basalt requests use explicit request options first, stored model overrides second, and the pack defaults for omitted values. Both the Pumas chat gateway and typed chat operation honor this ordering.
- The isolated `feat/basalt-runtime` worktree is based on current upstream main and owned by the Codex task through PR handoff.

## Composed design review

Applicable. Pack recognition has a filesystem and immutable-artifact lifecycle. Release acquisition has a vendor and version lifecycle. Launch and readiness are Pumas process-owner concerns. Sampling defaults are model metadata and request concerns. The shared identity is the selected artifact and managed profile; their release, launch, and request lifetimes remain independent. Callers supply a model/profile pair or request overrides and receive current availability or typed failures. They do not need the archive layout, GPU query details or provider JSON.

A new Basalt release changes installation qualification and its fixture; an engine readiness response change affects the Basalt readiness adapter; an inference option change reaches the model schema, provider conversion and generated UI contract only where the option is supported. Installed-version receipts, a selected file and bounded model options are the stable seams. Model metadata remains usable without the build or hardware capability. Removing the Basalt adapters drops that provider's install/launch/serve behavior without removing common acquisition, model editing, or other providers. Provider-specific launch and request mapping contain the required complexity. There is no separate downloader, process supervisor, settings database or settings dialog.

## Milestones and acceptance

| Slice | Lifecycle | Write set and evidence |
| --- | --- | --- |
| Pack admission | Accepted | Structurally valid and malformed container tests, local import, and the pinned Hugging Face pack verified against its publisher digest. |
| Runtime install and profile | Accepted | GitHub acquisition of v0.5.6, unsupported-device refusal, and managed load and unload on the RTX 5090 Laptop. |
| Inference defaults | Accepted | Electron and JSON-RPC persistence, explicit request precedence in both inference endpoints, typed vision and streaming, and a Workbench Pumas client response. |

| Claim | Kind | Environment | Mode | Status |
| --- | --- | --- | --- | --- |
| Pack acquired or imported and identified as a native Basalt artifact | Integration | Required real | Automated | Passed |
| Pumas installs Basalt, launches the selected profile, serves Inkling and releases the owned process on unload | System | Required real Blackwell | Automated | Passed |
| Ctrl+click editor persists supported values | User workflow | Electron and RPC | Automated | Passed |
| Real inference honors saved defaults and explicit overrides | System | Required real Blackwell | Automated | Passed |
| Feature/platform and unsupported-device outcomes agree with discovery and launch admission | Contract | Representative | Automated | Passed |

[Acceptance evidence](execution-ledger.md) · [Issues](issues.md)
