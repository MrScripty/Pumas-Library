# Explicit ONNX Runtime provisioning qualification — 2026-10-05

The owner required that Cargo never download ONNX Runtime. This bounded build
correction preserves the in-process execution architecture and the existing
acquisition/runtime owners. It does not start runtime R1, advance AQ gates, add
a downloader, or change application settings into a provisioning mechanism.

## Candidate and isolation

| Identity | Exact value |
| --- | --- |
| Approved main base | `838eb2990905144a59830f1a16fe91b4e1105d4d` |
| Base tree | `f6d6c1d3c5ba5be0fb998a6fd157fcb31c63dc72` |
| Branch | `fix/ort-explicit-runtime-838eb299` |
| Qualified implementation commit | `524391ecec40b9f64432e20ca0f12dfd4c27375d` |
| Qualified implementation tree | `43b2ee46d8272bde323219650757ca7c149f7c3d` |
| Frozen S3 composition head | `493b935c6a41d4d4aeca8e8a66f10b4aba114365` |
| Frozen S3 tree | `ecb51f20c0740fa7d88e6d0eface06c45feb2e07` |

The implementation commit's sole parent is the approved main base. The S3
worktree remains clean at the frozen identity. No watcher, importer, S3 reader,
or acquisition manifest source changed. Parent coordination owns consumer pins,
PRs, review, and merges; no external reviewers were contacted.

## Dependency and public contract

`ort` and `ort-sys` remain at `2.0.0-rc.12`. The workspace disables defaults,
removes `download-binaries`, `copy-dylibs`, and `tls-native`, and selects
`load-dynamic`. Its resolved native binding uses `disable-linking`. Core adds
optional `libloading 0.9.0`, also selected by ORT's dynamic-loading feature.
There are ten removed lockfile packages and no retained package version changes.
The generated 0.7.0 notices/inventory were regenerated with the existing producer.

Core's default `full` still includes `onnx-runtime`; RPC defaults still include
inference. Execution DTOs and public session signatures are unchanged. Metadata,
index, and import APIs remain outside the ONNX feature. Metadata-only consumers
can use `default-features = false` and enable only features their calls require.
Features are additive: parent consumer integration must audit the complete
resolved graph and reject another dependency enabling ORT downloading.

Native SDK provisioning precedes application execution. The execution owner
selects an absolute `ORT_DYLIB_PATH`, or the exact platform-named library beside
the executable. Invalid explicit paths do not fall back to the packaged SDK.
There is no current-directory/system-basename discovery or network fallback.
`ORT_LIB_PATH`/`ORT_PREFER_DYNAMIC_LINK` no longer select this dynamic runtime.
The host owns ORT environment settings and selects the same process-wide SDK
before direct ORT calls; changing the SDK requires a process restart.

Missing/invalid SDKs produce `Backend` errors with field `runtime_library`,
provisioning instructions, and an explicit no-download diagnostic. The pinned
ORT implementation can recursively initialize its native error machinery on
loader failure. Core therefore performs a narrow preflight through maintained
`libloading` and `ort-sys` ABI types before ORT adopts the library. It retains the
loader handle while checking the entry point, API base, version, and C API 24.
Unsafe operations are isolated to this private module with adjacent ABI/lifetime
proofs. A separately verified host SDK is trusted executable dependency code;
this is not a validator for arbitrary hostile libraries.

The [development contract](../../../DEVELOPMENT.md) documents explicit SDK
verification and per-invocation loader configuration. The existing acquisition
owner handles any separately authorized setup transfer. The existing release
stager copies provisioned native files; it does not fetch them. Release operators
must record official archive/library hashes, target/version and notices, stage
the full SDK closure, and prove real inference in the extracted package.

Primary upstream anchors: the pinned
[ort-sys build implementation](https://github.com/pykeio/ort/blob/079ecb47034ec8188e3a06fc04f49ec28a6499e8/ort-sys/build/main.rs)
and Microsoft's [ONNX Runtime 1.24.2 release](https://github.com/microsoft/onnxruntime/releases/tag/v1.24.2).
The qualification SDK pin is 1.24.2/C API 24; no SDK was downloaded by this correction.

## Verification and limits

Local evidence is in `/workspace/scratch/ort-no-download/`; `verification.json`,
`dependency-delta.json`, `feature-inventory.json`, and `log-inventory.json` retain
commands, resolved features, source identity, trace results and log hashes.

| Check | Result and scope |
| --- | --- |
| Feature matrix | 21 graphs passed across Linux x64, Windows x64 and macOS arm64; includes workspace defaults/all features/dev/build/bindings and explicit ONNX. Resolution evidence, not cross-platform execution. |
| Feature guard / release contract tests | 16 Node tests passed, including rejection of download feature unification and unsupported linking mode. |
| Cargo build without suppression | `cargo build --locked -p pumas-library` passed with a separate SDK-free target/cache; no `--offline` or ORT download-suppression variables. Rust crate sources had already been fetched normally. |
| Cargo ONNX tests without suppression | `cargo test --locked -p pumas-library onnx_runtime::` passed: 31 tests plus one subprocess marker ignored in the parent inventory. |
| Network trace | Both normal Cargo build and ONNX test traces exited zero with zero `AF_INET`/`AF_INET6` lines; SDK caches remained absent/empty. Resolved fingerprints have `load-dynamic`/`disable-linking` and no download capability. |
| Native failure regression | Six fresh-process cases passed within a 30-second child deadline: missing file, invalid binary, missing entry point, null API base, incompatible version, unsupported C API. Four synthetic Linux C fixtures use the installed compiler. |
| Default workspace tests | Passed: core 1767, app-manager 288, RPC 294 unit + 20 integration + 2 intent, UniFFI 17; remaining core integrations/doctests passed. Existing ignored tests and marker children are recorded in the log. Rustler host execution is outside this scope. |
| Core without defaults | 1740 unit tests and integrations/doctests passed without ONNX enabled. |
| SDK-free default RPC | Default build and health smoke passed with inference enabled; this proves startup/health, not actual inference. |
| Lint/format | Core all-target/all-feature Clippy with `-D warnings` passed. RPC equivalent passed with the existing scoped `-A dead_code` exception. Cargo formatting and whitespace checks passed. |
| Attribution | Producer emitted 365 notices; checker confirmed current inputs and pinned texts. This does not qualify a native SDK release closure. |

Earlier failure logs are preserved as diagnosis: a loader error hung inside the
pinned ORT initialization before the preflight correction. Final logs have the
`final` names above and are distinguished in the receipt; those historical
failures are not current qualification results. Optional real-model tests lack
their fixtures locally and return early, so their harness success is not an
inference claim.

The owner's prior separately verified official 1.24.2 installed-library success
remains valid at its earlier scope. Its SDK/archive evidence is not available in
this worktree and was not requalified against this implementation. Positive
native loading/inference on this new head needs that verified SDK, the model
fixtures and their licenses/hashes. Windows/macOS execution and extracted full
desktop package inference remain separate acceptance work.

## Existing S3 qualification and next inputs

The independent
[frozen S3 qualification record](https://github.com/MrScripty/Pumas-Library/blob/493b935c6a41d4d4aeca8e8a66f10b4aba114365/docs/plans/artifact-acquisition/reports/s3-composed-provider-qualification-2026-10-05.md)
retains the authenticated/anonymous fixture, signature/token, native-owner and
Linux installed RPC evidence. This branch neither replaces nor broadens it.
Real-provider and full shipping-platform acceptance remain pending:

- MinIO needs an approved official module/dependency route or a preinstalled,
  versioned official service; the previous official download route returned 403.
  No alternate mirror or network bypass was used.
- AWS and a named non-AWS provider need an authorized test endpoint/region,
  addressing/versioning facts, exact VersionIds and trusted hashes, and a scoped
  test-owner credential/token procedure. No real credentials were obtained or
  provisioned in this task.
- Actual browser/Electron flow needs a supported sandbox environment; the prior
  sandbox-helper ownership failure was not bypassed. Platform shipping claims
  need their actual hosts/packages and model/runtime closures.

The next existing acquisition-plan feature is completion of **Q3 real-provider
acceptance**, including credential-expiry/provider behavior, then **Q4 installed
and shipping-platform qualification**. Runtime **R1** remains gated by its
target-scoped AQ-HTTP acceptance. This correction does not advance those gates.
