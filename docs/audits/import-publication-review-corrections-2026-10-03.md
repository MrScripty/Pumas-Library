# Copied-publication library review corrections

This bounded follow-up starts at local `72b809096186b5cb8f5ed2d41ad821254b0faab4`
(tree `e7ca8110925f8c743c4ee51f6d9c5d648fda70cb`), equivalent to PR 28 head
`48d86d91f3da94a2d0703ff936e8c18d98448655`. It addresses supplied review
comment identifiers `4172968746`, `4172968749`, `4172968738`, and `4172968698`
on [PR 28](https://github.com/MrScripty/Pumas-Library/pull/28).
Source inspection supplies the implementation evidence below.

## Scope and outcomes

Production changes belong only to `model_library/library.rs`, its migration
owner, and a private publication-observation helper. Import copying, hashing,
filesystem custody, merge, and receipt production are unchanged by this slice.

1. Duplicate cleanup checks repo-key eligibility before requesting publication
   edit authority. A typed `Validation` refusal for `import_publication` retains
   that model and emits a per-model diagnostic; independent eligible entries
   continue. `DuplicateRepoCleanupReport.blocked_models` contains model IDs and
   actionable errors, including location. It is serde-defaulted for old report
   JSON. Existing duplicate/removal counters continue to describe the eligible
   entries actually considered by deduplication, while this new list separately
   reports entries excluded by the publication fence. Unexpected errors still
   abort and propagate. In-repository consumers obtain this report from the
   operation or `Default`; there are no exhaustive literals or generated
   contracts for this report in the checkout.
2. A pre-effect source or conversion-target publication refusal becomes the migration
   result `blocked_import_publication`, with the original actionable error. It
   contributes one to both `skipped_move_count` and `error_count`, never to
   `completed_move_count`. A blocked run has `completed_at: null`. Independent
   moves continue, and successful prior checkpoint results remain successful.
   The checkpoint retains blocked terminal attempt results even when its
   pending-move queue is empty. Explicit resume preserves those results and
   does not re-run or silently finalize blocked publications. This does not
   add a Pending recovery or checkpoint-unblocking API; retained publication
   evidence still requires manual diagnosis. The existing internal field
   `completed_results` stores completed attempts, including blocked/error
   outcomes, rather than asserting that every recorded move succeeded. A
   publication failure after rename keeps its error outcome and is not
   mislabeled as a skipped pre-effect refusal.
3. Public record observation decodes only publication authority fields. It
   overlays readiness state/validation diagnostics onto the original JSON;
   unrelated omitted fields and unknown keys are not round-tripped through
   `ModelMetadata`. Existing raw publication identity is preserved, including
   malformed evidence. Malformed authority fields make that row diagnostic and
   unavailable, without failing unrelated list/search records. Protocol-row
   refresh uses the same raw-preserving conditional projection. Subsequent
   dependency/backend projection leaves unavailable publication evidence intact
   rather than reparsing it to derive runtime facts. Database and
   genuine IO failures propagate in this record-observation path; missing or
   malformed evidence remains unavailable. Other metadata readers retain their
   existing observation policy.
4. Rebuild conditionally prunes undiscovered legacy rows without then refreshing
   the deleted snapshot. A prune conflict leaves the newer row untouched.
   Undiscovered protocol rows remain indexed. Invalid indexed paths become
   per-row validation diagnostics while their original identity/path evidence
   stays intact; unexpected filesystem or database errors still propagate.
   Rebuild reaches its existing WAL checkpoint after ordinary stale-row pruning.

All conditional index writes retain the previously observed row token. The
new code performs no filesystem work or callbacks under an index transaction.

## Consumer integration disposition

The bounded write set does not change these adjacent consumers:

- `api/reconciliation.rs` and `examples/repair_library_integrity.rs` consume
  duplicate cleanup counters without exhaustive report construction. They do
  not yet display the new blocked-model list; the operation itself emits
  per-model warnings, and direct callers receive the list. External Rust
  consumers constructing this public report exhaustively must initialize the
  additional field or use `Default`; old serialized reports deserialize with an
  empty list. No in-repository generated schema includes this report.
- `api/migration.rs::recompute_execution_report_counts` runs when partial-download
  outcome wording changes. Its current fallback counts the new blocked action
  as an error but omits its skipped count. The parent integration owner must
  preserve both counters when blocked publications and partial downloads occur
  in one report. `api/state.rs` invokes this same aggregation helper.
- `MigrationReportsPanel.tsx` currently decides its success wording from
  `referential_integrity_ok` alone. That can label a blocked migration complete
  even when `completed_at` is absent and errors are reported. The parent owns
  updating this interpretation. `MigrationReportSummaries.tsx` already displays
  the completed/skipped/error aggregates. The frontend report type already
  allows null `completed_at`, and migration item actions are strings, so the
  blocked action requires no schema expansion.

These integration findings remain acceptance requirements for the combined
change; this source slice does not claim the unchanged UI presents them fully.

## Evidence and qualification

Regression source was added before implementation in `library/review_tests.rs`.
Every new case matches the existing native `--lib copied_import` filter:

- `copied_import_cleanup_skips_only_eligible_blocked_models`
- `copied_import_cleanup_preserves_unexpected_metadata_errors`
- `copied_import_record_observation_preserves_raw_projection_fields`
- `copied_import_malformed_index_identity_is_diagnostic_not_collection_failure`
- `copied_import_record_observation_preserves_io_and_database_errors`
- `copied_import_rebuild_prunes_invalid_legacy_paths_and_retains_protocol_evidence`
- `copied_import_migration_resume_retains_source_and_conversion_blocks`
- `copied_import_migration_preserves_unexpected_error_and_checkpoint`

Fixtures use isolated temporary roots and synthetic retained Pending evidence.
They do not represent live-store repair or supported Pending recovery. The
resume case asserts unchanged source/conversion receipt bytes, retained blocked
checkpoint results, accurate counts, and an independently completed move across
two explicit executions. The stale-row case also observes WAL truncation.

Local qualification is limited to Rust formatting, source/callsite inspection,
and Git whitespace/diff review. No Rust compilation, test execution, native
filesystem qualification, real store/model operations, or remote writes are
performed in this slice. Independent source review and fresh hosted Linux,
macOS, and Windows gates remain mandatory before acceptance.


## Combined review follow-up

The caller corrections identified above are included in the combined branch:
API recounting preserves skipped plus error counts, and the panel requires
completed checkpoint evidence and zero outstanding/skipped/error work before its
completion message. See the adjacent import-copy review correction record for
local frontend results. This supersedes the caller-pending disposition of the
isolated source milestone.

Independent review added three record-observation edge regressions. Bounded
canonical metadata/receipt overflow now has the specific typed field
`import_publication.evidence_size`; only that known invalid-evidence outcome is
converted to per-model unavailability. Other operational failures retain their
errors. Receipt-only malformed gates receive a diagnostic non-confirmed identity
if none existed, so downstream projection cannot reinterpret them as legacy.
Receipt-backed non-object index values are retained under the diagnostic-only
`unparsed_index_metadata` member; the stored row is not rewritten by list/get/
search. All three public query paths are covered, including scalar, array and
null evidence. Unknown object members remain in place.

The new retained-publication refresh uses the same awaited blocking-task pattern
as adjacent library metadata observations. Its original complete row snapshot is
cloned into the closure and remains the CAS precondition. This does not add a new
mutation owner or extend the broader library read/projection lifecycle contract.

A retained empty `pending_moves` checkpoint represents **one frozen incomplete
migration plan**. Later Execute calls preserve its results; they do not retry
blocked publications or discover newly eligible moves. There is no supported
checkpoint reset/replan or Pending reconciliation API in this bounded repair.
Operators must retain the evidence and diagnose the blocked publication; ordinary
retries cannot advertise completion or manufacture a new plan. Independent moves
already in the admitted plan still complete. This limitation is explicit, not a
claim that retained publication recovery has been implemented.
