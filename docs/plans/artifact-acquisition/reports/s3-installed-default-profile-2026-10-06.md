# Installed default-plus-S3 qualification — frozen local milestone

The actual Linux x86_64 production backend passes all ten owned installed HTTPS
scenarios and the default-profile health smoke. This supports bounded AC17/Q4
backend evidence. AQ-S3, AQ-COMPLETE and full Q4 remain not ready.

## Exact source and artifact identity

- Branch: `feat/s3-installed-prefix-main95`, committed and nonforce pushed.
- Base: `0901f6591249ccf955c10f29666e6380e898d201`, tree
  `422a71f51ab8403ac2f24ff1fc90b25d2a3b71e1`.
- Initial production source: `4f5a2a7415193873e81e3737e1a41cfe3b5c18f3`, tree
  `12bf1bd1bb465dde4604a91f8dd3d8dd85ba0634`; build duration 19m25s.
- Final qualification source: `b0a66ba36773c2e4f73e5d1d982759d2f827eea0`, tree
  `5250f10681e62e6f66c478f946b4e149ba10bebd`.
- Binary SHA-256:
  `69df92ec6ece85cfc257e841cfd33f76d624e57a634fa7a319e8bd7a7e00ef57`.
- Final installed archive SHA-256:
  `fdc13ae46eb6993368200dfeec30dbc65a5a9e32dc7cb2f9957c77ff99fd31bb`.

Final source changes only the fixture oracle and its control/documentation after
the initial production build. The builder reran its fixed command against that
clean source (0.25s cached build), observed the same production artifacts and
emitted a new source-bound record. Subsequent qualification documentation changes
no Rust, harness or builder bytes. Complete build records, installed result and
external evidence hashes are committed in
[machine-readable evidence](s3-installed-default-profile-evidence-2026-10-06.json).

## Admitted scope and public behavior

The historical installed harness attributed arbitrary supplied binaries to
checkout HEAD, hardcoded a headless profile and selected default plus old reader
notices. The admitted successor changes only the existing harness, one provenance
helper and focused controls, the existing Linux release-contract test hook, and
owning qualification/plan/ledger documentation. Rust, dependencies, native owners,
ONNX behavior, S3 public APIs and acquisition policies are unchanged.

The new `scripts/release/s3_build_provenance.py --output <external-record>` owns
the fixed locked/offline release command with defaults and explicit `s3`. It
requires clean committed source before/after, native Linux x86_64, unchanged
release settings and no active Cargo config or Rust/profile override. Actual
Cargo compiler-artifact events must report RPC features
`default,inference-plugins,s3`, core features
`gpu-monitor,hf-client,onnx-runtime,process-manager,s3`, release optimization and
no `test-support`. Target comes from the actual Rust host/build target, never an
opaque fingerprint hash. Existing 33-graph checks retain dynamic ONNX and
single-SDK contracts. No ONNX runtime is obtained.

The installed harness now requires `--provenance <record>` and
`--source-head <exact-built-head>` in addition to `--binary` and `--output`.
Before execution it rechecks source/tree, binary SHA, production profile, actual
feature observations and authoritative full S3 notice bindings. The allowlist
contains the unchanged binary, project license, full 415-entry S3 notice text,
attribution README/inventory and qualification JSON. Copied bytes must match the
build bindings; exact member sets and hashes are rechecked after extraction.
No model fixture, credentials or runtime state enter the archive. Records are
unsigned local evidence, without a claim of caller-authored JSON authenticity.

## Actual execution and fixture repair

The first actual run passed anonymous single/bundle cases, then the old fixture
HMAC oracle refused authenticated signing. It hashed query pairs in wire order.
SigV4 and the existing independently checked Rust fixture sort encoded pairs;
the current SDK can emit them in a different order. The narrow correction sorts
the fixture's already encoded pairs without relaxing method/path/query, signed
header, token, pin or wrong-secret checks. A fixed synthetic HMAC vector fails
on the old reversed-order URI and passes repaired; wrong secret, token and
VersionId still fail. An external diagnostic first passed all ten scenarios;
final qualification reran the committed correction with fresh provenance.
Original failure/archive, red control and diagnostics remain retained.

Final installed results at b0a66ba3:

| Scenario | Result |
| --- | --- |
| Anonymous single object and bundle | Both passed |
| Explicit access/secret without and with session token | Both passed |
| Wrong digest, missing object and unknown length | All retained failure custody; no publication |
| Truncated body | Passed existing bounded range resumption and failure behavior |
| Cancellation during held body | Passed source closure, retained custody and drained exit |
| Empty primary | Passed refusal and expected drain failure; no publication |

All signatures are independently recomputed by the fixture, including a negative
wrong-secret oracle. Synthetic ambient credentials remain unselected. Successful
receipts retain exact demand, key/VersionId pins, verified digests and empty
auxiliary identity. Cold owners find indexed models without transfer replay or
receipt replacement. Invalid reserved paths perform no source I/O. The harness
scans RPC output, bounded backend diagnostics and all disposable-root files for
synthetic credentials. Every owned child/listener drains. A separate integrity
check also scans all retained evidence and uncompressed archive members.

Default-profile `/health` smoke passes with `inference_disabled=false`. This
confirms backend startup/routes/drain; no inference executes and no runtime or
real model asset is obtained.

## Checks and preserved evidence

All 16 provenance/oracle controls pass. Pinned Ruff0.15.2 lint and format pass for
the entire existing CI Python scope (`torch-server scripts/release`, 56 files).
Workflow YAML parses and the controls are wired into its existing Linux contract
gate. S3 attribution validates 415 entries; the builder runs the existing 33
default/headless/S3 feature graphs. `git diff --check` passes and the Q4 comparison
has no Rust changes. An extra whole-repository lint invocation found an unchanged
unused `sys` import in `scripts/acceptance/flux2_v214_rpc_acceptance.py`, outside
the CI scope and admitted write set; its failure log is preserved.

External evidence root: `/workspace/scratch/s3-installed-default-profile/`.
Primary files: `production-build-query-fixed.json`, its `.cargo.jsonl` and
`.cargo.stderr.log`, `installed-final.log`, `installed-final/installed-result.json`,
`installed-final/pumas-s3-qualification-linux.tar.gz`,
`default-profile-smoke.log`, `query-oracle-red.log`, `query-oracle-green.log`
and `query-oracle-ruff-ci-{lint,format}.log`. Initial build/failure and diagnostic
artifacts remain alongside final evidence. The checked-in JSON records hashes.

## Remaining gates and next existing-plan slice

Actual AWS/non-AWS/MinIO acceptance, supported desktop/browser, Windows/macOS,
native/runtime/model workflows, independent clients and old-writer deployment
facts remain unqualified. Provider fixtures/authorization and supported hosts
are not supplied. No real account credential is obtained, sent or provisioned;
no network/security bypass or paid service is used. Parent owns PR publication,
external review, hosted CI and integration; none is performed here.

The next feasible existing AC05/AC17 slice is one installed Linux S3 process-loss
boundary before `FilesReady`: hold a synthetic authenticated HTTPS transfer after
one byte, observe exact durable selection and partial custody, SIGKILL/reap the
owned backend, then reopen without source replay or model/receipt publication.
Cold task observations are ephemeral; the oracle must inspect canonical custody,
not infer completion from a lost task status or saved counter. Read-only design
is `/workspace/scratch/s3-installed-process-loss-design/DESIGN.md`; execution and
a separate admitted write set remain future work. This would not establish
power-loss durability, actual-provider acceptance or full Q4.
