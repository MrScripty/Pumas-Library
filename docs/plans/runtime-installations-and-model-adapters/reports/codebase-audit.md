# Runtime source audit — coordinated revision 4

Baseline Pumas `04e7f1568f00693c0ef26c77e0150e5e4dd112ea` is unchanged. This report carries forward the preceding pinned code review; this round's additional inspection concentrated on the shared acquisition boundary. No new whole-repository or executed runtime qualification is claimed. Source-derived scenarios remain unexecuted unless evidence is explicitly recorded in a later ledger.

| Finding family | Evidence-backed concern | Primary pinned source | Current owner / acceptance |
| --- | --- | --- | --- |
| E01/E02/E11 | Tag-only installed identity and verify-A/reuse-B path; global/PID gate does not represent per-installation use | [rust/crates/pumas-app-manager/src/version_manager/mod.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-app-manager/src/version_manager/mod.rs) | R1, A01/A02/A09 |
| E03/E06/E07 | Generic image capability, repo-specific selection and actual raw-slot UI alternate path | [rust/crates/pumas-rpc/src/handlers/serving_torch.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-rpc/src/handlers/serving_torch.rs) | R3, A03/A07/A12 |
| E04/E05 | Dependency profile/context env_id, hash-based conflicts and modality pins need explicit additive interpretation | [rust/crates/pumas-core/src/model_library/dependencies.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/model_library/dependencies.rs) | R3, A04/A05/A22 |
| E08 | Future profile schema accepted into an old reader/writer; a new schema number cannot protect against that binary | [rust/crates/pumas-core/src/runtime_profiles/route_config.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/runtime_profiles/route_config.rs) | R1/R5, A10/A19 |
| E09/E10 | Fixed sidecar file inventory and partly handwritten tag-specific UI require producer/distribution cutover | [torch-server/probe_runtime.py](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/torch-server/probe_runtime.py) | R2/owning consumer slices, A06/A15/A16 |
| E13/E14 | App-manager already installs native and Python runtimes; generic process factory and version launcher have different assumptions | [rust/crates/pumas-app-manager/src/process/factory.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-app-manager/src/process/factory.rs) | R1, A20/A27 |
| E15/E16 | Provider integration enum is not an open model-adapter registry; current plugin loader overwrites duplicate IDs and reload semantics differ from cached managers | [rust/crates/pumas-core/src/plugins/loader.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/plugins/loader.rs) | R2, A21/A23/A24 |
| E17/E18 | Provider advertises image-only Torch while text code assumes Transformers and directly executes synchronous generation | [torch-server/openai_api.py](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/torch-server/openai_api.py) | R3, A22/A26 |
| E19 | Affected loaders unconditionally allow remote model code | [torch-server/loaders/safetensors_loader.py](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/torch-server/loaders/safetensors_loader.py) | R2/R3, A25 |
| E23–E28 | Generic acquisition brief exists; HF/native/package paths need a shared byte handoff without erasing their consumer policies | [docs/breif/s3-model-fetch.md](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/breif/s3-model-fetch.md) | Acquisition Q1/Q2/Q3 and runtime dependent-consumer tests |

## Preserved strengths and boundaries

Earlier reads established useful staged installation/publication recovery, explicit Torch in-place dependency-repair rejection, owned process generations/listener custody, ready-slot checks and compensating unloads, device custody through cancelled loads, and a valid empty-supplement result. Extend those guarantees; do not duplicate them to satisfy generic naming.

The new acquisition plan now owns the transfer-family refactor and its existing-consumer proof. The runtime plan keeps installed identity, registered code, effective requirements, supported tasks, exact process/load receipts and actual GUI behavior. A working acquisition layer does not close those runtime claims.

## Current additional review

[The acquisition audit](../../artifact-acquisition/reports/codebase-audit.md) identifies the actual model-bound destination and persisted record shapes that prevent treating the existing HF client as a finished generic layer. The shared contract and one-way prerequisite gates replace the old runtime S1a sequencing. New adapter registration continues to be open within the supported host/task contract and independent of Pumas builds; the two plans must not regress that requirement into a bundled allowlist.

## Limits and adoption

Refresh current local/dirty state, public/binding callers, generated output population, native artifact layout, actual legacy stores and old-writer deployments before their cutover. Search alone does not establish that public factory methods are unused. No user installation is an authorized test fixture. The source audit is not a certificate that every source file currently complies with the standards.
