# Artifact acquisition contract

**Status:** Q1 implementation contract; its public API and acceptance remain pending.
**Canonical owner:** Pumas acquisition integration.
**Implementation authority:** [Acquisition plan](../plans/artifact-acquisition/plan.md).
**Consumer:** [Runtime installation and model-adapter plan](../plans/runtime-installations-and-model-adapters/plan.md), plus the existing model-library and native/package integrations.

This document owns the semantic handoff. Names below describe required domain values and obligations, not a preapproved Rust signature, JSON schema, endpoint collection or permanent type per row. Choose the smallest API that preserves these facts. The first implementation supplies exact typed constructors/decoders and fixtures here or through linked canonical generated sources before integration.

## 1. Authority and promise

A consumer provides an accepted immutable artifact specification and current authority to obtain/store it. Acquisition produces ordinary local files matching the requested evidence and retains them for the agreed consumption lifetime. It does not import models, install packages, choose runtime builds, execute code, allocate model slots or publish runnable state.

Three boundaries remain distinct:

1. **Resolution:** choose exactly which files and source versions are required.
2. **Acquisition:** obtain and verify those files under owned transfer and storage custody.
3. **Consumption:** validate/import a model, or extract/install/probe a runtime, then publish that consumer's result.

A consumer can fail after bytes are ready. That is not a failed HTTP transfer. The combined UI must preserve both states without marking the overall operation complete early.

## 2. Domain values

| Value | Required meaning | Excluded authority |
| --- | --- | --- |
| Artifact specification | Complete logical file set; source-object/revision evidence; sizes when known; required verification policy; non-sensitive provenance | Transient credentials, runtime readiness, package-solving logic |
| File identity | Selected representation, expected digest/algorithm when available, source-scoped immutable identity/validator, logical destination role/name | URL alone, filename alone, size alone |
| Source reference | Stable provider/configuration reference plus object locator and pinned version/evidence; refreshed access must preserve identity | Permission to change selection or execute a retrieved file |
| Authorization reference | Owned credential/policy handle and allowed destinations/redirect/retrieval behavior | A persisted bearer token, presigned URL in the artifact identity |
| Acquisition demand | The consumer's operation identity and retained request for these bytes, including policy and destination grant | Ownership of another consumer's demand |
| Attempt generation | Exact admitted execution generation; stale work cannot mutate a successor | A timestamp or arbitrary current process assumption |
| Workspace grant | Capability-backed authority to a bounded destination owned by the composition/consumer; mutation rights scoped to acquisition | An arbitrary caller path revived after restart |
| Verified file set | Validated files plus actual evidence, provenance and live use custody; optional per-file verified observations while set remains partial | A boolean promising the model or installation works |
| Consumer settlement | Explicit release or transfer of acquisition custody after consumer completion/failure and required cleanup | Automatic deletion of consumer-published files |

The same file digest may satisfy multiple specifications only when the receiving policy permits it. RuntimeInstallationId and ModelRef remain consumer identities. Do not replace them with acquisition IDs or digests. A source revision is not necessarily a byte digest; retain the distinction in the evidence.

## 3. Resolution and completeness

HF selection pins one concrete repository commit for the selected file set and preserves current file/variant intent. A runtime driver selects the native archive/build. A package owner resolves distribution artifacts. S3 supports explicit object references and explicit multi-file manifests; bounded prefix enumeration must complete and pin each selected object before it can claim completeness. Listings do not promise an atomic package snapshot without a source contract proving one.

Reject duplicate/colliding local logical paths, incomplete source manifests, unsupported representation kinds, contradictory size/digest evidence and unsupported versions. Validate platform-specific filename collisions at materialization without changing the source key. Encode object keys according to the source protocol, not by treating them as filesystem paths.

An object with only weak/no stable identity can be acquired as one complete transfer under an explicitly weaker caller policy, but cannot silently acquire strong resume/reproducibility or executable-origin guarantees. If the caller requires strong identity, return insufficient-evidence. Preserve current explicitly supported legacy policies without silently upgrading their claims or weakening strict new requests.

## 4. Public surface and state

A deep interface owns submit/acquire, observe/wait, pause/resume, cancel, open verified files, release/settle, and shutdown as necessary to current consumers. Reuse current operation/control handles and public domain facades where possible. No new public planning-session protocol is needed just because internal manifests are immutable.

Validate inbound and outbound wire/persistence representations at their receiving/destination boundaries. In-process consumers retain constructed validated values rather than re-decoding the same JSON. Status/progress is a projection of one owner, not another mutable store.

Illustrative lifecycle:

`Admitted → Transferring ↔ Paused/WaitingForAccess → Verifying → FilesReady → Settled`

Failures and cancellation retain their exact generation and move through owned cleanup or an explicit RecoveryRequired/Uncertain state. These labels do not replace existing domain states or mandate a single enum. Do not claim Paused until no worker still writes the affected destination. FilesReady requires complete selected content and required verification; a partly verified file set can only produce explicitly partial observations.

A disconnected observer does not cancel durably admitted work. Explicit cancellation records the decision, prevents new effects, signals workers, and observes terminal/cleanup effects. Dropping a future or lease wrapper does not prove executor I/O stopped. Synchronous Drop can release a safe local handle or enqueue cleanup with a retained supervisor, not detach required cleanup.

## 5. HTTP representation rules

Source readers use established HTTP parsing/transport facilities and preserve status, headers and representation facts needed by acquisition. Request exact bytes (normally identity content encoding); decompression must not silently alter offsets or the representation being hashed.

Resume requires verified local partial identity, an exact offset/length, and source identity evidence appropriate to that source. With strong HTTP validators, apply the relevant conditional request. Validate the returned range, total length where known, actual byte count and current validator. A `200` full-body response to a ranged request is a restart/replan outcome, never data to append. A `206` with wrong/missing range evidence is invalid. `416` is not proof of completion; inspect the selected length and verify the entire local file before any ready transition. `304`, short successful bodies, extra bytes and changed representations retain their owning result rather than becoming success.

A source can explicitly decline resume. Starting a fresh transfer is a policy-authorized owned action that preserves/disposes old partial state safely, not a silent best-effort splice. Unknown size is represented as unknown and bounded by consumer capacity; percentage progress is not invented. Zero-byte files and selected file-set completeness have explicit tests.

RFC authority: [HTTP Semantics, RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html), particularly conditional requests, Range, Content-Range and 206/416. These rules are an acquisition use of the protocol, not a new HTTP implementation.

## 6. S3 and other source readers

Source configuration includes endpoint, region where applicable, bucket, addressing style, TLS/approved local-development policy, and explicit credential-provider identity. Preserve object version IDs and conditional reads. Treat ETag as an opaque source validator unless its specific documented checksum semantics are established. A multipart or encrypted-object ETag must not be relabeled SHA-256/MD5 evidence.

Use one range per S3 GetObject call and bounded parallel calls only when the supported reader and identity contract permit them. Pagination failures or capacity limits produce incomplete/unavailable manifests, not silently shortened file sets. Recheck the exact version/evidence when refreshing credentials or location. Capability differences of compatible endpoints produce explicit unsupported results.

[AWS GetObject](https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html) and [Object metadata](https://docs.aws.amazon.com/AmazonS3/latest/API/API_Object.html) provide source semantics. A maintained SDK supplies signing and request mechanics. Pumas owns selected identity, allowed access, acquisition attempts, destination custody and verification.

A new source implementation can register behind the internal source-reader interface without changing model-adapter IDs or installation-state variants. This plan does not enable untrusted executable source plugins or assume all providers obey identical APIs.

## 7. Integrity, provenance and trust

Record separately: requested origin and revision, actual retrieval location, expected evidence and its authority, observed evidence, and consumer approval. Compare expected digests using the existing hashing facilities. A locally computed hash alone proves neither publisher origin nor permission to install.

Expiring locators are refreshable access material. Keep them in ephemeral/scoped storage rather than ordinary manifests/logs; when they cannot be recovered after restart, report WaitingForAccess. A refreshed credential/URL may access only the previously selected object, unless a new explicit selection creates a new request. Forward credentials across redirects only under the configured receiving-origin policy. Public progress identifies safe source labels, not query secrets or private object paths by default.

Private MinIO/enterprise endpoints can be explicitly allowed. Untrusted model metadata cannot authorize arbitrary host access, turn off TLS verification, change registries or expand credential scope. Local-cache access follows its retained consumer/policy scope; remote token expiry is not an instruction to delete already authorized local content, and possession of shared cached bytes is not authorization for another consumer.

Initial multi-location support means one selected source plus explicit complete-content reuse. A permitted mirror can satisfy the same expected digest, but matching ETag text from different origins is not equivalence. Cross-origin reuse of partial ranges is deferred until an independently verified chunk/range contract is implemented. Full re-acquisition may be offered explicitly instead.

## 8. Filesystem and handoff lifetime

The composition supplies trusted root capabilities; source metadata supplies only validated logical names. Maintain no-follow/containment and actual file/directory identity through effectful operations, including creation, replacement, deletion and reopening. Serialized paths/IDs are equality/context only, not filesystem authority.

Write only into owned staging. Finish and flush owned writes and apply the required durability barrier before exposing a ready handoff. Verify the selected file set. Seal its mutation grant and issue a read/use capability; consumers may obtain ordinary paths while retaining that capability. Partial observation can expose verified auxiliary files to the existing model workflow without exposing the entire model as ready.

Prefer same-filesystem staging/adoption for large models where the consumer can safely publish it. Provide ordinary copying when different storage or immutable-cache requirements demand it; support reflink only as an optimization. No unconditional cache-plus-full-model duplication, guaranteed zero-copy, or mandatory CAS. If a file/directory is adopted into a consumer's authority, record the transfer and exact identity before acquisition cleanup can reclaim the old workspace. A move that has unknown visibility remains an uncertainty state.

An installer holds the handoff until package/extraction workers and their cleanup finish. Mutation of installed outputs cannot modify shared cached inputs. Eviction considers durable demands, active read leases and uncertain handoffs. Release of one demand does not release another. If coalescing is not implemented, do not add an idle global demand scheduler just to prepare for it.

## 9. Persistence and consumer finalization

Persist the selected manifest/specification identity, current demand/attempt, source-scoped nonsecret revision evidence, workspace identity, validated resume progress and lifecycle dispositions. Reuse the owned atomic store/publication machinery after separating model-specific records. Runtime artifacts must not require a fake repo/model/library UUID. One shared acquisition implementation can retain a supported legacy decoding boundary, but two stores must not both authorize the same transfer.

Persisting progress is not the same as making the file bytes durable. After a crash, validate source identity and actual partial files under reopened authority; do not trust a saved byte counter as proof. Complete-file verification must re-establish readiness when prior verification cannot safely be reused. Unsupported recovery remains non-authorizing.

Acquisition FilesReady and model/runtime publication are distinct commits. Consumers record their own idempotent finalization linked to the exact request/generation. If a crash occurs after consumer commit but before acquisition settlement, retain inputs conservatively and reconcile through the consumer's authoritative result; do not automatically repeat installation/import or delete final output. There is no assumed cross-store atomic transaction or exactly-once remote-I/O guarantee.

Consumer settlement is idempotent for its exact generation. Old acknowledgements cannot release a successor. Durable unfinished consumption retains custody across restart even though in-memory RAII handles no longer exist. If confirmation is unavailable, surface recovery-required and bounded retained state, not deletion by age.

## 10. Failure, retries, progress and shutdown

Differentiate malformed request, unsupported source/representation, authorization-required/denied, source-changed, integrity mismatch, not found, network inconclusive/transient, storage full, busy/capacity, cancellation, consumer failure, and visibility/durability uncertainty. Project through existing public error contracts with bounded non-sensitive diagnostics; the list does not mandate a new cross-project universal error enum.

The source reader reports retryability and required refresh, not a second outer retry loop. Acquisition owns the combined retry budget; respect source Retry-After only within authorized policy. Paused or access-blocked jobs release active transfer capacity while retaining resumable custody. Hashing/writes use governed async/blocking capacity and never hold bookkeeping locks through external code or blocking I/O.

Progress separates logical verified bytes from wire bytes, retries and unknown totals; preserve current UI-visible domain progress. Bounded coalesced notifications have snapshot recovery and a reliable terminal/control path. Download 100% does not mean install complete. End-to-end cancellation follows the initiating domain's explicit action while retaining cleanup owners.

Shutdown closes new admission, signals appropriate work, drains tracked async and blocking effects, publishes the true resumable/terminal disposition, then releases custody. Repeated shutdown shares the owned result. An elapsed deadline can produce incomplete shutdown, not fictional cleanup success.

## 11. Package consumption

The package owner supplies a version-checked accepted resolution and trusted immutable artifact set. Acquisition obtains files; package tooling installs them into an owned empty stage from exact local inputs. Preserve original URLs/digests as provenance and validate the installed set afterward. Use only supported public tooling. A report is evidence about a resolution, not by itself a consumable lock format.

The decisive test denies network during the final installation leg and rejects hidden direct-URL retrieval, alternate same-name/version wheels, and missing closure members. Resolver metadata and managed-Python bootstrap traffic remain separately recorded; the claim is exact payload handoff, not interception of every package-tool request. Sources: [pip report](https://pip.pypa.io/en/stable/reference/installation-report/) and [pip install](https://pip.pypa.io/en/stable/cli/pip_install/).

## 12. Compatibility and extension

Internal coordinated DTOs, public Rust/IPC APIs, persisted formats, optional source implementations and consumer install/model records have different evolution obligations. Update actual generated consumers with their producer. New source support within this contract does not change model-adapter registration or runtime installation identity. Unknown schema/protocol versions are explicitly rejected, not decoded into weaker defaults.

Before independently deploying a breaking contract, disposition every supported public client and retained data state. Gate status is owned by the acquisition plan, not by a version number in this document. This document remains proposed until the implemented producer/consumer evidence exists.
