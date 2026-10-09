# Sanitized HF download-status diagnostics — 2026-10-09

This bounded change preserves the existing public `get_model_download_status` response and error contract while distinguishing failure stages in private structured logs. It is based on commit `23f783b4d7d06965d623a6b809b1c9ec69bdb059` (tree `9c8872353d41b8f02d156dac4ccb5dc40458d9ff`). It does not diagnose or repair the historical HF `-32603` observation.

## Behavior and privacy

The production getter feeds a shared private projection helper. Failed state lookups emit `state_lookup` / `domain_lookup`. The existing DTO validation path emits `outcome_validation` with a closed `library_model_id` or `numeric_evidence` category. Validation predicate order, original domain errors, DTO fields, missing-download behavior and public error envelopes remain unchanged. There is no category inferred by parsing domain messages.

An INFO-level async request span covers dispatch, projection, serialization and final response construction. It adds a process-local `rpc_call_id`, an allowlisted method label and the existing numeric-only safe request ID. String request IDs still echo on the wire but are omitted from logs. Parameters, download/repository/model identifiers, raw domain errors, raw progress numeric evidence, paths, credentials and URLs are not added to diagnostic events. Serialization failures have a static `serialization` / `outcome_serialization` label. Successful status calls add no INFO event with the default subscriber configuration.

Correlation requires the consumer to retain these logs. The process-local counter is neither durable nor a global identity, and does not recover missing historical evidence. Serialization failure labeling was source-reviewed; no synthetic serializer-failure runtime witness is claimed.

Dispatch is heap-pinned at the request boundary to avoid increasing the nested async future's stack footprint. This adds one bounded allocation per admitted RPC. An initial unboxed implementation aborted the unit suite with a stack overflow; the boxed implementation passes that exact regression without changing thread stack limits.

## Validation

All Rust commands use locked, offline official dependencies, `ORT_SKIP_DOWNLOAD=1`, one build job and the existing shared target directory. The diagnostic fixtures are synthetic projection/transport tests: they exercise the same projection helper and response construction, not an HF transfer or a historical snapshot.

- Final RPC binary suite with `--no-default-features --features s3,export-contract,test-support`: 248 passed, 0 failed, 2 existing ignored; 92.16 seconds test runtime. Log SHA256 `53971d99a16b5ab510810ab72570e7c29f83d5f39e9e1c753bb63e7d4e38eb8a`.
- RPC-only Clippy (`--no-deps`, binary and tests, `-D warnings -A dead_code`): passed. Log SHA256 `bee41ca6d69824a800e7de6689da2b150e5311f33065e17909157a8e5312864e`. The dead-code allowance covers inherited no-inference contract fields.
- All-feature production compile check (`cargo check --locked --offline -p pumas-rpc --all-features --bin pumas-rpc`): passed. Log SHA256 `485db2640383513f7e230a4588f1821e9eecdffd4b68ef34a1497097882b7c6c`. This compiles gated routes; it executes no inference.
- Formatting (`cargo fmt --all -- --check`) and whitespace (`git diff --check`) checks: passed.

The fixtures cover state-lookup, invalid-library-ID and nonfinite-numeric failures; exact original static public errors and string-ID wire echo; omitted private sentinels; unchanged valid/missing responses; and concurrent calls that yield between stage and final events while retaining distinct request correlation. The previously failing backend-setup admission case also passed separately after boxing.

Dependency-inclusive Clippy is blocked by four existing lints in unchanged core files: `nonminimal_bool` in `model_library/library/projection.rs:164`, and `type_complexity` in `api/reconciliation.rs:547,556,732`. No unrelated core fix or repository-wide Clippy pass is claimed. Earlier failed compilation, stack-overflow and target-only Clippy iterations are retained as failed iterations in local evidence, not qualifying runs.

## Boundaries

This is source and Linux unit/compile qualification. It does not qualify a newly released executable, real model inference, a provider campaign or a native consumer against this new source. The earlier pinned Chrema consumer qualification and its binary remain separate and unchanged. No Cargo ONNX download, untrusted ONNX load, model inference, S3 retry, credentials, global Python, physical-store recovery, main merge, release, tag or version bump is included.
