# Bounded S3 prefix discovery: candidate decision

Date: 2026-10-05. Status: investigation complete; SDK ownership decision pending.
This is a documentation checkpoint, not an implemented discovery adapter or
provider acceptance. It preserves the bounded AC10 checkpoint without extending
its performance or platform claims.

## Authority and preserved checkpoints

[Contract sections 3 and 6](../../../contracts/artifact-acquisition.md) authorize
bounded prefix enumeration with complete selection and per-object pins. AD5 and
AC13 in the [plan](../plan.md) require pagination failure to remain incomplete
and forbid treating a listing as an atomic package snapshot. The
[architecture review](architecture-review.md#bounded-library-and-mechanism-decisions)
selects `object_store` for initial evaluation and requires evaluating a maintained
direct S3 SDK if a necessary guarantee cannot be represented. It does not select
an automatic fallback, second SDK, fork, or local S3 response parser.

The coordinator requested options before implementing an unapproved architecture
choice. That condition applies: the missing completion evidence requires changing
the protocol dependency or its ownership. No implementation or dependency change
was made. No credentials, account resources, provider service, native SDK or ONNX
download was used. Public crate archives were downloaded only for source inspection;
they were not installed or executed. PRs, integration and merges remain coordinator-owned.

The separate documentation branch starts at AC10 checkpoint
`d56b2b91b7d0d4cc38627e6eab541b937ef5f027`, tree
`77decaa50029652b54df83592a3ee5f122d17c3b`. Remote refs observed during inspection:

| Checkpoint | Head | Tree |
| --- | --- | --- |
| Accepted main | `838eb2990905144a59830f1a16fe91b4e1105d4d` | `f6d6c1d3c5ba5be0fb998a6fd157fcb31c63dc72` |
| Frozen composed S3 | `493b935c6a41d4d4aeca8e8a66f10b4aba114365` | `ecb51f20c0740fa7d88e6d0eface06c45feb2e07` |
| PR41 formatter successor | `26a84e323cae566a46a8f76bef48fa1010aed48b` | `3ee66988eb1668188011b2124890b10031403ebd` |
| Frozen AC10 | `d56b2b91b7d0d4cc38627e6eab541b937ef5f027` | `77decaa50029652b54df83592a3ee5f122d17c3b` |

The existing watcher/importer/S3 manifest repair files remain untouched. The base
of an eventual implementation must be selected by the coordinator; this report
does not combine the frozen composed S3 branch with PR41 or AC10.

## Deciding SDK evidence

The repository pins optional `object_store =0.12.4` with default features disabled
and `aws` enabled. Its internal list parser retains contents, common prefixes and
the next token, but discards `IsTruncated`; its public object stream also hides page
boundaries. A truncated response without a continuation token can therefore look
complete to an adapter consuming that stream.

The published `object_store 0.14.2` has a public
[PaginatedListStore](https://docs.rs/object_store/latest/object_store/list/trait.PaginatedListStore.html)
with raw string prefixes, `max_keys`, and continuation tokens. Unlike the ordinary
list API it does not append `/`. However, its published `src/client/s3.rs:28`
still omits `IsTruncated`, and `src/aws/client.rs:1032` extracts only
`next_continuation_token`. There is no internal check relating that token to the
discarded truncation flag. A version upgrade alone does not close this gap.

[AWS ListObjectsV2](https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html)
defines the completion flag separately from its continuation token and warns that
a successful HTTP response can contain invalid XML. SDK parsing success and token
absence alone are insufficient completion evidence for the requested contract.

The published `aws-sdk-s3 1.137.0` output preserves both `is_truncated: Option<bool>`
and `next_continuation_token: Option<String>`; its generated XML decoder parses
both fields. Its service builder exposes explicit credentials, endpoint, region,
addressing, HTTP client, retry/timeout settings, disabled S3 Express session auth
and anonymous `allow_no_auth`. These are API-fit observations, not a claim that
the transport or credential policy is qualified.

Registry metadata and archive contents agree on the following facts:

| Candidate | Declared Rust minimum | License | Archive SHA-256 |
| --- | --- | --- | --- |
| `object_store 0.14.2` | 1.85 | MIT / Apache-2.0 | `f1796bc93603f78c5760a69f2d58badc9618d22adade0a95385bb2adbae4eb94` |
| `aws-sdk-s3 1.137.0` | 1.91.1 | Apache-2.0 | `c2dd7213994e2ff9382ff100403b78c30d1b74cdfcd8fa9d0d1dc3a94a5c4874` |

Archives came from `static.crates.io` and matched the registry checksums. AWS crate
provenance identifies upstream commit
`b05d1f072f646d8e22cb9b639de6126859e502b7`, path `sdk/s3`.
The newest registry AWS S3 release inspected, `1.152.0`, declares Rust 1.94.1;
the repository pins Rust 1.92. `1.137.0` is the newest non-yanked S3 version in
the inspected registry metadata whose declared minimum fits that toolchain.
Its 14 direct AWS/Smithy minimum-version requirements also declare Rust 1.91.1.
These are metadata checks, not a successfully resolved or compiled dependency
graph: semver requirements can select newer transitives, and a compatible locked
graph still needs proof. No toolchain upgrade is proposed.

## Options and recommendation

| Option | Result and cost | Recommendation |
| --- | --- | --- |
| Evaluate one maintained direct AWS SDK for the optional S3 reader, then replace `object_store` if it passes | Preserves completion fields and gives one protocol/credential owner; must requalify existing anonymous/authenticated HEAD and range behavior, source compatibility, transport and build isolation | Preferred evaluation path; replacement requires explicit selection after the bounded prototype |
| Add AWS SDK only for discovery; retain `object_store` for reads | Can isolate listing changes, but retains two protocol clients, credential lifetimes, transport adapters and dependency graphs | Available if the coordinator prioritizes migration isolation and accepts dual ownership |
| Keep incumbent SDK and obtain a maintained upstream API/validation repair | Preserves the existing owner; no inspected published release supplies the necessary guarantee | Valid deferral path, but does not deliver this missing feature now |

Recommend the first option as a bounded suitability prototype using candidate
`=1.137.0`, not an immediate SDK migration. Keep defaults disabled; initially
evaluate `rt-tokio` and `http-1x` with an explicitly supplied transport. Do not add
`aws-config`, ambient loaders, default HTTPS client discovery, SigV4a, automatic
S3 Express sessions or a toolchain bump by convenience. Record the actual resolved
features, licenses and targets before selecting the dependency. A local XML guard,
vendored patch or fork would be a separate protocol-maintenance decision and is
not selected here. No upstream issue or reviewer was contacted.

## Smallest proposed acquisition boundary

After dependency selection, add one reader-owned bounded prefix-selection method.
Keep SDK types private and existing `S3ReaderConfig`, anonymous `new` and explicit
authenticated construction compatible where practical. A request supplies the
exact prefix, explicit logical-path mapping and trusted expected SHA-256 evidence;
the already constructed reader supplies provider/access authority. A listing does
not supply digest authority. Require evidence for every selected object rather
than weakening the current SHA-256 policy to admit arbitrary discovered bytes.

The operation owns in-memory continuation state and explicit positive page,
object, metadata-byte, response-byte and elapsed-time limits. It makes bounded
single-page calls without a delimiter, preserves the raw prefix without appending
or normalizing it, rejects out-of-prefix/duplicate keys and malformed pages, and
rejects missing or inconsistent completion flags/tokens and empty or cyclic next
tokens. Cancellation, timeout, capacity and pagination failures return an error;
there is no partial manifest labeled complete or new durable planning-session
protocol. SDK retries must not replace the acquisition owner's retry policy.

ListObjectsV2 does not provide VersionId pins. Resolve every selected key with
HEAD, require a non-null immutable VersionId, compare the listed validator and
size, then confirm the exact version under the existing selection/read contract.
Return source-changed or insufficient evidence on disagreement. Sort accepted
entries deterministically, retain exact key/version identity and existing receipt
encoding, and validate the complete logical/staging namespace before acquisition
admission. Even a complete enumeration of individually pinned objects is not an
atomic package snapshot and cannot detect every concurrent prefix membership
change; stronger coherence requires an authoritative explicit source manifest.

The implementation must retain explicit credentials only in memory, HTTPS for
authenticated production reads, unit-test-only plaintext loopback credential
fixtures, no ambient proxies or credentials, no redirect scope expansion and
redacted debug/errors. Requalify signing/session-token absence and presence,
anonymous behavior, malicious continuation fields, interrupted/cyclic pagination,
bounds, HEAD races, deterministic ordering, unchanged receipt identity and optional
feature isolation. AC13/AC14, live AWS/non-AWS/MinIO and native platform acceptance
remain pending. Frozen native repair files are outside the implementation write set.

## Inspection evidence and next decision

Local inspection evidence is under `/workspace/scratch/s3-prefix-decision/`:
`dependencies.json`, registry metadata, checksum-verified archives and selected
source files, plus `aws-direct-minimum-requirements.json`. The first archive
inspection stopped because the Apache archive has no `.cargo_vcs_info.json`;
the corrected inventory records absent VCS metadata without inventing provenance.
Some browser source URLs were unavailable; published registry archives supplied
the exact-version parser evidence. No parser fixture, SDK build, new provider test
or acquisition test was executed because no implementation was selected.

The concrete blocker is the unselected protocol ownership mechanism, not missing
real account credentials. Select the single-SDK evaluation or the dual-SDK scope
before code changes. Prefix discovery remains the next existing-plan feature;
AC10 measurement and additional hardening are not substitutes for it.
