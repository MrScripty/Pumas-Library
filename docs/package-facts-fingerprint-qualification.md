# Package fingerprint repair qualification

Qualified on 2026-10-06. Repair commit:
`ee1c364654a40d7c02d58c59f8e3904d26ba1d20`, based on Pumas main
`5e114f6d8e4559e0a4d67e56000b423120a0fde0`. This report summarizes the recorded
local tests; publication does not constitute a new qualification run.

## Controlled importer result

The unchanged-source control reproduced
`Required pinned package-facts output is stale or unsupported`.
The repaired control completed through the managed Pumas importer, issued its
own validated pinned completion receipt, and settled custody to `adopted`.

| Observation | Unchanged source | Repaired source |
|---|---|---|
| Status | Error | Completed |
| Completion receipt | Absent | Present |
| Custody | Using | Adopted |
| Read-only currentness | 0/32 | 32/32 |
| Source requests | 0 | 0 |
| Shutdown | Same owned download failure | Success |

Each control used a separate new library/store and a local copy of the verified
Qwen payload. Hub, API and payload endpoints were replaced with a rejecting
loopback server. Pinned publisher metadata was supplied as fixture input; it was
not a new publisher fetch. The source and copied bytes were independently hashed:

- Repository: `Qwen/Qwen2.5-0.5B-Instruct-GGUF`.
- Revision: `9217f5db79a29953eb74d5343926648285ec7e67`.
- File: `qwen2.5-0.5b-instruct-q4_k_m.gguf`, 491400032 bytes.
- SHA-256: `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`.
- Source metadata license: `apache-2.0`.

The actual GGUF v3 parser output was present: architecture `qwen2`,
`MOSTLY_Q4_K_M`, tokenizer `gpt2`, chat template present, context 32768, embedding
896, blocks 24 and attention heads 14. The fixture checked metadata schema 2,
package-facts contract 3 and model-ref contract 1. The GGUF inspector revision
remains `llama-ftype-v1`.

## Validation and source trace

The producer constructs `PackageInspectionContext`, parses GGUF evidence and
persists facts under `PackageInspectionManifest::source_fingerprint`.
Receipt issuance recomputes the context through
`cached_model_package_facts_are_current` and the existing contract/artifact/
revision classifier. The repaired importer passed these checks and published its
own receipt; the test did not fabricate or force one.

The six fingerprint regressions passed, covering six insertion orders, sixteen
fresh deserializations per order, nested objects, six separate process hash
seeds, semantic/array changes, size/mtime changes and legacy hash invalidation.
The unchanged source failed the read-stability and process-seed controls.
The full managed-importer test passed explicitly with its 180-second completion
bound and rejected mismatched fingerprints and package-facts versions 2 and 4.

| Relevant suite or checks | Passed |
|---|---:|
| Package facts, including fingerprint regressions | 23 |
| Importer | 33 |
| Receipt store | 68 |
| Cache contracts | 7 |
| Freshness, canonical finalization, pinned verification and settlement | 9 |

These are 140 successful regression executions; one targeted case also appears
in the importer suite. Two receipt-store process-fixture tests remained ignored.
The explicit full Qwen importer control is additional. Coverage is limited to
these suites and the selected features. Rust 1.92.0 builds were locked/offline, with
`--no-default-features --features test-support` and `ORT_SKIP_DOWNLOAD=1`.
Default and test feature graphs contained no ORT `download-binaries` feature.
Reproduction commands are in the [repair documentation](package-facts-fingerprint-canonicalization.md).

## Compatibility and limits

Metadata object keys are recursively canonicalized; arrays, nulls and meaningful
values retain their significance. Existing raw-JSON fingerprints become stale
and require fresh observation through the normal owned producer. No old row or
receipt is relabelled, and pinned-output validation is unchanged.

The existing file fingerprint observes size and nanosecond mtime, not file
content. Same-size tampering that restores mtime remains outside that cache
contract; managed SHA-256 verification is separate. The GGUF parser reads the
version field without enforcing general rejection of unsupported versions, so
this qualification establishes the actual v3 fixture only.

The defect is sufficient to reproduce the complete importer failure and the
repair is sufficient for this controlled import. Exact reconstruction of the
historical fingerprint remains unresolved because its original generator inputs
were not logged. This does not prove that key ordering explains every historical
hash bit or that the preserved failed acquisition can safely be recovered.

All 77 original state files remained identical by SHA-256, size, mtime and inode.
Initial harness compile/lookup failures and corrected control evidence remain
preserved locally. Raw evidence, custody records, identifiers, workspace paths,
logs and weights are excluded from this publication. No redownload, original
record recovery, runtime startup or inference was performed; no story, voice or
production-loader behavior was qualified.
