# M2: Managed native intent acquisition

Status: accepted for the bounded native acquisition scope on 2026-09-12.

## Behavior

`PumasApi::intent().get_model` now admits an explicit `AllowUpstream`
repository requirement through the existing managed Hugging Face download owner.
It resolves branch/tag selectors to an immutable commit, selects a supported
artifact, and returns a pinned local requirement for subsequent status calls.
Ambiguous upstream candidates expose distinct artifact selectors; callers do not
need to construct operational download requests or coordinate import callbacks.

Query and status remain read-only. `LocalOnly` performs no upstream access.
Progress and download IDs are advisory; the resolved requirement is the stable
correlation identity. Existing download admission, persistence, recovery,
import, and shutdown retain ownership. No desired-state store, transport method,
listener, node, fleet, or peer networking is introduced.

Pinned execution rehydrates the exact admitted filenames from commit-specific
metadata and verifies available LFS SHA-256 and size through bound filesystem
access before promotion/import. Existing final files, completed partial files,
and restored byte-complete state receive the same checks. Unavailable or
contradictory evidence fails closed and preserves failed bytes. Pinned import
must produce canonical package facts before completion becomes observable.

Supported acquisition is bounded to coherent single-file GGUF, ONNX, and bare
Safetensors artifacts with LFS integrity evidence. Known directory, adapter, and
sharded layouts are unsupported. Safetensors with configuration indicating an
HF directory package is rejected before transfer while I8 remains unresolved.
Availability is an observation, not a lease or inference-execution guarantee.
Auxiliary files without hash evidence are not represented as cryptographically
verified.

## External-path evidence

The native `intent_acquire` example successfully acquired
[`ggml-org/test-model-stories260K`](https://huggingface.co/ggml-org/test-model-stories260K)
using a fresh temporary library and registry on 2026-09-12. It returned an
available local load path and immutable revision:

- Commit: `479896ec924af6d40fd419ab8f4d1eb2101de00d`.
- Artifact: `stories260K-f32.gguf`.
- Size: 1,185,376 bytes.
- SHA-256: `270cba1bd5109f42d03350f60406024560464db173c0e387d91f0426d3bd256d`.

An independent local hash and size check matched the pinned upstream tree.
The indexed library contained both canonical facts records (summary and detail).
The final rebuilt example also passed with explicit `main`; a second process
reopened the same isolated library and returned the same pinned available handle.
The example awaited managed download shutdown. No model runtime was executed.
This verifies one Linux/native external acquisition, not every HF package
layout or platform.

Reproduce with an isolated launcher root and `PUMAS_REGISTRY_DB_PATH`, then run:

```sh
cargo run --manifest-path rust/Cargo.toml -p pumas-library --example intent_acquire -- /path/to/isolated-root ggml-org/test-model-stories260K
```

## Controlled verification

Completed native and operational regression checks:

- `intent_api_tests`: 9 passed.
- `api_tests --skip runtime --skip launch_llama_cpp`: 30 passed, 6 filtered.

The sandbox blocks IPC/loopback sockets; these fixtures were run with local socket
access enabled. Six runtime cases were excluded because of the separately recorded baseline
failures; this does not claim that the full operational API suite passes.

Final compiled library test binary filters passed:

| Filter | Passed | Ignored helpers |
| --- | ---: | ---: |
| `model_library::hf::` | 233 | 0 |
| `api::hf::tests` | 25 | 0 |
| `intent::` | 7 | 0 |
| `model_library::importer::tests` | 25 | 0 |
| `model_library::download_recovery::tests` | 32 | 1 |
| `model_library::artifact_identity::` | 11 | 0 |
| `model_library::download_store::` | 61 | 2 |

Together with the two integration suites above, **433 tests passed**, with three
ignored subprocess helpers and six excluded runtime cases. Suites ran against
`rust/target/debug/deps/pumas_library-74a3a1e3f9916b12` after
`cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --no-run`.
Each named filter can also be run via Cargo with `--lib <filter> -- --quiet`.

Static checks:

- `cargo clippy --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --tests --examples -- -D warnings`: passed.
- `cargo check --offline --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features -p pumas-uniffi`: passed.
- Native acquisition example build, exact Rustfmt edition 2021, Markdown relative
  links, and `git diff --check`: passed.

Review-driven regressions cover completed explicit-branch resolution after live
worker cleanup, exact-pin substitution refusal, active artifact-constraint
matching, selector-ID collisions, retained corrupt bytes, and canonical-facts
publication failure. Public fixtures cover coalescing, caller drop after
admission, preflight shutdown, upstream 403/503 refusal, and actionable ambiguity.
Distinct revision isolation is exercised in the real lower managed-worker
admission/persistence path; it is not a public concurrent two-revision test.

## Boundaries

I2 remains open for broader lifecycle/crash evidence. This milestone exercises
ordinary persisted resume and owned shutdown, without claiming general
hard-crash recovery or durable desired-state reconciliation. M3 ensure/release
and M4 existing-local-transport projections remain separate milestones. Existing
unrelated runtime-profile test failures are recorded in the execution ledger.
