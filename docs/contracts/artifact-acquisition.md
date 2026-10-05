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

Reject duplicate/colliding local logical paths, including collisions between any selected final path and a derived `.part` staging path, as well as file/directory prefix conflicts across that composed namespace. Validate the complete namespace before workspace mutation, independent of manifest entry order, using the same staging-path mapping as workspace operations. The portable collision key normalizes each component to NFC, applies lowercase mapping, then normalizes to NFC again; preserve the original logical path and source key. This rejects canonically equivalent spellings under that comparison but does not claim to reproduce every target filesystem's identity rules. Also reject incomplete source manifests, unsupported representation kinds, contradictory size/digest evidence and unsupported versions. Validate additional target-platform filename collisions at materialization without changing the source key. Encode object keys according to the source protocol, not by treating them as filesystem paths.

An object with only weak/no stable identity can be acquired as one complete transfer under an explicitly weaker caller policy, but cannot silently acquire strong resume/reproducibility or executable-origin guarantees. If the caller requires strong identity, return insufficient-evidence. Preserve current explicitly supported legacy policies without silently upgrading their claims or weakening strict new requests.

## 4. Public surface and state

A deep interface owns submit/acquire, observe/wait, pause/resume, cancel, open verified files, release/settle, and shutdown as necessary to current consumers. Reuse current operation/control handles and public domain facades where possible. No new public planning-session protocol is needed just because internal manifests are immutable.

Validate inbound and outbound wire/persistence representations at their receiving/destination boundaries. In-process consumers retain constructed validated values rather than re-decoding the same JSON. Status/progress is a projection of one owner, not another mutable store.

Illustrative lifecycle:

`Admitted → Transferring ↔ Paused/WaitingForAccess → Verifying → FilesReady → Settled`

Failures and cancellation retain their exact generation and move through owned cleanup or an explicit RecoveryRequired/Uncertain state. These labels do not replace existing domain states or mandate a single enum. Do not claim Paused until no worker still writes the affected destination. FilesReady requires complete selected content and required verification; a partly verified file set can only produce explicitly partial observations.

A disconnected observer does not cancel durably admitted work. Explicit cancellation records the decision, prevents new effects, signals workers, and observes terminal/cleanup effects. Dropping a future or lease wrapper does not prove executor I/O stopped. Synchronous Drop can release a safe local handle or enqueue cleanup with a retained supervisor, not detach required cleanup.

## 5. HTTP representation rules

### Authenticated transport and caller migration

`AcquisitionConsumer::acquire_http` accepts `Into<AcquisitionHttpClient>`.
Existing `reqwest::Client` arguments retain their configuration for requests
without an explicit `AcquisitionHttpSource.authorization` value or URL userinfo. Passing an
opaque existing client with explicit source authorization or detected URL
userinfo now returns a typed
validation refusal before a network request, including for an HTTPS URL. This
is a deliberate behavioral change for authenticated external callers; source
compatibility alone does not preserve their authenticated behavior.

Authenticated callers must construct
`AcquisitionHttpClient::https(their_configured_reqwest_builder)` and pass that
client. This retains proxy, custom CA, timeout, user-agent and redirect options,
but enforces HTTPS on the initial request and every followed redirect. A
custom redirect policy cannot authorize a plaintext downgrade. No global trust
store or caller client is rewritten. Credentials embedded in a caller's opaque
client configuration remain that caller's responsibility; the explicit source
credential channel must use the HTTPS-only constructor. Plaintext literal-IP
loopback credential fixtures exist only in unit-test builds and are not enabled
by the public `test-support` feature or a production constructor.

Continuation still requires a matching strong ETag and total length. For
`github` on `release-assets.githubusercontent.com`, only SAS expiry/signature
and JWT transport-grant fields are ignored in effective-resource comparison.
For `huggingface` on the recognized HTTPS delivery hosts
`cdn-lfs.huggingface.co`, `cdn-lfs.hf.co`, `cas-bridge.xethub.hf.co`, and
`us.aws.cdn.hf.co`, only the known AWS signing/expiry fields are ignored. Origin, port, path, version/query
selectors, response overrides and every unknown query field remain part of
resource identity. Other providers and hosts retain exact URL comparison.
This is bounded HTTP adapter behavior, not S3 acquisition support. Signing
parameter semantics are documented by [AWS SigV4](https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-query-string-auth.html)
and [Azure SAS](https://learn.microsoft.com/en-us/rest/api/storageservices/create-service-sas).


Source readers use established HTTP parsing/transport facilities and preserve status, headers and representation facts needed by acquisition. Request exact bytes (normally identity content encoding); decompression must not silently alter offsets or the representation being hashed. Inspect every `Content-Encoding` field before exposing the response body: accept no field or exactly one `identity` field, and refuse duplicate fields or unsupported encodings with typed response validation.

Resume requires verified local partial identity, an exact offset/length, and source identity evidence appropriate to that source. With strong HTTP validators, apply the relevant conditional request. Validate the returned range, total length where known, actual byte count and current validator. A `200` full-body response to a ranged request is a restart/replan outcome, never data to append. A `206` with wrong/missing range evidence is invalid and must contain exactly one `Content-Range` field; duplicate fields are ambiguous even when one value matches the selected offset and length. `304` and `416` are refused in this slice; neither is completion evidence. Any future completion path after `416` must independently verify selected length and the entire local file. Short successful bodies, extra bytes and changed representations retain their owning result rather than becoming success.

The Q1 HTTP continuation checkpoint is private runtime evidence under the same live acquisition service and workspace capability. It binds the acquisition, exact demand and manifest/file selection, workspace owner, initial request URL and effective response resource, strong ETag, observed total, and regular-file prefix identity/length/SHA-256. The prefix digest records bytes actually streamed; hashing an arbitrary existing partial cannot create this evidence. Recheck the prefix through the held parent capability before the request and after the HTTP wait, retaining the same checked descriptor for append. Continue with `Range` and `If-Match` using the checkpoint's strong ETag. A successful continuation response must have that same strong ETag and effective response resource, including origin and path; identical ETag text on another resource is not equivalence. Changed, missing or weak validators, changed resources and `412` refuse continuation without append. A `200` to a conditional range request replaces from byte zero only when its validator/resource still match; validate its entire body before publication. Weak ETags and Last-Modified do not authorize partial continuation in this slice. Loss of either runtime owner, or a missing/conflicting/mutated checkpoint, requires a fresh byte-zero transfer under the current selection. A full-size digestless partial is never completion evidence; the only whole-partial shortcut independently verifies the selected expected SHA-256 before publication.

Warm-checkpoint retention is optional and bounded per acquisition service. The number of retained file checkpoints cannot exceed its validated worker capacity, and one checkpoint's encoded metadata is limited to 64 KiB before its record is cloned. These limits are shared by all consumers and service views. Capacity or metadata pressure preserves live checkpoints, the durable demand, and the partial file while skipping retention; it does not change the transfer's pause, cancellation, or failure result. An incomplete later attempt without its exact checkpoint starts at byte zero. Dead workspace owners are pruned during lookup and insertion without extending their lifetime. This bounds checkpoint metadata only; it is not a peak-memory or full resource-envelope qualification.

A source can explicitly decline resume. Starting a fresh transfer is a policy-authorized owned action that preserves/disposes old partial state safely, not a silent best-effort splice. Unknown size is represented as unknown and bounded by consumer capacity; percentage progress is not invented. Zero-byte files and selected file-set completeness have explicit tests.

RFC authority: [HTTP Semantics, RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html), particularly conditional requests, Range, Content-Range and 206/416. These rules are an acquisition use of the protocol, not a new HTTP implementation.

## 6. S3 and other source readers

Source configuration includes endpoint, region where applicable, bucket, addressing style, TLS/approved local-development policy, and explicit credential-provider identity. Preserve object version IDs and conditional reads. Treat ETag as an opaque source validator unless its specific documented checksum semantics are established. A multipart or encrypted-object ETag must not be relabeled SHA-256/MD5 evidence.

The optional S3 reader retains `S3ReaderConfig` and anonymous `S3Reader::new`.
`S3Reader::new_authenticated(config, S3Credentials)` consumes explicitly supplied
access-key ID, secret and optional session token for a bounded acquisition.
`S3Credentials::new` admits nonempty printable ASCII without whitespace; access-key
IDs additionally exclude SigV4 credential-field delimiters `/`, `,`, and `=`.
Authenticated signing regions use ASCII alphanumerics and hyphens. Invalid
credentials/configuration fail before I/O with non-sensitive configuration errors.
Authentication requires HTTPS regardless of `allow_http`; plaintext literal-IP
loopback signing fixtures exist only in unit-test builds, never via `test-support`.
No ambient discovery, persistence, automatic refresh, anonymous fallback, or
receiving-origin expansion is allowed. Redirects and proxies remain disabled.
The maintained SDK supplies SigV4; Pumas marks credential headers sensitive and
contains remote diagnostics before public errors or durable status can expose
them. Existing anonymous constructors, source identity, VersionId/If-Match,
receipt formats, retry budgets and byte-verification policy remain unchanged.
Selections retain their credential capability in memory through their existing
ownership lifetime; callers scope them to the authorized acquisition.

The optional native `PumasApi::import_s3_model` facade consumes explicit source
facts, pinned manifest entries, a model import spec, a reserved workspace, finite
retry budgets, a retained operation UUID and optional ephemeral credentials.
Its request is not Debug/serde-enabled; only phase/current-file byte progress is
serializable. Selection runs under the same bounded acquisition consumer scope
as transfer, and verified single/bundled GGUF publication uses the existing
importer and exact receipt pipeline. The stable demand owner is
`model.s3.workflow`. Neither source access nor credentials enter the importer
payload or progress; no account/source-configuration persistence is introduced.

Control cancellation and finalization have one atomic admission winner.
Cancellation can win through verification but is refused before receipt issuance
once finalization starts. A cancellation acknowledgement is not a stopped-effects
result. Final success follows durable settlement; a drain error preserves any
published result and original failure. Dropping the waiter is interruption, with
shared shutdown responsible for registered effect drainage. Retained custody must
be reconciled under the same operation identity, never implicitly replayed with
new demand identity. Desktop/RPC composition and live-provider acceptance remain
unqualified by this native entry point.

The optional RPC/desktop composition admits one explicit anonymous GGUF through
that facade. It requires caller-supplied HTTPS origin, bucket, region, addressing,
exact key, immutable VersionId and SHA-256; its closed wire rejects credential
fields and HTTP opt-outs. The backend retains one process-local UUID-correlated
job/result and drains it before shared acquisition shutdown. Current-file byte
observations use decimal strings; cancellation acknowledgement is distinct from
the owned terminal result. Existing import validation, classification,
registration and exact receipt identity remain authoritative. Retained custody
requires explicit reconciliation, never implicit replay. Source facts and access
material are not part of progress or new account/configuration persistence.
The dialog is anonymous only; this does not qualify an authenticated RPC secret
boundary, packaged/browser behavior or live-provider acceptance.

Explicit multi-file S3 selections identify each object by the pair of its exact key and VersionId. Their manifest files encode that pair as a JSON tuple in the opaque `source_key`, so different versions of one key may have different size/digest evidence. Conflicting evidence for the same key and version remains invalid. Per-object revision pins and reader requests preserve the raw protocol key and VersionId; single-object selections retain their existing raw `source_key`. Retained manifests are not rewritten.

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

Persist the selected manifest/specification identity, current demand/attempt, source-scoped nonsecret revision evidence, workspace identity, observed progress and lifecycle dispositions. Observed or persisted progress does not authorize ranged continuation. Reuse the owned atomic store/publication machinery after separating model-specific records. Runtime artifacts must not require a fake repo/model/library UUID. One shared acquisition implementation can retain a supported legacy decoding boundary, but two stores must not both authorize the same transfer.

The current model-download document is schema 5, with schema 4 as its supported legacy input. Q1 initially added neutral acquisition facts to that same physical store under schema 6; the exact importer-receipt extension advances the acquisition document to schema 7. This does not create a second transfer store or reinterpret old model records as neutral manifests. Schema 7 retains the complete legacy model-download, recovery, queue, hidden-admission, release-proof and quarantine partitions, including unresolved dispositions, alongside neutral acquisition state and a separate versioned model-consumer receipt partition. That partition is not part of the strict schema-5 model projection. New empty roots use schema 7. Ordinary open of schema 4, 5, or pre-receipt schema 6 reports that explicit migration is required and does not publish or authorize neutral work. A separate, explicit offline migration may convert supported schema 4/5 state or pre-receipt schema 6 to schema 7, preserving all model and neutral acquisition facts, leases, queues, revocations, hidden admissions, quarantines, and cleanup dispositions while creating no historical consumer receipts. Every old reader/writer, including one holding cached state, must be stopped before conversion; the new file lock does not exclude an old process that resumes later. Unsupported, corrupt, or unknown state fails closed without guessed conversion. The canonical `downloads.json` reader also rejects repeated JSON object member names at any nesting depth, including names that become equal after JSON string unescaping, before exposing acquisition, model, or receipt state. This strict rule is scoped to that store; unrelated JSON readers retain their own decoding contract. All supported readers reject unknown schema/receipt versions without rebuilding or publishing. Pending cleanup remains non-authorizing and is never replayed by migration. No automatic downgrade or old-binary rollback is supported after schema 7 publication; older strict readers must reject it without rewriting it. Actual deployed schema-6 population and writer/rollback state are unknown; only disposable fixtures qualify this transition here.

Q1 does not persist its HTTP continuation checkpoint, validator, access URL or prefix proof. Schema 7 and consumer receipt versions are unchanged, and no checkpoint sidecar is introduced. Reopened service/workspace owners retain durable demand and selected-manifest custody but begin each incomplete file with a fresh request from byte zero; they never reconstruct a continuation proof from the saved counter or `.part` bytes. Ordinary HF pause/resume constructs a new workspace capability, so its successor also restarts incomplete files from zero; retries inside the same held worker may continue when the checkpoint remains valid. The shared schema does not fence an older schema-7 binary that still trusts partial length; exclusion of older concurrent writers and rollback remains a deployment obligation and unqualified evidence gap.

Persisting progress is not the same as making the file bytes durable. After a crash, validate source identity and actual partial files under reopened authority; do not trust a saved byte counter as proof. Complete-file verification must re-establish readiness when prior verification cannot safely be reused. Unsupported recovery remains non-authorizing.

Acquisition FilesReady and model/runtime publication are distinct commits. The managed HF importer issues receipt version 1 only after complete finalization and durable metadata, index and applicable pinned package-facts outputs. The receipt binds the acquisition ID, persisted `Using` lease, HF demand and operation, current queue admission, manifest and ordered verified-file receipts, destination/workspace identity, resulting model ID, and a versioned canonical output projection. Canonical JSON sorts object keys recursively, preserves array order and explicit nulls, and rejects unsupported values. Its projection contract explicitly lists metadata, model-index and package-facts fields; package-facts content includes its independent contract version. Cold recovery compares current read-only outputs to the issuer-published proof, never computes new proof from current outputs alone. Partial imports and ordinary model-import callers have no receipt authority. A crash before receipt publication, unsupported/malformed receipt, or changed/missing output remains recovery-required and never replays import effects. Unknown publication visibility is failure, not success.

The current Q1 candidate also routes llama.cpp archive acquisition through the shared consumer. Its native receipt binds the exact tag/metadata and hashes of the extracted output tree and launcher. The installer claims its cancellation/publication arbitration before returning the prepared receipt payload, so an accepted cancellation cannot later be replayed as an installation; after the claim, cancellation is refused and restart may finish the exact staged publication. If cancellation wins before receipt issuance, the installer durably revokes the exact attempt, drains and removes its owned workspace under the native lock, then withdraws only the unchanged, receipt-free `Using` lease. Cleanup or withdrawal failure retains recovery custody and prevents same-tag retry from selecting the unresolved attempt. Cold recovery verifies or completes publication from a committed receipt, settles the same acquisition, and explicitly reclaims its owned workspace. A retained `Using` acquisition without a receipt or exact withdrawal remains unresolved. Source-only composed review found no substantiated P0–P3 issue; objective-level consumer/platform evidence is still required.

Receipt publication conditionally validates the exact durable `Using` lease, demand/manifest/files, non-revoked queue admission, workspace/destination, and held root grant in the same canonical store transaction. A cold worker does not renew or replace that lease before checking its receipt. Only after read-only validation may it obtain a private receipt-qualified settlement capability. Acquisition transition to `Adopted` and exact queue release are one atomic `AcquisitionStore` document publication; the immutable receipt remains paired with the adopted acquisition as completion history. Thus recovery has no intermediate acknowledgement-with-unreleased-queue state. An identical already-published receipt is idempotent; conflicting, orphaned, duplicated, malformed, or unknown-version receipts fail closed. No implicit receipt pruning exists; any future terminal-record compaction must remove the exact acquisition and receipt together under a separately selected retention policy. Migration never manufactures receipts, so pre-receipt `Using` remains unresolved even if output files match. No cross-store exactly-once remote-I/O guarantee is assumed.

Consumer settlement is idempotent for its exact generation. A matching visible receipt does not establish durability: retries of receipt issuance or settlement must complete a successful durable publication before reporting success, including after reopening a store whose earlier rename succeeded but parent-directory sync failed. Old acknowledgements cannot release a successor. Durable unfinished consumption retains custody across restart even though in-memory RAII handles no longer exist. If confirmation is unavailable, surface recovery-required and bounded retained state, not deletion by age.

Every effectful in-place model import, including orphan adoption, acquires the configured model-root execution authority and rechecks current durable model/acquisition custody immediately before metadata-present shortcuts, index publication, Diffusers delegation, or other importer effects. A scan is advisory and does not reserve the target. Ordinary imports receive no custody exemption. Managed Hugging Face has two private, operation-scoped stages: its partial metadata/index stub may proceed only under the current exact queue admission, `Transferring` generation, manifest/demand, workspace, destination, and held root grant; full finalization may proceed only under the current verified-file use lease and exact matching model admission. The partial-stage capability authorizes only the existing stub upsert and its index projection, including any metadata projection performed by indexing; it cannot authorize full import, package-fact resolution, another target, or a caller-supplied path or operation string. Neither stage bypasses unrelated queue/hidden custody or unresolved Pending/quarantined state. The held root grant and existing operation owner remain live until all importer/index effects settle, including after a caller stops waiting. Missing configured authority or unresolved custody fails closed. The current candidate's HF receipt now permits a reopened `Using` record to settle only after exact receipt and read-only output validation; missing or mismatched proof remains recovery-required and never replays importer effects.

### Durable handoff transition ownership

| Transition | Durable writer and exact identity | Filesystem authority and cancellation | Restart and reclamation |
| --- | --- | --- | --- |
| Admission → transfer | Acquisition records the selected manifest, demand, and attempt generation before starting network work. | The acquisition workspace grant is scoped to this generation; the caller cannot revive a serialized display path. | An admitted record without a live worker is recovered only after reopening its grant and revalidating the source identity. No input is reclaimed while the demand is retained. |
| Transfer → paused, waiting, or verifying | Acquisition records the observed transition; saved byte counts are progress only. | Pause is published only after the generation's write-capable worker and nested file effects have drained. Cancellation prevents successor writes and retains cleanup custody until they drain. | Reopen validates the actual partial bytes and matching source validator before a range request; otherwise it restarts that file from zero. |
| Verification → FilesReady | Acquisition durably flushes and verifies the complete selected set before it publishes FilesReady and a sealed read/use lease. | The live lease refers to the held workspace authority and exact attempt generation. A path string alone does not keep inputs alive. | Reopen re-establishes required byte evidence before issuing a new lease. No consumer may observe a set as complete before FilesReady is durable. |
| FilesReady → consumer using | Acquisition records the consumer demand and operation generation before handing out a lease; the model or runtime consumer remains authoritative for its own operation state. | The consumer holds the lease through importer/extractor work and its cleanup. Cancel is routed through that consumer and does not release acquisition custody early. | An unfinished durable demand retains the files after process restart even though the in-memory lease is gone. |
| Consumer using → committed | The consumer writes its own publication record; acquisition records the exact-generation settlement acknowledgement separately. | The consumer's publication authority owns installed/imported output. Acquisition may reclaim only its own workspace after it observes that output identity and the consumer's worker cleanup. | If consumer commit precedes acquisition acknowledgement, reconcile against the consumer's exact durable generation; do not repeat import/install or remove uncertain inputs. |
| Consumer committed/failed → settled | Acquisition settles only the acknowledged demand and attempt generation; the consumer does not rewrite acquisition state. | Adopted output is recorded before workspace release and is never deleted by acquisition cleanup. Failure retains uncertain input until required cleanup has a positive result. | Old acknowledgements cannot release a successor. Unknown cleanup or publication visibility remains recovery-required; elapsed time is not a reclamation proof. |

The composition root owns shutdown ordering: stop new admission, drain each consumer and its cleanup while its leases remain valid, then drain acquisition workers and persist their truthful resumable or terminal disposition. A consumer-specific shutdown must not close the shared owner while another registered consumer can still use it.

## 10. Failure, retries, progress and shutdown

Differentiate malformed request, unsupported source/representation, authorization-required/denied, source-changed, integrity mismatch, not found, network inconclusive/transient, storage full, busy/capacity, cancellation, consumer failure, and visibility/durability uncertainty. Project through existing public error contracts with bounded non-sensitive diagnostics; the list does not mandate a new cross-project universal error enum.

The source reader reports retryability and required refresh, not a second outer retry loop. Acquisition owns the combined retry budget; respect source Retry-After only within authorized policy. Paused or access-blocked jobs release active transfer capacity while retaining resumable custody. Hashing/writes use governed async/blocking capacity and never hold bookkeeping locks through external code or blocking I/O.

Progress separates logical verified bytes from wire bytes, retries and unknown totals; preserve current UI-visible domain progress. A multi-file byte denominator is known only when every selected size is known and the checked sum is positive; a known-size subtotal is not the whole transfer. Bounded coalesced notifications have snapshot recovery and a reliable terminal/control path. Download 100% does not mean install complete. End-to-end cancellation follows the initiating domain's explicit action while retaining cleanup owners.

Shutdown closes new admission, signals appropriate work, drains tracked async and blocking effects, publishes the true resumable/terminal disposition, then releases custody. Repeated shutdown shares the owned result. An elapsed deadline can produce incomplete shutdown, not fictional cleanup success.

## 11. Package consumption

The package owner supplies a version-checked accepted resolution and trusted immutable artifact set. Acquisition obtains files; package tooling installs them into an owned empty stage from exact local inputs. Preserve original URLs/digests as provenance and validate the installed set afterward. Use only supported public tooling. A report is evidence about a resolution, not by itself a consumable lock format.

The decisive test denies network during the final installation leg and rejects hidden direct-URL retrieval, alternate same-name/version wheels, and missing closure members. Resolver metadata and managed-Python bootstrap traffic remain separately recorded; the claim is exact payload handoff, not interception of every package-tool request. Sources: [pip report](https://pip.pypa.io/en/stable/reference/installation-report/) and [pip install](https://pip.pypa.io/en/stable/cli/pip_install/).

## 12. Compatibility and extension

Internal coordinated DTOs, public Rust/IPC APIs, persisted formats, optional source implementations and consumer install/model records have different evolution obligations. Update actual generated consumers with their producer. New source support within this contract does not change model-adapter registration or runtime installation identity. Unknown schema/protocol versions are explicitly rejected, not decoded into weaker defaults.

Before independently deploying a breaking contract, disposition every supported public client and retained data state. Schema 7 is a breaking retained-store transition: migration is opt-in and offline, schema 4, 5, and pre-receipt 6 remain untouched until that operation is invoked, and old writers must be stopped before publication. The actual deployed root population, retirement of old writers, and operational rollback policy are not established by disposable fixtures; those facts remain required for live-root qualification. Gate status is owned by the acquisition plan, not by a version number in this document. This document remains proposed until the implemented producer/consumer evidence exists.
