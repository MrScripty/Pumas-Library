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

## B2: Read-only canonical-root shard discovery

Implementation is present; native acceptance remains pending the focused Rust
suite on Linux, macOS, and Windows. Formatting and source review are supporting
evidence, not native filesystem or startup execution evidence.

Owners: `model_library/importer/recovery/shard_discovery`, the public discovery
DTOs in `importer.rs`, and the shard-only observer in `api/builder.rs`.

`ModelImporter::discover_shard_recovery[_async]` returns a
`ShardRecoveryDiscovery`. It inspects only the canonical three-component storage
layout `{model_type}/{family}/{model_or_artifact}`. Category/family names use the
existing ordinary normalization; model names also permit the existing artifact
slug separators. Model/index/metadata evidence above the model-root boundary,
noncanonical names, nested package markers, and symlinks receive diagnostics.
They are never used to guess another repository root. Metadata-bearing model
roots are observed too; metadata presence is not a completeness shortcut.

The configured library root may use a canonical platform alias. Discovery binds
its physical identity, holds the canonical ancestor chain, and reads descendants
through held no-follow directories. It rechecks observed bindings before/after
reads and before returning. An observed replacement invalidates the affected
root's evidence. `BindingChanged` also covers a binding check that could not be
observed (for example, a missing or inaccessible component); it does not assert
that an actor replaced a directory. There is no library-ID initialization,
marker, database, cache, cleanup, or network write. This is point-in-time evidence, not an immutable
snapshot or a grant for a later filesystem action. Concurrent in-place changes
can still require a fresh scan and authoritative package validation.

Held descendant directory capabilities live only along the active traversal
path. Completed bindings retain physical identity and root-relative path, not
handles; final checks reopen one binding at a time through the existing
no-follow helper and invalidate overlapping model evidence on mismatch or
unobservable custody. This bounds retained handles by root-ancestor plus
traversal depth and allows already-scanned unrelated Windows subtrees to be
renamed before the entire scan finishes. The bounded entry/depth limits still
apply; identity observations do not become capabilities for later actions.

Every counted set retains its full model-root-relative directory and
base/extension. Zero, out-of-range, duplicate, inconsistent, overflowing,
malformed, and uncounted/ambiguous shard names remain diagnostic. Missing
ordinals are inclusive compact ranges, so the declared total never determines
allocation size. Weight-map indexes are read through held directories with a
16 MiB per-index limit; invalid/duplicate maps, unsafe references, absent
conventional indexes, and unobserved referenced files remain visible. Filename
coverage never asserts global package completeness or repository identity.

Enumeration/metadata failures, refused layouts, symlinks, the 100,000-entry scan
budget, the 64-level model traversal budget, and failed blocking workers cannot
become an empty successful scan. Callers must inspect `enumeration_complete`,
per-model observations, and diagnostics together.

Startup logs the report and admits **zero downloads from inferred shard
candidates**. Its observer receives only `ModelImporter`, without an HF client
or download admission capability. Existing persisted authorized recovery and
the separate interrupted-download owner are unchanged; this repair makes no
new safety claim about those owners. Restoring missing orphan shards requires
an explicit recovery/download workflow with independently established source
and destination authority.

The old public `recover_incomplete_shards[_async] -> Vec<IncompleteShardRecovery>`
helpers remain deprecated, lossy projections of this one scanner. They warn on
incomplete/ambiguous observations and retain unverified path-derived name hints
only for compatible Rust callers. Their filenames are now root-relative,
including nested directories. An empty Vec cannot establish completeness, and
the reconstructed `repo_id` never authorizes a download. There is no second
legacy scanner or implicit acquisition fallback.

Regression sources cover sibling model roots, nested identical basenames,
complete and incomplete sets together, legacy/ambiguous layouts, failed
iterator creation/items/completion, root/ancestor/child rebinding, symlink
sentinels, malformed ordinals and indexes, missing index evidence, bounded
missing ranges, read-only observations, and the exact startup shard observer.
The existing persisted-download recovery tests remain with their unchanged HF
owner. The incomplete-import validator and B1 copy-staging owner are unchanged.

Native rebinding regressions require the attempted rename to succeed before
binding. Where held Windows descendant handles reject that same rename, only
access-denied/sharing-violation errors are considered refusal candidates. The
test must also prove unchanged source/sentinel names and bytes and a successful
same-path rename after discovery drops its handles. This establishes native
pin refusal rather than pretending rebinding occurred. Successful mutations
still require stale-evidence rejection. The startup observer regression checks
its actual byte/name effects; zero acquisition authority is established by its
`ModelImporter`-only input and the reviewed production call boundary, not an
unconnected test download client.

The broad-tree regression observes the scanner's own `HeldDirectory` capability
lifetimes with a per-scan RAII high-water counter, independent of process-wide
file-descriptor activity. Its 48-model, 12-component fixture permits only four
active descendant capabilities above the selected root's ancestor chain and
requires all of them to be released on return. This counter measures retained
scanner capabilities; the existing no-follow helper's transient handles are a
separate bounded implementation window. A real rename during later sibling
traversal must succeed on Windows as well as Unix, and final identity checks
must reject the changed model while preserving unrelated observations. Symlink
replacement after traversal remains no-follow. These new regression sources
require native execution; source review and formatting alone do not qualify
this resource-lifetime correction.
