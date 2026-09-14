# M4 — Existing local transport projections

Status: Accepted within the local Linux evidence and explicit compatibility limits below.

## Scope

Project the seven existing native intent operations through authenticated local
IPC and the existing loopback JSON-RPC server. Both delegate to the primary's
intent service. Requests express domain requirements; transport handlers do not
plan downloads, persist declarations, or implement another lifecycle owner.
The server drains the composed intent owner during its existing shutdown.

Wire methods use the `intent_` prefix to preserve operational method names.
The first three accept `requirement`, ensure accepts `request`, release and
ensure-status accept `reference`, and declaration listing accepts an empty object.
IPC additionally carries its existing registry connection token.

## Existing consumer inventory

| Consumer | Existing contract | M4 treatment |
| --- | --- | --- |
| Native `PumasApi` | `get_model(&str)`, `start_hf_download(&DownloadRequest)`, model-library helpers | Retain methods; intent remains a separate borrowed facade |
| Native `PumasLocalClient` | Selector snapshots, artifact-load resolution, batch settings/facts, updates | Add a borrowed intent facade; retain existing methods and authentication |
| Standalone JSON-RPC | `get_models`, `download_model_from_hf`, download status, artifact-load resolution | Add seven closed typed commands/outcomes; retain operational routes |
| UniFFI `bindings/api_models.rs` and `api_hf.rs` | `get_model(String)` and `start_hf_download(FfiDownloadRequest)` | No binding migration or generated DTO change |
| Electron `rpc-method-registry.ts` | Explicit operational allowlist and schemas | Existing allowlist remains; desktop intent adoption is not this slice |
| Frontend `api/models.ts` and `types/api-bridge-models.ts` | Existing model/download bridge methods | Existing UI behavior remains; no new GUI dependency |

No existing consumer needs to migrate to complete the local intent API. Native
examples and local-process integration tests are the new consumers. No node,
fleet, discovery, remote control, new production listener, or gateway feature is
introduced. Real HF process tests use the existing outbound client with a test
CONNECT proxy configured only in the child environment.

## Behavior evidence

- All seven operations use closed typed commands and outcomes. Receiver tests
  reject unknown outer and nested fields while preserving native defaults and
  semantic-invalid domain outcomes. Existing public infrastructure-error
  projection remains in force. Available paths remain intentional handle data.
- A registry-backed child primary exposes authenticated IPC. Tests compare its
  real local Available observation field-for-field with the native observation,
  exercise missing/invalid/unsupported cases, and verify reconnect, forced-exit
  restart, generation-aware release and release durability. No parent-global
  registry environment is changed.
- Actual library-only RPC processes exercise all seven HTTP and IPC facade
  methods, Available/Missing/Invalid/Unsupported results, persisted declarations
  and restart. Unix cleanup uses the existing shutdown request plus SIGINT and
  requires successful process exit after owner drainage.
- Both real acquisition gates passed against public repository
  `ggml-org/test-model-stories260K`, immutable commit
  `479896ec924af6d40fd419ab8f4d1eb2101de00d`, artifact
  `stories260K-f32.gguf` (1,185,376 bytes). Each fresh owner uses a child-only
  transparent CONNECT proxy to throttle the real transfer. After Acquiring,
  consumer reconnection observes the same declaration generation, model pin and
  download ID. Lifting the throttle reaches Available. Release preserves the
  downloaded file, and graceful shutdown exits successfully.
- Existing operational RPC process checks pass after the tracked-download test
  fixture explicitly supplies the current persistence revision field.

The live tests are ignored by default because they require actual HF/CDN access;
this acceptance run invoked both explicitly. Reproduce with:

```sh
cargo test --offline --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features --test intent_integration_tests -- --ignored --test-threads=1
```

`--offline` restricts Cargo dependency resolution; the explicit live test still
uses the public upstream. The default run exercises the two independent local
process tests. The proxy, bounded log tail and process harness exist only in the
integration test file; no test endpoint or networking feature was added to core.

## Verification

| Suite | Passing tests |
| --- | ---: |
| Core reconciliation | 30 |
| Runtime task owner | 11 |
| Existing IPC and new protocol contracts | 37 |
| Native intent including upstream crash parents | 11 |
| Registry-backed IPC process consumers | 2 |
| RPC unit/contract/owner regressions | 159 |
| Existing operational RPC process regressions | 13 |
| Local RPC-process intent consumers | 2 |
| Explicit real HTTP/IPC acquisition consumers | 2 |

All 267 relevant tests passed. Normal and `--no-default-features` RPC builds
passed. Core library/tests/examples and RPC library-only all-target Clippy passed
with `-D warnings`; RPC `export-contract` and UniFFI compile together without
changing generated consumers. Exact Rustfmt, whitespace and relative document
link checks passed.

The core crash and IPC subprocess entrypoints
are ignored as standalone tests and exercised by their parent tests. Ten older
RPC tests that expect an externally running server remain ignored; the 13
process regressions launch their own instances. A pre-existing IPC 25ms timing
assertion briefly exceeded its budget during a concurrent compile and passed
with its unchanged limit when rerun without compilation.

Evidence is bounded to this Linux host, the supported single-file acquisition
contract and the recorded fixtures. It does not establish power-loss guarantees,
all-platform behavior or a full runtime-suite pass; I7 and I8 retain their
separate dispositions. The test harness uses forced termination on non-Unix
hosts and does not claim graceful-shutdown evidence there.


## Integration findings

The real upstream process gate reached availability through both transports but
exposed a composed-shutdown failure: reconciliation racing an active download
returned expected `DownloadRootBusy`, which the finite owner archived as a
failed operation. The narrow reconciliation correction preserves dirty
work and bounded retry for opportunistic contention. Forced refresh still
returns DownloadRootBusy to its caller, carried as an expected owned result so
it does not poison drainage. Nested task failures take precedence over deferral,
and actual owner failures remain observable. Two held-root regressions and both
real upstream process tests verify the correction.

Process fixtures also needed to preserve normalized metadata across restart and
explicitly initialize the current persisted-download revision field. Strict
production decoding and native-versus-IPC availability assertions remain intact.
