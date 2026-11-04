# Plan: source-neutral artifact acquisition

**Plan status:** `Verifying`. Q1 and Q3 source work is merged and ships in
0.8.0; their objective gates (AQ-HTTP, AQ-S3) are not accepted. Q2 and Q4 are
`Planned` and have not started. No slice is currently being implemented.
**Objective acceptance status:** `pending`.
**Current phase:** acceptance evidence for AQ-HTTP, then admission of Q2.
**Exactly one next slice:** AQ-HTTP acceptance on current `main` — rerun the
pending AC01–AC10, AC15, AC16 and AC18 claims against the shipped source and
record the deployed retained-state population (schema 6/7, old writers,
rollback) before any live-root mutation. Q2 and runtime R1 stay gated on it.

**Status review (2026-11-04, v0.8.0):** The Q1 shared HTTP lifecycle, neutral
persistence/handoff and HF/llama.cpp consumer cutover merged through PR #34
(`232698f1`), with follow-ups in PRs #39 and #43 (HTTP elapsed-time budget).
Q3 S3 reader, explicit/conditional manifests, authored bundles, desktop/RPC
workflows and installed-process fixtures merged through PRs #38, #42 and #52
and the 2026-10-09 v0.8 integrations (PRs #59, #60, #61 and #68). The shipping
implementation is `rust/crates/pumas-core/src/acquisition/`; behavior is owned by
the [artifact acquisition contract](../../contracts/artifact-acquisition.md).
No plan record changed between 2026-10-09 and this review, and v0.8.0
qualification does not depend on these gates. Q2 has no source yet: the Torch installer does not
consume the acquisition owner. Evidence bound to pre-merge branch heads is
historical; acceptance needs fresh exact-head evidence. Dated slice reports,
command-log archives and generated evidence for merged slices were removed in
this review and remain in Git history
(`git log -- docs/plans/artifact-acquisition/reports`).

**Canonical plan path:** `docs/plans/artifact-acquisition/plan.md`.
**Owner:** Pumas acquisition integration. The repository owner assigns the
implementation and integration roles when admitting source work.
**Operation:** `verify` this plan for AQ-HTTP; `start` Q2 only after AQ-HTTP is
accepted. Work starts from current `main`; the earlier `work/acquisition-q1-http`
and `work/artifact-acquisition-runtime-plan` branches are no longer the
integration path.

## Objective and scope

Provide one reusable acquisition owner that obtains authorized, specifically identified artifacts from HTTP/Hugging Face and S3-compatible object stores, preserves resumability and recovery guarantees, verifies the required content evidence, and supplies ordinary local files with an owned lifetime. Model import and executable/package installation consume that result without implementing their own byte-transfer lifecycle.

The initial production consumers are the existing Hugging Face model workflow, the existing llama.cpp archive installer, and the existing Torch package-installation path. They establish the interface before the later runtime-installation and arbitrary-model-adapter changes depend on it.

The complete acquisition plan includes HTTP, the current Hugging Face resolution path, exact package-file acquisition, and S3-compatible object retrieval. It is more than a URL download utility. Full acceptance requires the real source/consumer, restart, desktop, and affected platform evidence named below. The first accepted gate can unblock runtime work before the complete acquisition roadmap is accepted.

Source intent: `docs/breif/s3-model-fetch.md` and `docs/breif/intent-discovery-distribution.md`. They remain explanatory intent; this plan owns current execution sequencing. Existing model intent APIs remain the normal model-facing interface. Runtime archives and wheels do not become models to reuse them.

### Included boundaries

Source resolution and immutable file manifests; source readers; range/stream transfer; governed retries and capacity; source authorization refresh; attempt persistence and restart; contained destination effects; integrity evidence; ordinary-file handoff and retention; model/native/package consumer integrations; relevant RPC/UI projections and source-configuration workflows; supported retained-state evolution.

### Preserved constraints

Pumas core remains independently usable. Acquisition starts without Python, Torch, model-adapter imports, inference plugins, or a populated model index. A storage root and acquisition metadata may be needed; a model database is not. Reuse existing HTTP, hashing, capability-filesystem, task-custody and atomic-publication mechanisms where their actual invariants fit.

Preserve model selection/revision, partial-model visibility, awaited importer completion, current root/destination exclusion and explicit refusal of unaccepted Pending cleanup replay. Preserve native build selection, wheel/source trust, staged installation, process ownership and existing generation lifetimes at their separate owners.

Source resolution never grants execution permission. Acquisition readiness never means imported, installed, loaded or runnable. Credentials and signed URLs never define content identity. Normal files remain the consumer boundary without a mandatory second full copy of model weights.

### Explicit exclusions and continuation

Xet-native reconstruction, peer discovery, source racing, cross-origin partial-byte failover, chunk CAS, deduplicating concurrent transfers, a virtual filesystem, remote/multi-tenant service deployment, write access to remote stores, a universal package solver, arbitrary scripts, and new model execution tasks are excluded from the initial implementation. They are explicitly deferred with owner and triggers in [issues](issues.md), not represented as stub production capabilities. Existing HF retrieval continues through its supported file path; a source requiring an unimplemented reconstruction protocol returns a truthful unsupported/unavailable result, not fabricated complete coverage.

Source-neutrality is achieved by a real shared contract and independently replaceable readers, not by promising every storage provider. S3-compatible endpoints are configuration variants of the same integration where their tested protocol permits it.

## Binding decisions and owners

### AD1 — one domain boundary for acquired files

The proposed canonical interface is [Artifact acquisition contract](../../contracts/artifact-acquisition.md). Acquisition owns that contract; the companion runtime plan references it. The contract separates an immutable selected file set from refreshable retrieval authorization, durable acquisition ownership from in-memory task generations, and verified files from consumer publication.

Place the source-neutral implementation in `pumas-core::acquisition` by default. App-manager already depends on core. The acquisition module has no dependency on app-manager, a model importer, Torch, or the desktop. The existing composition root creates and drains one acquisition owner for its scope; embedding can construct that owner directly with supported storage/network capabilities. No additional process, global singleton, scheduler, or crate is justified by current facts.

### AD2 — independent owners, not a universal job engine

| Concern | Canonical owner | Result promised |
| --- | --- | --- |
| Model/release/package selection | Existing model or runtime/package integration | Exact selected source revision/build/dependency artifacts |
| Source resolution | HF, HTTP or S3 resolver implementation | Complete artifact manifest and resolvable source references |
| Protocol access | Source reader and established HTTP/S3 libraries | Correct conditional full/range bytes or typed source failure |
| Transfer, custody and byte verification | Shared acquisition service | Verified ordinary files under a live/durable handoff contract |
| Model validation/import/index | Existing library consumer | Model publication, only after its own finalization |
| Archive/package installation | App-manager/package consumer | Validated installed unit, only after its own publication |
| Adapter loading and inference | Companion runtime/adapter plan | Compatible bound execution, not an acquisition responsibility |

The contract's source reader is a bounded I/O capability; it does not carry model classification, install hooks, ABI selection, inference logic, or a duplicate retry/store manager. Packaging authentication and protocol signing remain with maintained tooling, not a home-grown signing engine.

### AD3 — bootstrap-safe and incrementally extracted

Q1 reuses and extracts the existing HF lifecycle rather than renaming the entire HF client into a generic manager. Source-specific resolution, model-destination authorization and importer settlement remain on the model side. Generalized attempt custody, persistence and byte effects become one acquisition owner.

Current model-specific fields and root UUIDs cannot become mandatory runtime-artifact fields. Durable state has one authority per fact: acquisition state in the acquisition owner; model/installation publication at their consumers. A legacy adapter can read the retained supported format at an explicit migration boundary, but it cannot run a parallel transfer writer for the same attempt.

Q1's small native-installer bridge changes only selected acquisition calls and their progress/handoff integration. It uses the currently supported tag-based installer until runtime R1; it does not implement installation-ID migration early. That proves a real consumer without depending on the downstream refactor.

### AD4 — normal files and evidence-scoped reuse

Use consumer-approved capability-backed staging roots, with transfer mutation grants and sealed read handoffs. Models may stage on their destination filesystem so publication can preserve ordinary paths without always duplicating the full artifact. Native archives/wheels can use owned staging or verified retained input cache. Shared raw inputs remain immutable; mutable install outputs cannot be writable hardlinks to them.

A borrowed path is valid only while its associated use custody is retained. Child processes reading archives or wheels retain that custody through actual exit/cleanup. Restart uses durable consumer demand and verified file/workspace identity, not resurrection of a serialized OS handle or trust in a displayed path. No expiry timer alone authorizes deletion.

Q1 does not require shared transfer coalescing, content-addressed filenames, or an LRU service. It does require safe release/reclamation of its owned staging and explicit retained-input policy. Identical complete bytes may be reused only with matching content evidence and the receiving consumer's authorization. Size/filename equality alone is insufficient.

### AD5 — HTTP and object stores must preserve representation identity

HTTP whole-file and range access implement the specific contract in the canonical document. Resume is conditional on stable source/representation evidence, and final verification satisfies the consumer's required evidence. A server returning a full body to a range request cannot have that body appended at an old offset. Source refresh cannot silently choose a new object or mix revisions.

S3 resolves endpoint/bucket/key plus an immutable version or sufficiently strong content evidence. ETags are conditional validators, not universally cryptographic digests. An S3 prefix listing is not an atomic multi-file snapshot: either use an explicit manifest or pin every listed object and report incomplete/racing enumeration rather than asserting a coherent package. Remote object keys and local logical paths have separate validation contracts.

Reuse current reqwest-based HTTP facilities. Evaluate the maintained Rust `object_store` S3 implementation as the first candidate for Q3's needed endpoint/credential/version/range semantics; its API is not the public Pumas contract. The bounded dependency decision and rejection conditions are in [design review](reports/architecture-review.md). Exact version, feature set, license compatibility and supported target evidence must be recorded before adding a dependency. A failed candidate requires a named alternative decision, not custom S3 protocol code by convenience.

### AD6 — scoped authorization and bounded operational policy

The authorized source configuration specifies endpoints, trust, permitted redirects and credential references. Keep credentials request-scoped and out of durable artifact identity, progress and public diagnostics. Support explicit private/local object-store endpoints; do not impose a blanket private-network ban that makes legitimate configured stores unusable. Conversely, arbitrary model metadata cannot grant access to local services or cloud metadata endpoints.

The acquisition owner controls overall attempts, cancellation, backoff and resumption. Integrate or disable nested SDK retries so budgets do not multiply invisibly. Every task, file descriptor, network request, buffer, progress queue and blocking hash/write job has governed capacity. Reuse measured current settings where semantically compatible; declare units and saturation outcomes. Progress can coalesce, but terminal/control information cannot disappear. Time budgets may interrupt authorized I/O, not manufacture proof that file effects stopped.

### AD7 — package semantics stay outside the fetcher

Q2 consumes the exact accepted Torch/wheel artifact closure from the existing package owner. The resolver can use supported metadata/candidate access; the final acquisition-to-installation leg uses verified local inputs and is tested with network denied. Preserve original resolution provenance separately from local installation paths. Do not treat a pip report as a lockfile, rewrite a private pip downloader, or assert that all pip/managed-Python network traffic is shared.

Runtime R2 adds arbitrary registered adapters later using that same accepted file-set handoff. Q2 does not depend on R2's adapter catalog. Bootstrap/provider-managed downloads receive a written migrate/retain-with-scope disposition; any retained mechanism has a distinct standard-tool responsibility, not a second Pumas payload downloader.

### AD8 — own evolution before mutation

The existing download schema, recovery admissions, root markers, hidden history and cleanup custody have retained consumers. The Q1 candidate extends the existing canonical store through schema 7; it does not reuse schema 5 with an expanded shape or add a second writer. Schema 7 includes the source-neutral acquisition fields and the versioned consumer-receipt partition. Receipt-free legacy schema 4/5/6 conversion is explicit and offline; normal open/admission fails closed without modifying those formats. Every old reader/writer must be stopped before conversion. Disposable fixture migration is implementation evidence, not proof that the deployed root population or rollback policy is qualified. Preserve legacy IDs through explicit mapping where needed for existing UI/consumer operations; unsupported, corrupt or custody-unresolved states remain visible and non-authorizing.

Reopen after each interrupted publication boundary to source, destination or an explicit uncertainty state. Do not mark completion, delete partials, auto-retry imports/installations, or replay Pending cleanup from guessed facts. New readers reject unknown future formats. Older writers must be retired or isolated; a new format number cannot constrain an old binary. Rollback requires evidence about both stores and subsequent authored changes, not just a backup file.

## Milestones and release gates

Each row is one coherent semantic unit including producer and actual consumers. [Write sets](reports/write-sets.md) are part of the row's allowed paths; each implementation admission binds the exact changed files and tests before editing.

| Milestone | Goal | Dependencies | Gate / evidence | State |
| --- | --- | --- | --- | --- |
| **Q1** | Shared HTTP lifecycle, neutral persistence/handoff, and real HF/native consumer cutover; existing UI outcomes preserved. | Source/retained-state preparation and active recovery-owner handoff. No runtime milestone. | `AQ-HTTP`: AC01–AC10, AC15, AC16, AC18; real HF model import and native archive extraction plus corresponding UI/contract/cancellation/reopen evidence. | Implemented (merged, ships in 0.8.0); AQ-HTTP not accepted |
| **Q2** | Exact wheel-file-set acquisition and local-only consumption in the existing Torch installer. | AQ-HTTP. | `AQ-PACKAGES`: AC11, AC12 and Q1 regression evidence affected by this composition; network-denied installation and actual package identity. | Planned; not started |
| **Q3** | S3-compatible acquisition using the same lifecycle, direct explicit source workflow, tested credentials/version semantics, and a real model import through the existing model-facing operations. | AQ-HTTP. (Implemented ahead of Q2 by delegated slices.) | `AQ-S3`: AC13, AC14 and source-neutrality regressions; native AWS S3, one non-AWS compatible service, local MinIO, and S3-to-model-library evidence. | Implemented against controlled HTTPS fixtures (ships behind the `s3` feature in 0.8.0); live AWS/non-AWS/MinIO evidence absent; AQ-S3 not accepted |
| **Q4** | Complete affected public/installed/native qualification, migration documentation and removal of superseded authority. | Q1–Q3 implemented. | `AQ-COMPLETE`: all AC01–AC18 satisfied and all four milestones Accepted; packaged/independent-consumer/native-platform evidence in AC17. | Planned |

[Dependency gates](reports/dependency-gates.md) is the single gate-status and consumer-handoff record. Every gate is currently **not ready**. A gate opens only after its claims pass for identified material source, supported API and stated target scope; availability is not inferred from a commit label or partial implementation. Material incompatible changes require targeted re-verification of affected gates.

Runtime **R1 requires AQ-HTTP**. Runtime **R2 requires AQ-PACKAGES** as well as R1. Runtime use of S3 requires AQ-S3 on the relevant target. S3, Xet, peers or chunk CAS are not secretly prerequisites of HTTP runtime management. The runtime plan does not implement or accept these gates itself. Both plans share one serial integration owner for overlapping files.


## Acceptance and verification

[Acceptance matrix](reports/acceptance-matrix.md) owns criteria, evidence kind, environment, execution mode and named procedures. Every production claim remains pending. Static checks and disposable fixtures support, but do not replace, real transfer, package, desktop, native and deployment evidence. Source review and this package's link checks do not certify code compliance.

Composed-design review is **applicable**. [Architecture review](reports/architecture-review.md) answers all eight probes for this design and its runtime consumer, plus the authority-scope and dependency questions. The required simplicity result is reduced caller knowledge: a new source changes a source integration, not runtime installers or model adapters; a new installer consumes existing verified artifacts without learning HTTP/S3 recovery.

Independent read-only review covers durable migration, credential/destination authority, cancellation/retention and final producer/consumer composition. Reviewers never edit; narrow verification reuses the reviewer. Shared contracts, durable formats, generators, lockfiles and current plan state have one writer. Disjoint implementation roles and escalation rules are in the write-set report. No fixed commit count or exact-parent topology is prescribed.

## Blockers, re-plan triggers and limits

No source blocker. AQ-HTTP is blocked on evidence, not code: the deployed retained-state population (schema 6/7 libraries, overlapping old writers, rollback) is uninventoried, and AC10 representative resource bounds, AC16 public-client compatibility and AC17 platform/deployment qualification have no current-head evidence. Schema-7 conversion stays explicit and offline ([library upgrade](../../library-upgrade.md)); Pending cleanup remains refused. AQ-S3 is blocked on live AWS S3, one non-AWS compatible service and local MinIO environments, which have not been supplied ([provider record](reports/s3-composed-provider-qualification-2026-10-05.md)). Missing native/GPU/GUI/network credentials block only the claims that require them.

Re-plan when a new consumer changes authority, the proposed state store cannot preserve supported recovery, a file lease cannot survive required worker cleanup, a source cannot establish required identity, an S3 dependency cannot provide the selected API/target, or downstream semantics leak into acquisition. Adding a file within an already admitted owner only amends its concrete write set.

Authorized planning does not authorize deleting user files, publishing releases, migrating a live root, changing external repositories, or rewriting history. Prototype and real-source tests use disposable roots and approved test resources. No new production timeout/failure guarantee is inferred from test harness deadlines.

## Linked records and terminal documentation

[Ledger](execution-ledger.md) · [Issues](issues.md) · [Audit](reports/codebase-audit.md) · [Architecture](reports/architecture-review.md) · [Acceptance](reports/acceptance-matrix.md) · [Write sets](reports/write-sets.md) · [Dependency gates](reports/dependency-gates.md) · [Standards/source review](reports/standards-and-sources.md) · [Companion runtime plan](../runtime-installations-and-model-adapters/plan.md).

Do not create another master execution plan. The shared contract owns implemented behavior; update it in the same slices. At acceptance, move durable decisions to current owner documentation/ADR as required, and follow Pumas's documented terminal-plan lifecycle rather than retain duplicate active instructions. Git and the delivered older packages preserve history.
