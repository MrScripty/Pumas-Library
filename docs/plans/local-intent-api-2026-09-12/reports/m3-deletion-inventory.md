# M3 deletion and relocation integration inventory

Status: source findings for M3b; no deletion behavior changed in M3a.

The private M3a claim transaction is a persistence primitive. It does not protect
filesystem effects until these existing operations cooperate with it:

| Existing path | Required M3b behavior |
| --- | --- |
| `ModelLibrary::delete_model`, `model_library/library.rs` | Claim the model before index/link/file removal; reject retained targets; retain the exact claim through owned completion or fail-closed recovery. |
| `PumasApi::delete_model_with_cascade`, `api/links.rs`, and dispatch in `api/state.rs` | Delegate to the same guarded library operation; do not introduce a separate API-only check. |
| `ModelLibrary::cleanup_duplicate_repo_entries` | Guard removal and partial-payload merging of retained source/target identities; a retained duplicate is not permission to delete its declaration or substitute another model ID. |
| `ModelLibrary::reclassify_model` | Its directory rename changes model ID before deleting the old index row. Require retention-aware relocation semantics or a typed conflict before moving retained targets. |
| `LibraryMerger::merge_single_model` and source cleanup, `model_library/merge.rs` | Moving payloads/removing source directories must respect the source library's declarations and claims; merely guarding destination import is insufficient. This file needs a narrow M3b write-set amendment before edits. |
| `ModelIndex::delete`, `clear`, startup stale-row removal and FTS rebuild | These are projection operations. Declarations and deletion claims must survive them; an absent projection row does not prove no retention or no deletion custody. |

A guard implemented only as a retention SELECT followed by asynchronous file
removal races with a concurrent ensure. An in-memory lock released when a caller
future is dropped also cannot prove that spawned filesystem effects have stopped.
The durable claim must be acquired atomically against target retention and
settled by the existing operation owner only after all effects are observed.
Claims must not expire automatically after a crash.

Before public ensures are enabled, M3b must test actual deletion/reclassification
and cancellation races, rather than inferring filesystem exclusion from M3a's
SQLite transaction race tests. The claim also must compose with the existing
managed download destination authority: it is not a replacement for its writer
reservation or recovery validation. Unsupported historical downgrades cannot be
made safe merely by a schema marker ignored by old binaries.
