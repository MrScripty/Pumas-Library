# Source audit supporting the paired layer design

**Date:** 2026-09-29. **Pumas:** `04e7f1568f00693c0ef26c77e0150e5e4dd112ea` (`work/torch-version-management`). **Standards:** `39d55dc330d44ecf940364ceada9d2527f7c7ea0`. No repository source, user environment, branch, installed artifact or production test was changed or executed.

## Method and extent

The attached revision-three plan and remote-acquisition review were read in full. Current GitHub branch and standards refs were checked. The Pumas feature branch is unchanged. Standards advanced one commit from `91ceb0fe`; the inspected comparison contains regression tests, their plan records and generated suite inputs, not changes to the previously read normative Core/Router/Planning/Architecture contracts.

This round additionally inspected the actual destination/recovery types, download-store records and task lifecycle, plus the core manifest. Prior source findings at the unchanged Pumas commit are carried forward with their scope. This is a bounded invariant-family audit; it does not claim every repository file, deployment, native target or external caller was inspected. Search results are navigation, never proof of complete consumer absence.

## Findings and resulting design decisions

| ID | Source finding / review result | Material consequence | Plan disposition |
| --- | --- | --- | --- |
| AQ-I01 | `DownloadRecoveryDestination`, `DownloadDestinationRoot` and recovery tickets carry model-relative identity; the root machinery validates `.pumas-library-id.json`. | Directly exporting that type as a generic wheel/archive destination would preserve model-specific authority. | Extract capability-backed workspace mechanics without making model ID/UUID mandatory; leave model authorization at its consumer. Q1, AC01/AC04/AC09. |
| AQ-I02 | `PersistedDownload` contains `repo_id`, `DownloadRequest`, optional HF evidence and model-root destination identities; its versioned store also owns exact-attempt admissions, releases and uncertainty. | A nullable-field patch is not enough; neither discarding the store nor duplicating it is justified. | Generalize the owned record at an explicit migration boundary, preserve retained custody/tombstone/refusal semantics, and give migrated attempts one writer. Q1, AC04–AC06. |
| AQ-I03 | `hf/lifecycle.rs` registers task roles/generations and destination queues; pointer identities prevent stale tasks acting on successors. | A new `download()` future plus Drop cleanup would lose required supervision. | Reuse that lifecycle depth and isolate neutral execution identity from source/model-specific inputs. Q1, AC05/AC06. |
| AQ-I04 | Prior pinned inspection shows HF owns model importer completion as well as byte work; native installer and `network/download.rs` contain other byte loops. | Shared acquisition cannot equate byte success with imported/installed completion or merely rename the HF client. | One transfer owner; separate domain commits; real HF/native bridge in Q1. AC03/AC05/AC08. |
| AQ-I05 | The runtime r3 plan contains generic acquisition as S1a, while runtime consumers are needed to prove it. | Splitting plans carelessly could create two authorities or a prerequisite cycle. | Acquisition Q1 uses existing native behavior, Q2 existing package behavior; runtime R1/R2 wait on accepted gates. |
| AQ-I06 | Runtime/package artifact selection includes build/origin/hash policies and package-tool resolution. | A generic S3/HTTP read does not prove executable approval or Python compatibility. | Preserve package/native policies; Q2 validates exact acquired local inputs with denied network during installation. |
| AQ-I07 | Current core directly uses reqwest, Tokio, sha2, capability filesystem and atomic store mechanisms; current no-default feature flags are not a full minimal-core implementation. | There is a reuse path without a new service/crate, but no basis for claiming all old dependencies disappear. | Neutral core module, optional source dependency, measured feature/target claims only. Q1/Q3/Q4. |
| AQ-I08 | The source brief's scope includes S3 and future Xet/peers/CAS, but source manifests appear after transport phases in its suggested sequence. | Different consumers need file identity and handoff before concrete transport expansion. | Establish the minimal manifest and consumer contract in Q1; Q3 adds S3; future reconstruction/peers/CAS remain explicit deferrals. |

### Exact source population inspected this round

- [rust/crates/pumas-core/src/model_library/download_recovery.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/model_library/download_recovery.rs) — lines 1–290: model/library markers, recovery values and held destination/root capability.
- [rust/crates/pumas-core/src/model_library/download_store.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/model_library/download_store.rs) — lines 1–235: schema 5, model/HF snapshot fields, exact admissions and uncertain publication.
- [rust/crates/pumas-core/src/model_library/hf/lifecycle.rs](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/model_library/hf/lifecycle.rs) — lines 1–235: task generations, destination claims and queue/lifetime ownership.
- [rust/crates/pumas-core/Cargo.toml](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/Cargo.toml) — full returned manifest: dependency/feature placement.

### Carried-forward pinned evidence

The preceding reviews read [the generic-fetch brief](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/breif/s3-model-fetch.md), [the model intent brief](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/breif/intent-discovery-distribution.md), [HF owner](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/model_library/hf/mod.rs), [HTTP helper](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-core/src/network/download.rs), [native installer](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/rust/crates/pumas-app-manager/src/version_manager/installer.rs), [package resolver/installer](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/torch-server/resolve_runtime.py), and [active recovery plan](https://github.com/MrScripty/Pumas-Library/blob/04e7f1568f00693c0ef26c77e0150e5e4dd112ea/docs/plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/plan.md). These establish existing paths and limitations, not passing evidence for new code.

The existing source brief is not an already implemented acquisition service. In particular, accepted recovery checkpoints are not permission to replay still-unaccepted Pending cleanup. Existing namespace and directory spelling are preserved.

## External mechanism facts used to constrain the plan

HTTP partial response assembly requires representation/range evidence; an ignored range is not an append operation. S3 version selection, range behavior and ETag semantics require a source-specific adapter rather than guessed generic checksum fields. Official references are in the [shared contract](../../../contracts/artifact-acquisition.md); they are source semantics, not Pumas conformance evidence.

The maintained `object_store` API is a plausible S3 implementation candidate, not a dependency installed or approved by this task. Exact target/features/license and operational checks remain Q3 admission. Supported pip reports describe resolutions but are not accepted as install input themselves; Q2 must prove its selected local-input procedure.

## Informed layer boundary

The shared state-machine and workspace invariants belong below both model import and runtime install. Model/release/package selection stays above acquisition; source access varies below it. A lower-level reader transfers protocol bytes but does not own another acquisition lifecycle. Verified-file custody connects the layers without collapsing their completion or rollback authority.

A new source then changes source integration and its evidence. A new model adapter changes adapter metadata/code and its execution evidence. A new runtime build changes native/package selection. None should require a new transfer state machine. That is the design claim to examine during implementation, separately from whether tests are green.

## Remaining bounded evidence needs

Before Q1 source writes: current local/dirty state, exact direct producer/consumer/external API population, supported persisted source states and old-writer disposition, destination-grant extraction and actual native integrity policy. Before Q2: actual package-local-input translation and network denial. Before Q3: maintained S3 implementation/target decision and real endpoint credentials. Missing facts prevent only their corresponding unsafe action or acceptance claim; they do not authorize weaker fallback or a new parallel implementation.
