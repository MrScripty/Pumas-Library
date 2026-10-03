# Proposed HTTP acquisition integration baseline

Status: approved implementation plan. The main prerequisite completed green in
Build 37127500248 on 2026-10-03. Integration is source preparation until the
combined branch's independent review and exact-head hosted qualification finish.
No live-store migration or deployment cutover is authorized by this baseline.

## Pinned entry state and scope

Start from B1 merge `58b74e83fdf34131290933f576c7e338bde4a49d`, with tested tree
`57b285ad99994de9e2cc0b83cc62916a3c184ae6`. Its candidate Build 37125867468 passed
all ordinary/native gates and CodeRabbit accepted/resolved all seven findings.
The merge's separate post-merge checks must finish before implementation.

Recheck the current heads and applicable repository/Coding-Standards instructions
at implementation admission. Preserve, without rebase or history replacement:

1. Qualified current main and B1.
2. [PR7](https://github.com/MrScripty/Pumas-Library/pull/7),
   `3b2a279b4d35bd19e5cc1727cc905c7ee7477fa6`, including its 97 original commits.
3. [PR10](https://github.com/MrScripty/Pumas-Library/pull/10),
   `57ed33db8c2995e4633f559d1a8d31d591b0dcca`, based on PR7.
4. Both PR10-based siblings, neither of which subsumes the other:
   [PR15](https://github.com/MrScripty/Pumas-Library/pull/15),
   `dd04fdcf48132ef7237e61feba5e5c1d3629d49a`, and
   [PR16](https://github.com/MrScripty/Pumas-Library/pull/16),
   `f344e7fed8a8cbc6f4c4831ec7c78aa4c44c2d64`.
5. Explicit integration corrections and the cross-owner regressions below.

Use a separate focused integration branch/worktree. The acceptance claim is a
fixture-qualified HTTP acquisition baseline, not deployment migration or release
qualification. S3 follows this baseline. Private ASR artifact authority, public
speech, generalized package/runtime installation, Xet/CAS and fleet management
are not prerequisites.

## Conflict-resolution plan

Core paths below are under `rust/crates/pumas-core/src/` unless stated otherwise.
There are four direct B1/PR7 overlaps, eleven overlaps including other main
changes, and two additional workflow overlaps in the repair history.

| File | Required resolution and owner |
| --- | --- |
| `model_library/download_recovery.rs` | One held filesystem authority. Retain B1's exclusive private staging, complete descendant binding, document bounds, mode settlement, no-replace publication, Windows release/rebind and retained uncertainty. Add PR7's acquisition workspace bridge over the same verified identity. Both typed and raw metadata writers obey bounds. Never substitute generic/ambient recursive cleanup. |
| `model_library/importer.rs` | Keep B1's ordinary copied producer under RuntimeTasks. Retain PR7's separate managed-HF capabilities, selected-file/revision provenance, completion/cancellation arbitration, guarded effects and HF completion receipts. Preserve in-place/idempotent Pending refusal. Do not reacquire a root grant already held by the active owner. Keep B1 fields/test hooks and expanded acquisition-custody checks. |
| `model_library/importer/recovery.rs` | Keep canonical read-only shard observation, staging/Pending exclusions and PR7's stale-orphan recheck at the actual mutation boundary. Discovery grants no import, cleanup or inferred-download authority. |
| `model_library/library.rs` | Combine guarded I/O with canonical copied-publication readiness. PR7's direct `guard.read_metadata` return must not bypass B1's readiness fence. Reads stay bounded/no-follow and held; no ambient fallback. Preserve immutable identity, raw projections, conditional Pending/Ready index commits and HF proof issuance/settlement as a separate protocol. |
| `api/builder.rs` | Main owns primary-instance/startup composition. Keep conditional instance-claim cleanup and read-only startup shards. Install exactly one shared AcquisitionService on HF before restoration and retain it in PrimaryState. Do not restore inferred-shard automatic downloads. |
| `model_library/mod.rs` | Retain discovery exports and remove only the superseded partial_download module. Resolve references rather than restoring an obsolete owner to silence errors. |
| `rust/crates/pumas-rpc/src/main.rs` | Keep main's signal/server lifecycle and add shared acquisition injection into LlamaCpp VersionManager. Inference-disabled RPC must not gain app-manager dependencies. |
| `rust/crates/pumas-rpc/src/server.rs` | Keep accepted HTTP connection/handler ownership, bounded drainage, repeated receipts and retained failures. Drain HTTP/domain owners concurrently. Extend native cleanup to shutdown_installations, then close/drain shared acquisition after consumers, including failure paths. Preserve request admission and private-ASR restrictions. |
| `rust/Cargo.lock` | Retain main's direct hyper/hyper-util ownership and PR7's unicode-normalization/tinyvec entries; no unrelated upgrades. |
| `docs/release-attribution/0.7.0/inventory.json` | Reconcile final inputs using the existing attribution process; retain both dependency additions and newer ownership inputs. Do not weaken notice/license checks or acquire new archives without separate authority. |
| `frontend/src/components/TorchInstallPreview.test.tsx` | Retain main's readyInstallButton, resetAllMocks and delayed-choice regression, which subsume PR7's narrower readiness wait. |
| `.github/workflows/build.yml` | Union PR10 all-base PR coverage, current main/B1 gates and PR16 native-cleanup/workspace gates. Preserve tag/manual expensive release/E2E restrictions and separate Cargo steps on Windows. |
| `scripts/release/check-ci-release-gating.test.mjs` | Retain all-base assertions, native-cleanup requirements and current release/native/feature contracts. |

Textually clean files still need semantic review: mutation_authority, B1
staging/publication/copy planning and index observers, acquisition/store/service/
task_custody, HF download_store/lifecycle, capability_fs and native installer
cleanup. PR15's hook files do not overlap newer main/B1, but retain their full
command-ownership behavior and tests.

### Three distinct persisted contracts

- Acquisition schema7: Transferring -> FilesReady -> Using -> Adopted/Withdrawn;
  owns input bytes, demand/use lease, consumer receipt and queue settlement.
- B1 copied publication: per-model `.pumas_import_publication.json` plus metadata
  identity; owns destination Pending/Confirmed evidence, mode settlement and
  conditional Ready index admission.
- Native installation attempt v2: physical workspace binding and cleanup-pending
  diagnostics; installed output success is separate from reclamation.

Receipts are not interchangeable. A file set is not an imported model; metadata
is not an acknowledged Ready index; installed output is not completed cleanup.
Keep B1's sequence: private stage -> no-replace rename -> Pending index -> exact
payload verification -> mode settlement -> durable Confirmed -> Ready metadata
-> conditional Ready index. HF adoption releases its acquisition lease only after
its own importer completion/output proof settles.

## Five proposed cross-owner regressions

Use an `acquisition_integration_` selector within the existing crate-private test
owners; do not widen production APIs just to expose test hooks. Use real composed
producer paths with synthetic bytes/temporary roots/loopback servers.

1. `acquisition_integration_copied_readiness_survives_guarded_observation`
   - Positive control: genuine confirmed copy remains Ready through guarded I/O.
   - Pending, unreadable/mismatched receipt, erased identity, oversized canonical
     evidence and replacement paths remain unavailable.
   - Overlays, index/cache observers and load-target resolution cannot manufacture
     Ready or follow a replacement.
2. `acquisition_integration_copy_and_hf_exclude_each_other`
   - Successful copied and managed-HF imports on fresh targets.
   - Hold each at a real effect boundary and attempt its competitor; include
     hidden/pending acquisition admission as well as active work.
   - Refusal precedes stage/metadata/network effects, preserves the incumbent and
     does not poison expected shutdown. Fresh permitted work succeeds after true
     settlement. Detect accidental nested acquisition of an already-held grant.
3. `acquisition_integration_completion_receipts_are_not_interchangeable`
   - Real producer outputs, plus wrong/missing/copied-versus-HF receipt evidence.
   - Neither receipt settles the other producer or clears retained custody.
   - Warm/cold observers retain ambiguity without source replay or reimport;
     valid positive control for each protocol.
4. `acquisition_integration_startup_preserves_legacy_and_pending_custody`
   - Schema4/5/6 temporary fixtures refuse ordinary acquisition startup/mutation,
     preserving download-store bytes and making no source requests.
   - Schema7 retained acquisition, copied Pending and incomplete shard evidence
     coexist without migration, deletion, promotion or inferred download.
   - Read-only shard observation remains available without admitting work.
5. `acquisition_integration_shutdown_drains_copy_hf_and_native_consumers`
   - Parameterize the active library owner as copy or HF; do not fabricate two
     simultaneous owners of one exclusive root.
   - Include an admitted HTTP handler and, for default RPC, native installation
     on its own workspace. Cancel caller and one shutdown waiter while held.
   - Consumers settle before shared acquisition closure, without early success,
     deadlock or loss of repeatable failure receipts. Run an inference-disabled
     variant without native managers.

## Exact retained selectors

All commands are proposed hosted qualification, not locally executed evidence.
From repository root, a core selector means:

`cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --lib SELECTOR`

An RPC selector means:

`cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc SELECTOR`

A native-installer selector means:

`cargo test --locked --manifest-path rust/Cargo.toml -p pumas-app-manager SELECTOR`

Check targeted filters with `-- --list`; require real matching cases on each
applicable platform. Zero tests or ignored-only runs are not qualification.

### Core acquisition/custody

- acquisition::manifest::tests
- acquisition::http::tests
- acquisition::service::tests
- acquisition::store::tests
- acquisition::task_custody::tests
- acquisition::workspace::tests
- model_library::download_store::tests
- model_library::hf::acquisition_source::tests
- model_library::hf::download::tests
- model_library::mutation_authority::tests

Retain explicit default/headless admission sentinels:

- local_preflight_refuses_missing_configuration_without_contacting_source
- unconfigured_download_refuses_before_remote_or_destination_effects
- durable_intent_claim_refuses_admission_without_poisoning_shutdown
- ticket_recovery_refuses_busy_before_index_or_download_mutation
- admission_rechecks_hidden_predecessors_after_its_store_transaction

Expected local refusal must preserve state, contact no source and settle cleanly.
Do not suppress aggregate shutdown failures to make the historical ticket case
pass.

### Core publication/recovery

- model_library::importer::tests
- copied_import
- model_library::download_recovery::tests
- model_library::library::tests
- custody_guard_
- canonical_acquisition_custody_blocks_model_mutation_until_withdrawal
- shard_discovery
- shard_startup_tests
- model_library::sharding::tests
- test_recover_incomplete_shards_async_detects_missing_shard_set
- test_find_interrupted_downloads_async_detects_partial_download_dir

Retain the specific composed-owner sentinels:

- real_import_precedes_completion_and_holds_destination_successor
- custody_guard_rechecks_a_real_stale_orphan_scan_after_admission
- builder_requires_download_restore_grant_but_no_client_reads_do_not
- acquisition_integration_startup_retains_separate_pending_custody
- startup_guard_cannot_delete_a_successor_or_promoted_instance
- copied_import_dropped_waiter_and_shutdown_wait_for_held_producer
- copied_import_collision_refusals_do_not_poison_shutdown
- copied_import_pending_and_confirmed_failures_do_not_become_ready_at_startup
- copied_import_public_metadata_edits_and_reinspection_cannot_forge_confirmation
- copied_import_deep_rebuild_and_in_place_retry_preserve_pending_fence
- copied_import_ready_observation_requires_readable_matching_bounded_receipt
- copied_import_ordinary_metadata_edit_preserves_receipt_and_cold_readiness
- copied_import_hashes_destination_once_after_every_callback
- copied_import_oversized_metadata_refuses_before_publication
- copied_import_nested_native_publication_rebinds_exact_payload
- receipt_reopen_settles_after_publication_failure_without_network_or_reimport
- receiptless_using_cold_reopen_retains_custody_without_network_or_reimport
- restored_files_ready_settlement_is_atomic_and_receipt_reopen_does_not_reimport
- cancellation_and_shutdown_drain_real_import_before_settlement

Retain these integration binaries as separate commands:

- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --test intent_ipc_tests
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --test intent_api_tests
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --test package_facts_resolution

### Native installation and RPC

Native-installer selectors:

- native_
- unconfigured_direct_llama_cpp_installer_rejects_before_native_mutation
- public_direct_installer_configures_shared_llama_cpp_acquisition

The native_ cases retain attempt-v2 binding, present-legacy retention, failed-clear
custody, cold reopening, process interruption, progress/caller-loss ownership and
repeatable installation shutdown. Retain acquisition::workspace::tests too.

RPC selectors:

- http_transport::tests
- server::tests

Separate RPC integration commands:

- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --test integration_tests http_admission_rejects_untrusted_sources_before_rpc_dispatch
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --test integration_tests rpc_shutdown_exits_after_delivering_acknowledgement_and_ending_sse
- Linux/macOS only: cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --test integration_tests unix_termination_signals_complete_the_owned_shutdown

The server suite also retains paused/status/cancel/resume HTTP round trips and
real_server_shutdown_closes_native_installation_admission.

### Frontend and contracts

`pnpm --dir frontend test:run src/hooks/useModelDownloads.test.ts src/components/TorchInstallPreview.test.tsx`

Retain PR15's pause/cancel/resume matrix: both failure forms, newer active/paused/
terminal snapshots, overlapping command pairs and both settlement orders.
Run the full frontend and desktop-contract gates afterward.

## Feature/native-host matrix

Ubuntu ordinary qualification:

- ./scripts/rust/check.sh
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --no-default-features
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --no-default-features --features hf-client
- cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features
- python3 scripts/release/check-dependency-features.py

The existing Rust script retains fmt, all-target/all-feature check/Clippy,
workspace default tests, doctests and no-default compilation. Keep its existing
BEAM-tooling exclusion for pumas_rustler. Package-separated no-default tests are
required; workspace feature unification is not inference-disabled evidence.

Native ubuntu-24.04, macos-15 and windows-2025:

- Preserve every current main/B1 native gate.
- Retain/add acquisition::workspace::tests and native_ cleanup qualification.
- Run the five acquisition_integration_ cases in their owning core/RPC packages.
- Retain copied_import, held destination/library metadata, shard/startup, intent
  IPC and HTTP transport suites.
- Windows: nested copy publication, locked stages and reserved-handle/reparse cases.
- Unix: signals and managed process groups. Linux: incomplete process observation,
  router and silent-SSE regressions.
- Core acquisition_integration_ with --no-default-features --features hf-client
  on all three hosts.
- RPC acquisition_integration_ and http_transport::tests with --no-default-features
  on all three hosts. Full no-default package suites remain Ubuntu gates.

Keep dependency ownership, version alignment, attribution, all scripts/release
Node tests, frontend lint/types/tests, Electron validation/tests, launcher tests,
desktop-contract generator/conformance and existing Python/Torch fixture QA.

## Schema and retained-state qualification

Main/B1 download schema5 and B1 copied-publication identity are independent.
PR7 acquisition mutations require schema7. Schema4/5 model projections retain
only their explicitly supported read-only inspection; schema6 rejects ordinary
access. Normal opening must not silently migrate.

Retain core selectors:

- runtime_refuses_old_formats_and_unadmitted_current_rows_without_mutation
- restore_rejects_old_tracking_formats_without_migrating_or_touching_files
- schema_six_rejects_normal_read_without_rewrite
- malformed_restore_inventory_is_an_error_and_preserves_store_bytes
- duplicate_members_fail_model_projection_and_receipt_read_without_rewrite
- duplicate_top_level_members_fail_closed_without_rewrite
- duplicate_consumer_receipt_keys_fail_closed_without_publication
- managed_hf_receipt_settlement_is_one_restart_safe_publication
- receipt_partition_rejects_the_same_acquisition_receipt_under_two_keys
- receipt_stays_with_using_or_adopted_record_in_one_owner_partition

Existing offline conversion unit tests remain isolated-fixture tests only:

- offline_v4_v5_migration_preserves_all_custody_partitions_without_authorizing_pending
- schema_six_migration_preserves_acquisition_and_model_partition_without_receipts
- duplicate_members_fail_offline_migration_without_rewrite
- malformed_schema_v4_upgrade_preserves_original_document
- schema_v4_upgrade_prepublication_failure_preserves_original_document

Do not silently upgrade fixtures to hide startup failures. Native attempt v2
remains separate: present legacy staging is retained without fabricated physical
binding; absent legacy cleanup is harmless; replacements are never adopted.

## Exit and deferred qualification

Exit requires exact integrated commit/tree, independently reviewed semantic joins,
all required ordinary/native checks, real matching targeted tests, five passing
cross-owner cases, applicable external review dispositions, unchanged authority
and readiness guarantees, and strict attribution/feature/release gating.

Ordinary CI does not establish tag-only archives/release binaries/Electron
packages, packaged startup, manually/tag-gated Torch RPC E2E installation, real
models/GPU inference, public ASR, live-store migration, old-writer exclusion,
rollback, networked deployment or a real cluster. No new archive/model acquisition,
credentials, public listener or migration is authorized by this plan.

After this baseline, S3 adds source resolution, addressing/pagination, immutable
object/version or digest evidence, checksum semantics and credential-safe scoped
HTTP access under the same retry, custody, verification and publication owners.
Existing schema5 deployments are not directly upgradeable merely because fresh
schema7 fixtures pass. Any deployment cutover and networked request/stop/drain
qualification remain separate bounded milestones.
