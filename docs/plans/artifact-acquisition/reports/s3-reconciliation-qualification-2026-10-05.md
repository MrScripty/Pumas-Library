# Confirmed S3 model publication reconciliation

**Review correction:** Independent review found acquired recovery could accept missing/null publication identities through legacy readiness helpers, skipping held payload proof. The earlier focused checks did not cover this defect. See [bounded proof repair](s3-publication-proof-repair-2026-10-05.md); this report's output-proof claim is superseded for that gap.

Branch: `feat/acquisition-s3-reconcile-e1ee893f`. Parent candidate:
`e1ee893ff67752bdc839a067f15519f25a323c1a`, tree
`b449f2c88d3e34a9cfa16dd14c778d6b802c8495`. Accepted reader
`2c7d6014658ef44575f3724f0ff6e7395f1fd7d8` and accepted main
`96c2dca97fad673a2735f9f778013067569fe692` remain ancestors. The prior reviewed
branch and reader PR38 are preserved. Coordinator owns PR/review/integration.

## Selected acceptance and authority

Contract section 9 requires exact consumer-generation reconciliation when
domain commit precedes acquisition acknowledgement. AC05 includes no repeated
side effects and retained uncertainty across reopen. This successor implements
that bounded window for the anonymous, single-GGUF S3 path through the existing
copied importer. It does not advance the complete AC05 or AQ-S3 gate.

The existing publication authority persists the exact issued consumer receipt
inside its model output receipt before publication. Source, manifest, demand,
workspace, lease, verified files and serialized import intent are bound there.
The acquisition service remains authoritative for its own current receipt and
use generation. Recovery passes through `AcquisitionConsumer::reconcile`;
`ModelImporter::reconcile_acquired_gguf` only observes candidate model output.
No parallel acquisition store, retry owner, installer or recovery writer is added.

The output proof requires a primary canonical model record, an acknowledged
Ready index with the same publication identity, exact model ID and acquisition
binding, a Confirmed publication receipt, held physical root/payload identity,
complete payload hashing, and the single verified input size/digest. The model
ID is a lookup candidate, not a path or authority receipt. Held model-root
authority and acquisition custody survive the registered blocking observation.
Completed read-only refusal is a nested observation result, not an unresolved
producer effect; join failures still belong to the task owner. Only the existing
consumer settles the proven exact use. No network, copying, index repair,
Pending promotion, output deletion or workspace reclamation occurs here.

## Persisted contract disposition

Ordinary copied imports still emit receipt version 1, with unchanged serialization
and no acquisition field. That retained format remains readable. Acquired copies
emit version 2 and require an acquisition binding. The decoder explicitly accepts
`1 + no binding` or `2 + binding`, and rejects other version/binding combinations.
The existing model-metadata publication identity remains version 1; this is a
different representation from the physical output receipt.

Old version-1 outputs created by the dispatch-only candidate are not silently
upgraded or treated as proof of an interrupted acquisition. Their model readiness
remains supported; their unbound acquisition settlement remains unresolved.
No live-root migration or rollback claim is made. An older reader rejects new
version-2 receipts; coordinator must own any deployed consumer cutover. The
canonical acquisition schema and all dependency/lock/license inputs are unchanged.

## Evidence and honest limits

Final source local qualification passed: 20 S3 dispatch/import/reconciliation
integration cases, 11 reader fixtures and five HTTP integration cases (36 total),
strict enabled-S3 all-target Clippy, actual headless compile, 12 feature contracts,
canonical attribution/dependency ownership, formatting and 20 release/workflow
contract tests. These are Linux x86_64 locked/offline checks with Rust 1.92 and
existing shared targets. Task-local XDG configuration, one build job and
command-local root-package debug/incremental/strip overrides fit constrained disk.
No complete unit/default ONNX suite or hosted execution is claimed. Runtime logs,
command arrays, source hashes and cache-retirement hashes accompany the handoff.
The ordinary copied-import control observes a version-1 output with no acquisition
field and Ready canonical metadata. The confirmed cold control settles Using to
Adopted, repeats output proof, preserves original input/output bytes, and observes
Ready through the independent read-only selector. All seven negative controls
retain their post-startup model/index/store/workspace evidence and observe only
the initial two source requests through shutdown.

The first check exposed a private helper
visibility and unnecessary Eq derive; both were corrected without widening public
types. The first integration run had 15 passes and five failures. A read-only
changed-output refusal was counted as a failed blocking effect by the original
observation wrapper. Several negative fixtures also tried to treat restored
fixture bytes as authority to repair a cold-start unavailable index. The observer
now separates observation refusal from effect failure; negative fixtures retain
their actual unresolved state instead of restoring it or expecting index repair.

Fixtures use a loopback S3-compatible HTTP/1.1 source and a synthetic 24-byte GGUF,
the real shared service/copied publisher, fresh same-process owner/root reopening,
primary files, the actual index and an independent read-only library selector.
The protocol listener remains alive during recovery to count any request replay.
Negative fixtures mutate only disposable evidence after completed model publication;
the Pending case tests refusal of Pending-shaped output, not a process killed
at the producer's Pending boundary. Startup may apply its existing unavailable
index projection; reconciliation itself is compared with a post-startup baseline.

No hard process/power loss, complete interruption matrix, pending-output
finalization, old-writer isolation, deployed-root migration, native cross-platform
build, full unit/default ONNX suite, live AWS/non-AWS/MinIO, credentials,
multi-file import, desktop/RPC source workflow, packaged consumer, real weights
or inference is qualified. Exact-head hosted execution and independent review
remain pending. AQ-S3 remains not ready.

The next required source capability is complete explicit multi-file selection
and consumer composition, plus a source-facing application workflow. Provider
and credential qualification remains separately authorized.
