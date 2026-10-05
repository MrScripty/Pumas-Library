# Empty selected S3 member qualification — 2026-10-05

Tested implementation milestone: `c8eec4224cf9041d1b2e3400271790e34f619b2e`; tree `833db2d9ab5bd5a37b1aa5de641dbb4f04cf1dca`.
The subsequent documentation-only commit and final remote state are recorded in
`/workspace/scratch/s3-empty-members/final-state.json`.

## Scope and frozen lineage

Branch `feat/s3-empty-members-e8082649` starts exactly at frozen
`e8082649a6d4aaebf996c30870d996d43bbee1d3`, tree
`050733c55a3011f23c8832e210d52d0d4d60d695`. Its accepted PR40 normal merge
`d9d907cfe0e57b1e2bc7e296eff1327de835bc5e` remains an ancestor with ordered
parents `339032ff4acb53e43aa36c6edf96739101121620` and
`838eb2990905144a59830f1a16fe91b4e1105d4d`. No frozen ref, main, manifest,
watcher, importer, publication, signing, dependency or generated contract is
changed by this slice. Parent owns PRs, review and integration.

Production acquisition changes only `S3ObjectSelection::open_acquisition`.
A selected immutable VersionId with checked HEAD size zero and resume zero
produces an empty stream rather than an invalid `0..0` range request. The
shared owner still truncates/creates its held partial descriptor, flushes,
hashes and verifies it, publishes the final member, verifies the complete
manifest and issues exact acquisition/consumer receipts. HEAD is existence,
identity and known-length evidence; it does not issue a verified-file receipt.
No GET or byte-range request is necessary for a known empty representation.

Missing/invalid Content-Length cannot become zero: the pinned object_store
0.12.4 header parser requires and parses the header. Missing object/version,
wrong returned VersionId and missing validator remain selection failures.
An incorrect caller SHA-256 fails shared verification and retains custody.
The public `read_range` contract still requires a nonempty range. Unknown
length, a nonempty object, and an empty object remain distinct. Existing
single-file request DTOs, progress, retry/deadline, cancellation, publication
and recovery policies remain unchanged; there is no new public API or schema.
The dialog and RPC/shared contract remove the former empty-member limitation.

## Direct behavior evidence

- The single-member owner fixture starts with untrusted nonempty `.part` bytes:
  valid empty selection replaces them and verifies the empty SHA, while a wrong
  digest or cancellation makes no consumer handoff or final member. Only HEAD
  occurs, and public empty-range reads still refuse without I/O.
- Mixed valid GGUF/empty auxiliary bundles use the same object key with different
  exact VersionIds. The auxiliary receipt records zero bytes and the standard
  empty SHA-256. Both live adoption and interrupted publication cold settlement
  succeed without source/import replay; changed empty auxiliary output retains
  the exact unresolved use and fails cold proof.
- The production HTTPS/RPC child covers fourteen scenarios: existing anonymous
  and authenticated success with/without session token; held HEAD/GET cancel;
  missing member; wrong nonempty digest; anonymous and authenticated empty
  auxiliary success; wrong empty digest; empty primary; mixed empty auxiliary
  plus truncated nonempty primary; cancellation after empty member completion;
  and an all-empty invalid model bundle. Exact request pins and If-Match,
  aggregate zero-byte/count observations, Ready/registered files, receipts,
  retained failures, refused replay, and credential absence from captured logs,
  safe outcomes and owned files are asserted.
- Empty required GGUF weights never become a working model. Existing parser
  validation rejects them; their nested owner failure remains observable during
  drained shutdown. Tests assert that aggregate shutdown failure rather than
  suppress it. No model-format rule is changed.

## Qualification

Logs and hashes are in `/workspace/scratch/s3-empty-members/verification.json`.
The supported environment is Rust 1.92.0, four jobs, debug/incremental disabled,
Linux x86_64, isolated `XDG_CONFIG_HOME`; frontend uses pinned pnpm 10.33.0.
Standards route reuses the inspected snapshot at
`188beda1fa477d21d576c233dd8c7f4c4c267d23` and the established Rust/library,
security, async, contracts, diagnostics, verification and commit owners.
No AGENTS or repository `.agents/skills` files were found.

| Check | Result | Log |
| --- | --- | --- |
| S3 acquisition/native/reader integration | 44 + 6 + 13 pass, plus isolated native signing child | `core-s3-qualified.log` |
| Full S3 RPC suite | 202 + 16 + 2 pass; twelve existing ignored/live cases | `rpc-tests.log` |
| Frontend existing dialog/hook | 14 pass | `frontend-source-tests.log` |
| Caption lint | Pass | `frontend-lint.log` |
| Acquisition unit regression | 171 pass plus isolated child | `core-acquisition.log` |
| Strict core and scoped RPC Clippy | Pass; RPC keeps inherited dead-code allowance | `core-clippy.log`, `rpc-clippy.log` |

Commands from the repository root: `cargo test --manifest-path rust/Cargo.toml
-p pumas-library --no-default-features --features s3 --test s3_acquisition
--test s3_model_workflow --test s3_reader`; `cargo test --manifest-path
rust/Cargo.toml -p pumas-rpc --no-default-features --features s3`; core Clippy
uses `--all-targets -- -D warnings`, RPC uses existing
`--features s3,export-contract,test-support --all-targets -- -D warnings
-A dead_code`. Frontend: pinned pnpm `exec vitest run` on the existing dialog
and hook tests and targeted ESLint. Counts overlap and are not a unique total.

Initial fixture compilation lacked an async ownership move; it was corrected.
The first RPC command incorrectly requested a library target. The added invalid
primary case exposed expected drained owner failure; the final fixture asserts
it. Failure logs are retained, and child panic excerpts were redacted before
saving. No credentials or raw child traces were persisted for diagnosis.

## Separate correction and remaining gates

The parent subsequently identified a pre-existing reserved-destination admission
gap at frozen e808. It is corrected separately on
`fix/s3-bundle-reserved-e8082649`, using the importer-owned namespace rule before
RPC job admission and native source resolution. These are separate successors;
this empty-member branch does not claim that correction is composed into it.
Parent coordinates review and eventual composition.

Synthetic fixtures establish no real-provider or packaged/browser acceptance.
Existing default ONNX CDN failure and fully strict RPC dead-code baseline remain
outside these scoped checks. No account credentials, external reviewer contact,
paid service or security bypass was used. AQ-S3 and Q4 are not advanced.
The next existing-plan work is authorized Q3 cross-provider acceptance (AWS,
one non-AWS service and MinIO), followed by Q4 installed/native qualification;
credential expiry/refresh remains independently scoped.
