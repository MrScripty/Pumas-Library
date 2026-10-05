# Consumer receipt byte-budget PR39 review correction

PR39 review 5409255854 selected all 18 files and identified a valid new-publication
bound defect at discussion_r4180140732. The S3 group-pin encoding limits source
identities but not arbitrary generic manifest lists, callback payloads, or the
sum of unique directory prefixes in a payload proof. Separate branch
`fix/acquisition-output-receipt-bound-159fd371` starts at exact
`159fd371d7669e53959e874ffc7f989a66f999b0` (tree
`4a9a0d515538418056a24ca85c19edf9fa143bfd`) and preserves accepted main
`73fec2e06c75ddae8c0e58be6ffcf352923376bd` (tree
`b449f2c88d3e34a9cfa16dd14c778d6b802c8495`). Parent owns PR39 review and integration.

## Admission and issuance contract

The sole production write is `acquisition/service.rs`. New HTTP/S3 facade
admission counts actual `serde_json::to_writer_pretty` manifest bytes without
allocating a serialized buffer, capped at 4 MiB. It separately reserves 2 MiB
for every file and each unique parent prefix, charging its actual JSON-encoded
name plus 512 bytes for physical evidence and formatting. Deep paths cannot
hide quadratic prefix expansion behind the S3 pin bound. Refusal occurs before
schema/store admission and payload transfer; prior S3 selection HEAD metadata
resolution is a separate existing step.

The actual completed `AcquisitionConsumerReceipt`, including demand, workspace,
verified files and callback data, must fit 4 MiB immediately before its new
store issuance. Refusal retains verified inputs and an unreceipted Using use,
and never invokes publication. These checks run within the existing owned
blocking scope, retain capabilities, use checked accounting, and return normal
typed validation results without turning input refusal into a shutdown failure.

This is a conservative new-operation support budget. Existing manifest
construction/decoding, schema-7 persistence, receipt validation, retained
settlement/reconciliation, model publication formats and late defensive 16 MiB
checks remain unchanged. No new abstraction, dependency, generator, lifecycle
owner, fallback, source reselection or cross-layer acquisition dependency is added.

## Actual output schema reserve

Existing real two-file bundle publication/cold-proof fixtures now inspect their
actual generated version-2 output receipt. The oracle widens both physical u64
identity fields and file size to u64::MAX, SHA to 64 characters, publication ID
to 36 characters, model ID to its held-path 4096-byte bound, and original stage
to its current 48-character basename. It measures the real fixed envelope and
individual directory/file map entries; new structural fields or reserve overflow
fail the oracle. It also measures nested binding indentation.

Embedding pretty JSON adds at most two spaces per existing newline, bounded
by twice the 4 MiB binding. Current proof entries fit the encoded-name-plus-512
reserve; doubling the 2 MiB namespace allowance is conservative. A 16 KiB fixed
envelope reserve covers the current bounded root/stage/ID/state fields:
`2 × (4 MiB + 2 MiB) + 16 KiB < 16 MiB`. Producer digest verification, complete
file-set proof, Pending/Confirmed transitions and Ready index ordering remain
the existing authorities. The late bound remains defense for retained/ordinary
publication and schema evolution; it is not removed or treated as early admission.

## Red baseline and final evidence

The two red regression bodies are byte-identical in final qualification:

- A valid generic manifest serialized to 17,073,387 bytes, round-tripped through
  the unchanged decoder, and reached a source GET on original 159fd371 rather
  than refusing admission.
- A real pinned S3 acquisition issued a 17,827,288-byte callback binding and
  reached the publication callback. The fixture deliberately observed issuance
  without invoking model side effects.

Final controls refuse both with `acquisition.consumer_document_size`. A valid
3812-byte deeply nested path refuses with `acquisition.consumer_namespace_size`
before source/store/prepare effects. Callback refusal preserves exact verified
bytes, Using record, absent completion receipt and empty model index. Added
missing/null publication identity with intact payload keeps cold uncertainty;
existing changed/missing/legacy and bundle controls remain active.

Final runtime: 36 acquisition/import + 11 S3 reader + five HTTP tests = 52,
zero failures or ignored tests, Linux x86_64, locked/offline Rust 1.92,
no default inference features, S3 enabled. The low-space build uses the existing
shared target, one Cargo job, package-local debug/incremental settings and a
**test-only** `cargo rustc ... -- -C strip=symbols`, followed by its exact produced
integration executable. Test assertions and production behavior are unchanged
by stripping. Command arrays, executable SHA/size retirement journals and logs
are in the review package.

Supporting qualification: strict enabled-S3/test-support all-target core Clippy,
actual headless compile, 12 feature graph contracts, canonical attribution,
workspace dependency ownership, Rust formatting/diff, and 20 release/workflow
contracts. All 482 locked package identities and unchanged generator bytes are
verified. HF/speech, retained manifest/store, publication/staging and import
custody source bytes match 159fd371. The unchanged generator plus canonical
attribution check is qualification here; no regenerated dependency identities
or license substitution is introduced.

Failed attempts are retained: initial/retry linker SIGBUS at disk exhaustion
ran no tests; test-only symbol stripping allowed the red run. Initial source
compile failed E0425 from insertion in a similarly shaped method, corrected
before runtime. An unsourced feature-check invocation could not locate Cargo;
the sourced final command qualifies the result. Only positively identified
completed/failed task outputs were removed, with size/SHA journals.

## Separate observed AQ-S3 blocker and limits

A temporary real 3 MiB callback/bundle stress extension ran 36 passes/one failure.
During live copied staging, owned background reconciliation attempted
`.tmp_import_…/config/tokenizer_config.json/metadata.json`; shutdown reported
ENOTDIR. Its exact fixture source, patch, failure log and intermediate build
are preserved outside this bounded repair. No production discovery repair or
passing large-payload concurrent-discovery claim is included. The final repair
returns to the frozen real producer fixture and adds only the output-schema
oracle; the red byte-budget assertions are unchanged. This failure is useful
independent gap evidence, not silently retried or recategorized as acceptance.

Full/default ONNX/native execution, full unit aggregate, hosted validation,
independent review of this repair, supported-platform hard crash/power loss,
live AWS/non-AWS/MinIO, credentials, inference and desktop workflows remain
unqualified. Free space is tight and completed ort-sys native outputs are absent;
the install script was not rerun. AQ-S3 remains not ready. No credentials,
permission/network settings, external review requests or merges were changed.
README discussion_r4180140721 and ledger discussion_r4180140718 corrections
preserve exact wording/identities while fixing sentence placement/token spacing.

Manual standards route: core 7c0e07c670243267e8db2a801169d6a0bb09756d and router
3aec08f37c01fe8be2e701ca3abe68bb857f3388; implementation, verification/oracles,
contracts/evolution, persistence, performance, concurrency, code design,
library, Rust API/async/security, security, documentation and commit modules.
This is bounded route evidence, not whole-engine or external-review certification.
