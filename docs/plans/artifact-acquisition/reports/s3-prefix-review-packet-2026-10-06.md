# Parent draft PR packet: bounded S3 prefix enumeration

Status: saved source checkpoint; final qualification pending. Parent must
coordinate before any PR mutation. No external reviewer has been contacted.

Suggested title: Add bounded explicit S3 prefix enumeration

Base: `7cf17383001d582056ac4299803c83d0f9ae68a7`, tree
`046771b37c9a9f563936376539301b3225b48656`.
Branch: `feat/s3-prefix-main95`. Attribution prerequisite:
`a39121b4cc7667f901959497f71368179341cfd1`, tree
`35e24f3265b6064d16516493378e89f695609a4b`.
Source checkpoint: `253cc3d17aeae04f4df996133e0f7f4745e03a71`, tree
`1bf9e7614f8cfb9850f62399a0cd8f839db500d3`, committed and nonforce pushed.
Review comparison: `7cf17383001d582056ac4299803c83d0f9ae68a7..253cc3d17aeae04f4df996133e0f7f4745e03a71`.
Subsequent evidence-only commits do not change that source tree's Rust bytes.

## Proposed description

Callers can explicitly enumerate a raw S3 prefix with positive page, object and
XML budgets. Complete pagination is checked against the maintained SDK and the
reviewed XML agreement guard; every listed object then requires a matching
conditional HEAD with immutable VersionId. Errors and exhausted budgets return
no successful partial selection. Listing is discovery, not an atomic package.
Caller-selected paths and trusted digests still enter the existing manifest and
verified acquisition/import owner.

The additive API exports `S3PrefixLimits`, `S3PrefixError`, `S3PrefixListing`,
`S3PrefixObject` and `S3Reader::enumerate_prefix`. Config, existing constructors
and `S3ReaderError` remain compatible. One SDK owner preserves explicit in-memory
credentials, endpoint/TLS/proxy/redirect constraints, diagnostic redaction,
reader timeout and unchanged transfer/receipt identity policy. No ambient
credentials, hidden transport owner, arbitrary timeout or runtime download.

The same branch includes explicit S3 release attribution and 33 dependency graph
checks preserving the approved dynamic ONNX/no-build-download contract. Default
attribution remains 365 entries; S3 attribution is 415 with optional roxmltree.
Frozen manifest/native watcher/importer/recovery/ONNX sources are unchanged.

## Validation and open gates

Focused prefix checks pass 28 S3 units and 66 S3 integrations, including signing,
session tokens, anonymous compatibility, pagination refusal, cancellation,
TRACE redaction, prefix-to-import and exact cold receipts. Node attribution and
release-contract tests pass 19; graph/ownership/format/attribution checks pass.
The final shared test-fixture cleanup passes strict core Clippy with
`--all-targets -- -D warnings` and the 28-test focused unit rerun.
Production RPC compilation and 302 RPC tests passed before prefix implementation;
post-prefix production RPC Clippy passes with the inherited dead-code
allowance; post-prefix RPC tests remain pending. Hosted CI, actual provider acceptance,
platform/runtime/inference and desktop/accessibility are separate pending gates.
No provider account credentials have been used or provisioned.

Evidence: `/workspace/scratch/s3-prefix-main95/`; intermediate failures are
retained. See [qualification](s3-prefix-enumeration-2026-10-06.md) for scope,
check logs and limitations. Next planned work: Q3 actual-provider acceptance,
then Q4; do not silently close either gate.
