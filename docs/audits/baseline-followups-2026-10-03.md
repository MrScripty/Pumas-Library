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
