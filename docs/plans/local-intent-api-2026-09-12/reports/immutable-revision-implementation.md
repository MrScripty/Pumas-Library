# M2a immutable-revision implementation

Status: M2a accepted within the evidence and limits below.
Date: 2026-09-12.

## Selected contract

M2a's private managed entrypoint accepts an already validated immutable commit.
Existing public operational methods use the same implementation with legacy
main behavior. Production resolution of branch/tag/default intent selectors,
`AllowUpstream`, and full intent acquisition remain M2 work.

The private revision context travels through metadata, repository-tree cache
keys, bundles, auxiliary and payload reads, artifact destinations, admissions,
worker execution, import, and persisted resume. The public request and
notification payloads retain their existing fields. Notification data does not
replace the owned importer's revision context.

## Durable compatibility

The existing download store owns v5 publication and validated v4 upgrade under
its existing instance and operating-system locks. Every v4 snapshot maps to
legacy main, including snapshots retained under custody records. V5 requires
explicit null-or-commit revision data. V4 readers reject schema 5.

`PersistedDownload` gains a public `revision: Option<String>` field. External
Rust struct literals must initialize it; this is a source compatibility break
to that lower-level type. `DownloadRequest` and binding request constructors
retain their existing contract.

Malformed source and failures proven before replacement preserve original
bytes. Post-replacement uncertainty can leave complete v5 visible; the owner
returns failure and preserves observed state without automatic rollback.

## Recovery boundary

Ticket recovery cannot represent a pin and refuses non-main upstream revisions.
Destination checks read both metadata and download markers through existing
held filesystem authority, preventing legacy recovery from overwriting pinned
artifacts. Native persisted resume retains its pin. This does not establish
the separately deferred recovery lifecycle/crash acceptance gates.

## Verification

Built the library unit-test executable with
`cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --no-run`,
then ran the following owner filters against that executable. HF/API fixtures
require loopback sockets and were run outside the socket-restricted sandbox.

| Owner filter | Result |
| --- | --- |
| `model_library::download_store::tests` | 61 passed; 2 subprocess helper entries ignored by the normal harness |
| `model_library::importer::tests` | 24 passed |
| `model_library::download_recovery::tests` | 30 passed; 1 subprocess helper entry ignored by the normal harness |
| `model_library::hf::` | 221 passed on the final run |
| `api::hf::` | 19 passed |
| `model_library::artifact_identity::` | 10 passed |

The new evidence includes strict commit validation, stable identity before and
after implicit file expansion, equal-pin joining, distinct-pin admissions with
identical file evidence, absent/mismatched upstream SHA refusal before admission,
moving-main metadata/tree isolation, pinned bundle preflight, and auxiliary and
payload transfer through retry, persisted reopen, ranged resume, and real import.
The worker fixture uses the payload's actual SHA-256. Its failed transfer leaves
an error with partial bytes and a marker; it does not publish a final payload.
This proves the exercised transfer checks, not a new general hash-verification
or tensor-integrity contract.

Migration tests reopen valid records produced by real admissions, quarantines,
revocations, and interrupted confirmation. They prove custody preservation,
malformed-input refusal, explicit revision requirements, pre-publication failure
preservation, and uncertain post-publication failure without rollback. Downgrade
refusal follows the original v4 reader's strict schema-version check against the
verified v5 output; an old binary was not executed.

Additional Cargo verification:

- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --test intent_api_tests`: 9 passed.
- `cargo test --offline --manifest-path rust/Cargo.toml -p pumas-library --test api_tests -- --skip runtime --skip launch_llama_cpp`: 30 passed, 6 filtered.
- `cargo clippy --offline --manifest-path rust/Cargo.toml -p pumas-library --lib --tests --example intent_model -- -D warnings`: passed.
- `cargo check --offline --manifest-path rust/Cargo.toml -p pumas-rpc -p pumas-uniffi`: passed.

Six existing runtime-profile API cases remain excluded for the previously
documented baseline failures (I7). Initial HF socket failures were environmental;
the outside-sandbox run exercised those fixtures successfully. Six old lifecycle
fixtures required valid JSON markers or fault injection after admission to keep
testing their original admitted-error behavior under the new preflight guard.
Production provenance refusal was not weakened. One uppercase commit fixture
was corrected from 42 to 40 characters.

Evidence is Linux/native with controlled local upstream fixtures. Production
branch/tag resolution, external HF acquisition, verified intent availability,
desired-state persistence, and the broader recovery/crash gates remain pending.
