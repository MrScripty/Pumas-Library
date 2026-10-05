# Explicit pinned S3 file sets and GGUF auxiliary composition

Branch `feat/acquisition-s3-manifest-ea1c4ae7`; exact parent
`ea1c4ae75a8a6d4719a95f31c684e1d0398acbb8`, tree
`736424cfd86f4c3d24ce268718ebec21e5e9613e`. Frozen dispatch e1ee893f,
accepted reader `2c7d6014658ef44575f3724f0ff6e7395f1fd7d8` and accepted main
`96c2dca97fad673a2735f9f778013067569fe692` remain ancestors. Parent owns
independent review, PR actions and integration. This advances a bounded explicit
file-set consumer result; AQ-S3 and complete acquisition acceptance remain open.

## Authority and covered requirements

Each caller-declared entry supplies its exact source key, immutable VersionId,
logical path and expected SHA256 under one explicit reader endpoint/bucket/addressing
authority. The resolver sorts logical paths, validates all object identities and
the complete shared-manifest namespace before any HEAD, then resolves every entry.
Missing or changed declared versions return an error, never a shortened selection.
The group immutable revision serializes the original per-object source identities
in canonical order, preserving exact endpoint, addressing, bucket, key and VersionId.
The existing 16-KiB revision limit bounds this encoding before network calls.
This is an explicit set and makes no atomic remote-prefix snapshot claim. HEAD
operation and transfer retry budgets remain per object, not one aggregate deadline.

The opaque selection feeds all objects to the existing shared transfer, checkpoint,
verification, receipt, worker and settlement owner. No listing, new store/schema,
retry owner or protocol parser is introduced. Every declared member must verify
before consumer preparation. A later member's version/digest failure retains the
existing transfer state and already verified input, without model publication.

The bounded model consumer accepts one primary GGUF named by the exact bound
serialized import spec and digest-verified data/text auxiliaries with extensions
json, txt, md, model, tiktoken, vocab or merges. It refuses other/sharded weights,
executable auxiliaries and root metadata/receipt reservations. Extension admission
is a copy policy, not semantic validation of tokenizer/config contents. All held
verified descriptors enter one registered copied-import producer; the primary
can occur anywhere in canonical order. Auxiliary logical paths are preserved,
parent directories are created by existing held stage authority, each copy's
size/SHA256 is checked, and the existing complete-payload publisher confirms the
model/index before the acquisition owner acknowledges it. No ambient source path
is reopened. Ordinary directory and single-file copy behavior is preserved.

Cold complete-bundle reconciliation requires the exact issued/current acquisition
receipt, confirmed output receipt, canonical Ready metadata/index, physical held
payload proof and every exact logical path, size and digest. Changed or omitted
members retain uncertainty. The existing consumer alone settles the exact use;
observation performs no transfer, import, index repair, output deletion or Pending
promotion. Existing output receipt version 2 already binds the full acquired set;
ordinary copied output retains readable version 1. No format version/migration,
Cargo dependency/lock, attribution generator, HF or speech source change occurs.

## Local evidence and limits

Final source passes 29 S3 acquisition/import cases (20 prior plus nine new),
11 reader fixtures and five HTTP cases: 45 focused runtime tests. New controls
cover canonical exact pins, whole-set preflight with zero IO, missing member
resolution, full nested-auxiliary Ready publication, confirmed cold Using-to-Adopted
settlement without requests, same-size auxiliary mutation, omitted member intent,
last-member version/digest failures before handoff, and unsupported/reserved input
refusal. Reopened owners use a live loopback counter; successful recovery observes
no requests beyond the initial two HEADs and two GETs. Negative output assertions
compare post-startup baselines, retaining existing unavailable-index projections.
Synthetic input is a 24-byte GGUF and two-byte auxiliary file. Real shared owners,
model publisher/index and independent read-only Ready selector are exercised.

Strict `s3,test-support` all-target Clippy with warnings denied, actual headless
compile, 12 feature contracts, canonical attribution/dependency ownership,
formatting and 20 release/workflow tests passed. Locked/offline Linux x86_64 checks
used Rust 1.92, one build job, task-owned XDG config and the existing shared target
with root-package debug/incremental/strip overrides. Logs, exact command arrays,
source identities and task-only cache-retirement SHA256 journals accompany handoff.
The first integration compile failed Rust's nested module lookup (E0583); an explicit
path attribute corrected it before runtime. A static-command recorder initially
omitted the environment setup and could not locate Cargo; it was rerun with
the established toolchain. No failed runtime case was hidden.

No complete unit/default ONNX suite is executed. The 32-GiB environment has about
135 MiB free after focused checks; existing ort-sys build-script artifacts have no
completed native output. No large native acquisition was attempted, unrelated
cache removed, or permission/network/credential change made. Feature graph checks
cover default declarations but do not constitute default runtime qualification.

Fresh same-process owner reopening models the confirmed-publication/pre-acknowledgement
window; it is not hard SIGKILL, power loss or the full interruption/cancellation
matrix. No deployed migration/old-writer isolation, native cross-platform run,
real weights/inference, desktop/RPC source workflow, authenticated/refreshing
credentials, live AWS/non-AWS/MinIO, atomic prefix snapshot, sharded GGUF or other
multi-file weight format is qualified. Independent review and exact-head hosted
execution remain pending. AQ-S3 remains not ready. The next separately useful
source requirement is the source-facing application workflow; provider/credential
qualification needs its explicit authority and resources.
