# Remaining import baseline work

These follow-ups remain required before S3/network exposure. They are separate
from the net-improvement integration so an unsafe cleanup or inferred download
expansion is not added as a review shortcut.

## B1: Own ordinary/progress import staging through every outcome

Affected owners: `model_library/importer.rs`, `import`, `import_with_progress`,
`create_temp_import_dir`, and the copy/hash/publication phases.

The progress path already returned through `?` after a failed blocking copy on
main `e37bbf4`, retaining its temporary directory. The ordinary path also ignored
path-based cleanup errors. Exclusive creation now safely refuses additional
filesystem-equivalent names (including Unicode equivalence); a refusal can
expose that pre-existing incomplete cleanup contract. The refusal must remain.

Required repair:

- Hold the created staging directory's physical identity and grant; do not delete
  a replacement path or infer ownership from a name/nonce alone.
- Carry custody through copying, hashing, publication, caller cancellation and
  shutdown. Define cooperating-writer and platform boundaries explicitly.
- Preserve the original operation failure and any cleanup failure. Distinguish
  published output from retained/cleanup-pending workspace status.
- Exercise ordinary and progress imports, normalized and filesystem-equivalent
  collisions, copy/hash failures, cancellation, replacement sentinels, renamed
  original directories, successful publication, and platform-specific cleanup.

Do not implement this with an ignored `remove_dir_all(temp_dir)` result. No
existing store is migrated or cleaned merely by tracking this work.

### B1 implementation and remaining acceptance gates

The copied-import implementation is in `model_library/importer/staging.rs`.
Ordinary, progress and copied-Diffusers imports share one admitted
`RuntimeTasks::run_owned` operation with one awaited `run_blocking` producer.
The root grant and stage capability remain inside that producer through
preparation, no-replace publication, and cleanup. No cleanup can race a live
copy/hash waiter. Progress uses nonblocking best-effort delivery.

`DownloadDestinationRoot` exclusively creates a private stage, and
`DownloadRecoveryDestination` owns its no-follow file operations, physical
identity checks, publication and deletion. The shared rename helper now records
successful publication separately from later durability/visibility failure.
The legacy relocation wrapper preserves its previous error interface. Runtime
binding projection continues through held metadata with the final model ID.

This excludes competing cooperating Pumas writers through the existing root
grant. It does not claim protection against arbitrary hostile mutation by
another process with equal filesystem authority. Observed root, ancestor or
stage replacement refuses publication/deletion. A renamed original may remain
outside its original location; neither a nonce scan nor pathname reacquisition
is recovery authority. Failed cleanup is a terminal explicit retained/unknown
outcome, not a promised automatic retry. Existing abandoned stages are not
migrated or deleted.

Consumer disposition:

- Production copied-import calls are `api/models.rs::import_model` and
  `api/state.rs` dispatch, using the importer composed in `api/builder.rs`.
- RPC, UniFFI and Rustler route import calls through that owning API. Inspected
  Pantograph production-dispatch UniFFI/Rustler adapters also call
  `PumasApi::import_model`; its direct `ModelLibrary::new` fixture is read-only
  update-feed setup, not copied-import admission.
- In-repo importer fixtures now install the real `RuntimeTasks`, destination
  root and `LibraryMutationAuthority` composition. No test-only runtime fallback
  is exposed to production.
- The new Config refusal is specifically for copied imports. External-reference
  registration (`import_external_diffusers_directory`), in-place import,
  download finalization and recovery retain their existing owners/contracts and
  require separate ownership review; this slice does not expand or guard them.
- B2 below is unchanged, including its download-authorization limitation.

Independent review follow-ups:

- Copied Diffusers layout includes exclusively created, held empty directories.
  Source validation selects the copy strategy only. The shared validator reads
  `model_index.json` and referenced components through staged capabilities before
  metadata may claim Ready/Valid. Empty-component and changed-staged-index
  regressions reopen the published result through the normal reader.
- Import cleanup validates every observed descendant-directory binding and the
  complete directory inventory before deleting any payload. Unknown or replaced
  descendants retain the workspace and replacement sentinel. Physical identity
  is used when readdir spelling differs on Unicode-normalizing filesystems.
  This is import-specific; generic download deletion retains its prior contract.
- Publication repeats the complete held-directory and copied-file presence/size
  proof after the final metadata notifier and test hook. It revalidates staged
  Diffusers contents and requires exact `model_index.json` bytes to match the
  snapshot that produced metadata/runtime hints. No external callback occurs
  between this final proof and rename. A valid, same-size index rewrite is still
  refused and retained after payload evidence was captured; removed/replaced empty components retain unknown custody
  without deleting replacement sentinels. This is the cooperating-writer grant
  contract, not protection against arbitrary equal-authority hostile mutation.
- Copied metadata preserves final model ID, final bundle source/entry paths,
  generic recommended-backend normalization (shared with `save_metadata`), task
  projection, custom runtime bindings and active dependency references. The
  existing record-type fallback only applies when metadata lacks model_type;
  copied metadata explicitly supplies it. External-reference revalidation does
  not apply to the LibraryOwned copied path.
- Held metadata writes invoke the existing write notifier using display paths
  solely as watcher observations. They never derive filesystem authority from a
  callback path. Pending and final Ready metadata writes both notify before their
  last payload proof. ONNX persisted/indexed recommendations and notifier
  replacement-sentinel regressions cover these compatibility boundaries.
- New stages have no previous authoritative metadata to back up. Custom metadata
  projection is prepared before publication; an unchanged post-publication
  projection needs no replacement or backup. No existing model metadata is
  overwritten: final directory publication remains exclusive.

### Native Windows publication correction

A native Windows diagnostic established that cached descendant directory handles
caused `AccessDenied(5)` at directory rename; releasing those handles let the same
held-parent rename succeed. The production correction is import-specific:

1. Stream SHA-256/BLAKE3 while copying. Retain the configured root/stage IDs,
   created directory identities (including empty components), output descriptor
   identities, lengths and digests. Check the complete namespace against that
   evidence without rereading payload bytes.
2. Exactly four root documents are mutable owner documents outside payload
   evidence: `metadata.json`, `metadata.json.bak`, `overrides.json`, and
   `.pumas_import_publication.json`. Original source names, normalized destination
   names, and native filesystem-equivalent aliases are refused. Ordinary metadata
   backup behavior is preserved. Overrides currently use `keep_backup=false`;
   `overrides.json.bak` is not silently excluded. UUID temporary files created by
   the atomic writers receive no wildcard exemption.
3. Persist Pending receipt and Pending/Invalid metadata. Complete structural
   validation, release only descendants, and perform held no-replace rename.
   Rebind the exact recorded IDs, sizes and complete namespace after success or
   failure. A changed binding cannot authorize cleanup. Successful native rename
   never becomes rollback authority.
4. Keep receipt and index Pending while dependency projections and the final
   metadata notifier complete. Then perform **one full destination hash pass**,
   including exact namespace/binding and bundle-content checks. A private proof
   is consumed by a callback-free owner: durable Confirmed receipt, durable Ready
   metadata, and conditional final index commit. Any uncertain outcome reports
   the retained publication and does not claim success.

Primary metadata and receipt share an explicit **16 MiB document limit**, enforced
both before producer publication and by bounded no-follow observation. There is
no truncation or fallback to legacy on malformed/oversized protocol evidence.
Every public readiness decision requires canonical primary metadata, receipt/root
identity, and matching indexed publication state. A Ready index or cached summary
cannot conceal missing, malformed, identity-erased, or replaced canonical metadata.
Backups and effective overlays cannot replace the primary publication gate.

Conditional projections capture the source row before filesystem observation and
compare it inside a short SQLite transaction. No filesystem operation, callback,
root-grant acquisition or await occurs under that transaction. Stale Pending
watcher/rebuild results cannot overwrite producer Ready; only the producer may
advance its existing Pending row. A canonical-damage observation can invalidate
only the Ready row it observed. SQL update events are published after commit.
Deep rebuild preserves all protocol rows and conditionally prunes legacy rows,
so it cannot erase a Pending fence and subsequently cold-adopt its Ready file.

A visible Ready file after unacknowledged final metadata durability already has a
verified durable Confirmed payload receipt. The import still reports uncertainty
and its existing Pending index remains fenced. With no indexed owner, discovery
may adopt an **already Ready** primary only after full payload verification.
A failed cold proof is reported as unadopted, without creating a permanent Pending
row from a temporary observation. Actual Pending primary/index state never
advances automatically. Ordinary metadata and override edits require finalized
index admission first, so legitimate atomic writer temporary files cannot race a
cold scan. Existing Ready rows use bounded document observations without scanning
those temporary files; unknown temporary-looking names still fail cold proof.

Readiness integration includes canonical/effective metadata, public list/get/search
projections, execution descriptors and package reinspection, both artifact
resolution modes, `PumasReadOnlyLibrary`, public selector/summary snapshots,
watcher/index projection, startup/ordinary/deep rebuild, metadata/review/override
edits, in-place import, and owned metadata writers in reclassification, duplicate
maintenance and migration. Snapshot SQL captures publication fields with each row;
filesystem checks happen after the connection lock is released. Raw `ModelIndex`
methods remain trusted index-only primitives, not filesystem readiness observers.
Plain pathname/file enumeration helpers do not assert execution readiness.

Cross-root merge refuses new protocol assets before moving their files. An
explicit re-publication workflow is not provided. Same-root reclassification and
migration retain the receipt identity and use a conditional owned index-ID remap,
including durable references. Its original `model_id` and stage pathname are
provenance; physical root/stage identities are authority. A UUID still indexed at
another model ID blocks cold adoption at a moved path. Passive damage observations
cannot downgrade a source protected by the existing mutation claim during its
owned transfer. The public reclassification regression covers this transition.

A receipt with missing, malformed or identity-erased metadata remains diagnostic
protocol data. It is not adopted as a legacy orphan. Existing traversal recognizes
receipt-backed directories; orphan traversal prunes `.tmp_import_` subtrees.
In-place PreserveExisting checks the same Pending index fence before reporting
idempotent success. No database migration or live-store repair occurs.

Operational cost: successful copied imports hash during the source copy and read
the destination payload once for final verification. The streamed digests also
supply ordinary primary-file metadata, eliminating its separate hash read. There
are multiple inexpensive namespace/identity walks and bounded metadata/config
reads. Streaming replaces potential kernel copy offload and adds digest CPU work;
throughput still needs measurement. Exceptional cleanup can add a verification
pass. Corruption discovered after rename leaves published Pending data instead of
an unpublished cleaned workspace. Cold adoption/rebuild without a matching Ready
row hashes the payload; existing Pending rows stay fenced, and matching Ready rows
use bounded canonical/receipt reads. Public cached snapshots therefore add small
filesystem observations for protocol evidence, never full payload scans. Ongoing
payload freshness after successful confirmation remains package inspection's job.

There is **no supported reconciliation/finalization API for retained Pending
imports in this change**. Only the admitted live producer may finish its own
transition. Terminal Pending data requires manual diagnosis; retaining a receipt
is not a promise of automatic repair or a recommendation to delete/reimport it.
The root grant excludes cooperating writers; no arbitrary equal-authority hostile
mutation guarantee is claimed across handle release or any later filesystem edit.

Revision regressions cover canonical damage with pre-existing Ready cached facts
before any reindex; stale watcher and independent-connection conditional writes;
Pending preservation through deep rebuild and in-place retry; override edits,
backups and temporary-file cold admission; shared document bounds; original and
normalized reserved names; source/target generation preservation during the public
same-root reclassification path; and pre-mutation cross-root merge refusal.

Compile-plausibility audit: in-repo `ModelMetadata` construction uses defaults or
struct updates; the new optional field requires no exhaustive literal repair.
`ModelMetadata` has no UniFFI/contract-schema derive. RPC stored/effective metadata
is already `serde_json::Value` with `DesktopJsonValue` object properties; the new
identity contains only a version integer, UUID string and boolean. No generated
binding enum/schema is changed. Hosted contract generation remains a required gate.

Acceptance remains **verifying**, not accepted: local `rustfmt` parsing and
`git diff --check` passed; no Rust compilation or tests were run locally because
of the shared resource budget. Parent-owned hosted CI and independent review
must qualify the exact proposed commit. Commands from `rust/`:

Independent source review cleared the frozen production implementation at
`5e9c47ef7e57ff294a57bf1c13aec38aa595af61` after finding and correcting nested
replacement cleanup, empty-component/staged-validation, generic backend metadata
and final callback/publication issues. Native CI explicitly runs copied-import,
held-destination and metadata-projection suites. A Windows-only real open-handle
fixture forces cleanup failure after copying a read-only source, checking both
original and cleanup errors, preserved source/output attributes and retained
workspace/shutdown failure. This is prepared coverage, not yet a native pass.

```sh
cargo test -p pumas-library --no-default-features --features hf-client --lib copied_import -- --nocapture
cargo test -p pumas-library --no-default-features --features hf-client --lib model_library::importer
cargo test -p pumas-library --no-default-features --features hf-client --lib model_library::download_recovery
cargo test -p pumas-library --no-default-features --features hf-client --lib model_library::library
cargo test -p pumas-library --no-default-features --features hf-client --lib index::model_index::model_selector_snapshot
cargo test -p pumas-library --no-default-features --features hf-client --lib api::runtime_tasks
cargo clippy -p pumas-library --no-default-features --features hf-client --all-targets -- -D warnings
```

Run native Linux, macOS and Windows jobs. Capture the case/Unicode equivalence
oracle output for the actual mounted filesystem; a filesystem reporting distinct
names does not qualify an equivalent-name refusal claim on another filesystem.
Required native gates also include Unix mode/umask, no-replace rename, post-rename
fsync failure, Windows held-handle/reparse replacement and read-only output
semantics. Source inspection, formatter success, cross-compilation and a native
Linux pass are not Windows/macOS runtime qualification. Existing generic held
capability replacement tests and new import integration fixtures both apply.

## B2: Discover recoverable shards only under a proven model root

Affected owners: `model_library/importer/recovery.rs`, its discovery callers and
`api/builder.rs`'s startup recovery action. The recovery implementation was
unchanged by the integration: it enumerates only immediate files. Deeper
incomplete sets were already invisible to recovery, while the old import path
could incorrectly accept them. The new recursive all-set validation deliberately
fails closed instead of publishing an incomplete model.

Required repair:

- Select an authoritative canonical model root before recursively enumerating
  files. Do not treat category/family ancestors or arbitrary nested directories
  as another inferred repository root.
- Preserve root-relative directory identity while grouping every shard set;
  reject malformed, ambiguous or unobservable input with a visible diagnostic.
- Keep discovery separate from download authorization. Current startup recovery
  guesses a remote repository from path names; do not expand automatic network
  actions based on newly recursive guesses.
- Test multiple models under the same family, nested sets sharing a basename,
  complete and incomplete sets together, legacy/ambiguous layouts, enumeration
  failure, and absence of unintended download admission.

Until this owner boundary is implemented, nested incomplete imports remain
refused. No new automatic recovery/download capability is claimed by PR22.
