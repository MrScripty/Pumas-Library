# Standards applicability, sources and verification limits

## Baselines

Pumas feature-branch head observed: `04e7f1568f00693c0ef26c77e0150e5e4dd112ea`. Coding-Standards head observed: `39d55dc330d44ecf940364ceada9d2527f7c7ea0`. Source reads used the connected GitHub tools. The delivered r3 plan was read through Files and its exact mounted ZIP was inspected/copied for this documentation revision. No source archive or production runtime was required for this task.

The standards comparison from `91ceb0fe` to `39d55dc330d44ecf940364ceada9d2527f7c7ea0` was inspected. The previously read normative Core, Router, Planning, Architecture, Code Design, Contracts/Evolution, Implementation, Verification, Proportionality, Dependencies and Security material remains the governing context. This round additionally read Persistence, Concurrency and Rust Async. These are manual applicability/source observations, not a claim that an executable standards-router or whole-repository compliance suite was run.

## Applicable obligations and their representation

| Actual task fact | Applicable authority | Where the plan addresses it |
| --- | --- | --- |
| Material sequencing and two dependent plans | Planning, Implementation, Proportionality | Each plan's lifecycle, one next slice, Q/R dependencies, gate ownership, bounded unknowns and re-plan rules |
| New source/consumer/state boundaries | Architecture and Code Design | Both eight-probe composed-design reviews, authority matrix and deletion/change-locality tests |
| Persisted attempts and cross-process/independent consumers | Contracts/Evolution, Persistence and relevant IPC profiles at implementation | Shared contract, supported-source migration, old-writer disposition, gate evidence and generated-consumer cutover |
| Concurrent transfer, cancellation and cleanup | Concurrency, Rust Async and Rust profile closure at implementation | Single supervised owner, exact generations, capacity, leases, true shutdown and failure results |
| S3/Python/network protocol semantics | Dependencies and Security | Maintained library/tooling decisions, exact source/version/integrity and authorized provisioning; no homemade solver/signer |
| User interaction and generated TypeScript | Frontend, TypeScript/Async, Generated Contract and relevant language-binding profiles at implementation | Real UI path in each owning slice, canonical export, malformed/stale response tests and scope-preserving errors |
| Supported targets and filesystem identity | Cross-Platform, Rust Cross-Platform and platform verification at implementation | Actual native evidence, ordinary files, safe root identity and per-target gates |
| Source/public/installed result claims | Verification and documentation/release obligations where selected | AC matrix with kind/environment/mode/owner; package/readiness limits; durable contract stays proposed until implemented |

The implementation integrator routes the concrete exact write set through the current Router and retrieves its required closure before editing, including conditionally selected diagnostics, protocols/schemas, Rust API/dependency/tooling, documentation, licensing and platform/GUI evidence. The current deliverable records the task facts; it does not claim an automated routing certificate. Routine extra files within an established owner amend that write set instead of starting a broad new investigation.

## Normative links

- [CORE-STANDARDS.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/CORE-STANDARDS.md)
- [STANDARDS-ROUTER.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/STANDARDS-ROUTER.md)
- [workflows/planning.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/workflows/planning.md)
- [workflows/implementation.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/workflows/implementation.md)
- [workflows/verification.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/workflows/verification.md)
- [workflows/development-proportionality.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/workflows/development-proportionality.md)
- [topics/architecture.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/architecture.md)
- [topics/code-design.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/code-design.md)
- [topics/contracts.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/contracts.md)
- [topics/contracts/evolution.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/contracts/evolution.md)
- [topics/dependencies.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/dependencies.md)
- [topics/security.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/security.md)
- [profiles/boundaries/persistence.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/profiles/boundaries/persistence.md)
- [topics/concurrency.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/topics/concurrency.md)
- [profiles/languages/rust/async.md](https://github.com/MrScripty/Coding-Standards/blob/39d55dc330d44ecf940364ceada9d2527f7c7ea0/profiles/languages/rust/async.md)

## Code and intent authority

[Acquisition source audit](codebase-audit.md) records exact new ranges and pinned earlier evidence. The [runtime source audit](../../runtime-installations-and-model-adapters/reports/codebase-audit.md) preserves the original runtime/adapter findings. Repository intent comes from [Generic Model Fetching Backend](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/breif/s3-model-fetch.md) and [Intent/discovery brief](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/breif/intent-discovery-distribution.md). Existing project ownership and gate commands come from [Contributing](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/CONTRIBUTING.md), [Architecture](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/ARCHITECTURE.md) and [Development](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/DEVELOPMENT.md).

## Primary external references checked for this layer design

- [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html): representation and conditional/range response semantics, not a local implementation proof.
- [AWS S3 GetObject](https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html) and [Object](https://docs.aws.amazon.com/AmazonS3/latest/API/API_Object.html): versions, ranges and ETag limits.
- [object_store GetOptions](https://docs.rs/object_store/latest/object_store/struct.GetOptions.html) and [S3 builder](https://docs.rs/object_store/latest/object_store/aws/struct.AmazonS3Builder.html): a candidate's documented interface, not package approval, a version pin or target qualification.
- [pip installation report](https://pip.pypa.io/en/stable/reference/installation-report/) and [pip install](https://pip.pypa.io/en/stable/cli/pip_install/): resolution reporting versus actual local install input.

These primary sources were inspected on September 29, 2026. Pin implementation dependencies to versions actually resolved/tested with Pumas's toolchain; do not infer qualification from current online docs. No third-party source, font, model weights or dependency binary is redistributed in this planning package.

## Delivery verification

The root `validation.json` records checks on this documentation package only: required files/fields, internal links/anchors, distinct plan paths, milestone/gate graph, transferred claim dispositions and ZIP/manifest integrity. It is not a Pumas build/test, standards-engine run, independent design review or a source/GUI/native acceptance result.
