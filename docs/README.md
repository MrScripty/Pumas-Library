# Documentation

The documentation set is intentionally small. Current behavior belongs in the
nearest authoritative document; completed plans and obsolete audits remain
available through Git history rather than in the working tree.

## Current Guides

| Document | Purpose |
| --- | --- |
| [Project README](../README.md) | Product overview and quick start |
| [Architecture](ARCHITECTURE.md) | Process, state, storage, and contract ownership |
| [Development](DEVELOPMENT.md) | Setup, standards routing, and verification |
| [Contributing](../CONTRIBUTING.md) | Change, planning, documentation, and commit expectations |
| [Releasing](../RELEASING.md) | Current release flow, required evidence, and known gaps |
| [Security](SECURITY.md) | Reporting, trust boundaries, and accepted advisory exceptions |
| [Native bindings](native-bindings.md) | Binding status, generation, and packaging |
| [Opening an older library](library-upgrade.md) | The one-time library metadata upgrade |
| [Package-facts fingerprint](package-facts-fingerprint-canonicalization.md) | Canonical package-facts fingerprint |
| [Changelog](../CHANGELOG.md) and [0.8.0 release notes](release-notes-0.8.0.md) | What changed, per release |

## Contracts

Consumer-facing contracts:

- [Model operations v1](contracts/model-operations-v1.md) and [model lookup v1](contracts/model-lookup-v1.md)
- [Image-to-text](contracts/image-to-text-v0.8.md), [image generation](contracts/image-generation.md) and [generation lifetime](contracts/generation-lifetime.md)
- [Local HTTP consumer](contracts/local-http-consumer.md), [admission fence](contracts/local-http-admission-fence.md), [owner retention](contracts/local-owner-retention.md), [start authority](contracts/local-start-authority.md) and [inference-enabled bootstrap](contracts/inference-enabled-bootstrap.md)

Internal boundaries:

- [Artifact acquisition](contracts/artifact-acquisition.md), [Hugging Face model-detail cache](contracts/hf-model-detail-cache.md) and [retained download reconciliation](contracts/download-location-reconciliation.md)
- [Torch provider protocol](contracts/torch-provider-protocol.md)
- Audio: [custody foundation](contracts/audio-custody-foundation.md), [owned transport](contracts/owned-audio-transport.md), [owned lifecycle](contracts/owned-audio-lifecycle.md), [installed constructor](contracts/installed-audio-constructor.md), [installed session](contracts/installed-audio-session.md), [Cohere qualification](contracts/cohere-native-qualification.md) and [Cohere cohort](contracts/cohere-native-cohort.md)
- [Image-to-text native qualification](contracts/image-to-text-native-qualification.md)

Each native qualification contract links its machine-readable cohort or
acquisition plan (`contracts/*.json`).

## Durable Decisions

- [ADR 0001: ONNX Runtime provider model](adr/0001-onnx-runtime-provider-model.md)
- [ADR 0002: Torch image provider protocol](adr/0002-torch-image-provider-protocol.md)
- [ADR 0003: Explicit local acceptance of moved legacy download outputs](adr/0003-download-location-reconciliation.md)
- [ADR 0004: A model's category is metadata chosen by use](adr/0004-model-category-as-metadata.md)

## Runbooks

- [Recover a failed download whose files moved](runbooks/download-location-reconciliation.md)

## Point-in-Time Audits and Investigations

- [2026-09-03 current-standards audit](audits/current-standards-2026-09-03/README.md)
- [Download destination and model catalog investigation (2026-10-10)](investigations/download-path-discrepancy-2026-10-10.md)

Audits and investigations describe the named repository and standards commits.
They are evidence and remediation inputs, not current operating instructions.

## Release Evidence

- [0.8.0 attribution](release-attribution/0.8.0/README.md) and its
  [S3 profile](release-attribution/0.8.0-s3/README.md): the third-party notices
  and inventories that packages embed, checked by
  `scripts/release/check-attribution.cjs`
- [0.7.0 ONNX archive verification](release-evidence/0.7.0/onnx-archive-verification.json):
  point-in-time evidence for the 0.7.0 ONNX Runtime archive

Vulnerability results are not kept in the tree: each CI run produces the
`current-vulnerability-results` artifact (see [Security](SECURITY.md#dependency-scans)).

## Product Briefs

Direction, not commitments. Each brief states what of it exists today.

- [Hugging Face discovery cache](brief/hf-cache.md)
- [Generic model fetching (S3)](brief/s3-model-fetch.md)
- [Unified inference gateway](brief/unified-inference-gateway.md)
- [Intent, discovery and distributed model management](brief/intent-discovery-distribution.md)
- [Capability discovery and resolution fabric](brief/capability-discovery-fabric.md)
- [Agent interaction and MCP layer](brief/agent-interaction-mcp.md)

## Active and Planned Work

No plan is being executed right now. These hold open acceptance gates:

- [Artifact acquisition](plans/artifact-acquisition/plan.md) — Q1 (HTTP) and Q3 (S3) ship in 0.8.0; AQ-HTTP and AQ-S3 acceptance pending; Q2 not started
- [Runtime installations and model adapters](plans/runtime-installations-and-model-adapters/plan.md) — planned; R1 waits on AQ-HTTP acceptance
- [Download-location reconciliation](plans/download-location-reconciliation/plan.md) — implemented (ADR 0003); composed and artifact evidence pending
- [Torch cross-platform runtime management](plans/torch-cross-platform-runtime-management/plan.md) — packaged and Windows/macOS install gates open; the managed-Python license blocker is open
- [2026-09-03 current-standards remediation program](plans/current-standards-remediation-2026-09-03/plan.md) — open issues not yet re-verified against 0.8.0. Its focused plans: [Rust library and RPC](plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/plan.md), [frontend and UI](plans/current-standards-remediation-2026-09-03/frontend-and-ui/plan.md), [desktop, release, bindings and Torch](plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md) and [governance and verification](plans/current-standards-remediation-2026-09-03/governance-and-verification/plan.md) (accepted)
- [Hugging Face discovery cache readiness](plans/hf-discovery-cache/readiness.md) — deferred checklist for the shared cache
- Design records still linked as current: [operating owner custody](plans/operating-owner-custody-design.md) and [local discovery bootstrap](plans/local-discovery-bootstrap.md)

An implementation invocation must name the exact focused `plan.md` and an
explicit `start`, `continue`, or `verify` operation. Plans remain temporary
execution authority; durable accepted decisions move to the appropriate guide
or ADR when the program finishes.

## Subsystem Guides

- [Frontend](../frontend/README.md)
- [Electron desktop shell](../electron/README.md)
- [Rust workspace](../rust/README.md)
- [Core Rust crate](../rust/crates/pumas-core/README.md)
- [Torch sidecar](../torch-server/README.md)
- [Developer scripts](../scripts/README.md)
- [Generated bindings](../bindings/README.md)
- [Plugin manifests](../launcher-data/plugins/README.md)

When guidance conflicts with executable configuration or code, treat that as
documentation drift and fix the document and owning behavior together.
