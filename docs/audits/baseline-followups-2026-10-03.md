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
  refused and cleaned; removed/replaced empty components retain unknown custody
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
  callback path. Unchanged post-publication dependency projections skip another
  write/notification. ONNX persisted/indexed recommendations and notifier
  replacement-sentinel regressions cover these compatibility boundaries.
- New stages have no previous authoritative metadata to back up. Custom metadata
  projection is prepared before publication; an unchanged post-publication
  projection needs no replacement or backup. No existing model metadata is
  overwritten: final directory publication remains exclusive.

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
