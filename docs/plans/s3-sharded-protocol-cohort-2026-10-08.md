# Authenticated S3 sharded protocol cohort

This report describes focused synthetic protocol qualification of the shared format-independent acquisition/import bridge with a supported sharded safetensors package. It exercises existing transfer, receipt and publication interfaces without adding a storage service or runtime adapter.

## Real-service acceptance remains open

The original [S3 brief](../breif/s3-model-fetch.md) requires AWS S3, an independent compatible service, and local MinIO. Existing controlled HTTP/TLS tests do not fulfill those provider requirements. The [earlier provider report](artifact-acquisition/reports/s3-composed-provider-qualification-2026-10-05.md) already records the MinIO official-source HTTP 403 installation blocker.

Real-service tests were not executed because no supported local S3 service was available in the qualification environment. The recorded official MinIO source access returned HTTP 403; a separate official SeaweedFS release metadata request also failed with HTTP 403 before any service binary was downloaded. These environment blockers do not establish interoperability or a provider defect.

## Bounded fallback

`scripts/tests/qualify-s3-sharded-cohort.py` drives the new ignored `s3_sharded_cohort` Rust integration test. Python's standard-library TLS server is explicitly a synthetic protocol oracle. It creates owned real safetensors bytes (two named FLOAT32 tensors), config, tokenizer, and weight index files. It independently checks every production AWS SDK SigV4 signature using standard-library HMAC/SHA-256, an RFC 4231 known answer, and a wrong-secret negative check. Its fixed access/secret/session strings are synthetic and never sent outside literal loopback.

The complete case performs bounded two-object pages, conditional latest HEAD pins, explicit versioned HEAD selection, conditional range reads, a deliberately truncated first shard body, and retry at exactly retained offset 4096. Every resumed request retains the selected version and ETag. Supplied independent SHA-256 values become verified file receipts before the shared importer qualifies the package. The index names both present shards; config and tokenizer members are present. Atomic publication is checked by exact installed bytes, Ready import state, adopted acquisition, exact consumer receipt, and cold-open receipt equality. This proves import qualification and publication, not inference or backend compatibility.

The missing-shard case transfers and verifies every selected member, then rejects the index's absent required shard without a registered model or adopted acquisition. The arbitrary `.dat` case similarly verifies arbitrary bytes while refusing model publication. The cancellation case waits for actual shard-body progress before requesting cancellation, checks terminal Cancelled, incomplete verified membership, no completion receipt, and no registered model, and awaits owned shutdown.

Receipt checks bind acquisition ID, use lease, owner, demand operation, workspace, complete manifest, verified membership, and exact import specification. Refusals require the specific shared qualification Validation error and retained Using custody with the issued receipt. Successful publication requires a confirmed metadata identity and the matching Confirmed version-2 publication document containing the exact acquisition receipt. The actual import future is dropped, then the old API is shut down and dropped before reopening; cold metadata identity, receipt, and publication-document bytes must match.

TLS verification remains enabled. The existing repository test CA is trusted only by the Cargo child through its environment; global trust and network configuration are unchanged. The private fixture key is extracted into a temporary owned directory and removed with the fixture. Credentials, Authorization headers, and the private key are excluded from request evidence. The isolated registry uses the test's own directory.

## Reproduce

With official Rust dependencies already available and Python 3/OpenSSL installed:

```sh
ORT_SKIP_DOWNLOAD=1 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 \
  python3 scripts/tests/qualify-s3-sharded-cohort.py --output /tmp/pumas-s3-cohort-evidence
```

The runner uses locked offline Cargo with `--no-default-features --features s3`. The Rust test is ignored in ordinary test discovery because it requires this external owned fixture and isolated CA trust. The runner executes it explicitly with `--ignored`; successful output includes the exact Cargo command, sanitized request transcript, object digests, independent fixture failures, and evidence classification. The runner records scoped command output and fixture classifications for reproduction.

## Executed checks

The explicit ignored integration test passed all four cases across 61 independently signed requests. Resume used offset 4096, the oracle recorded no failures, and a wrong-secret check rejected every request. All four existing `s3_model_bridge` integration tests passed. Targeted strict Clippy, Cargo formatting, Python byte compilation and whitespace checks passed. This qualification changes no production code or dependencies; no new full-suite or real-provider run is claimed. Scoped checks had no baseline failures.

## Remaining production gaps

Actual AWS/non-AWS/MinIO interoperability, real version creation/storage, provider pagination/error behavior, and provider process loss are still unqualified here. The Python fixture does not implement a storage service. Listing plus conditional pinning is an observation, not an atomic multi-object snapshot; caller-authorized exact members and SHA-256 evidence remain required. The composed cohort covers a supported Transformers safetensors package and refusal cases, not every supported format. No unknown format is declared runnable, no downloaded custom code is executed, and no inference is performed. Existing shared receipt/recovery owners retain failed or interrupted custody; this work introduces no cleanup or recovery policy.

Run the Python driver explicitly to count the ignored cohort as executed. Its package cases require the shared safetensors importer, descriptor/receipt handoff and existing S3 selection/transfer interfaces. Keep real-provider acceptance open until supported service dependencies and actual provider tests are available.
