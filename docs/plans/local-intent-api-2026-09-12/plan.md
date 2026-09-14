# Plan: Local Intent API and Transport-Independent Domain Language

**Plan status:** `Complete`
**Current phase:** M1–M4 accepted within the recorded local release scope.
**Current acceptance status:** `accepted`
**Next slice:** None in this plan. Distributed capabilities remain deferred until after the next Pumas release.
**Development decision:** `defer-and-implement`:
implement the local API foundations; defer distributed architecture.

## Objective and release scope

Let an application describe the model artifact it needs and let Pumas own
resolution, acquisition, verification, and local desired-state reconciliation.
Define that contract independently of HTTP, JSON-RPC, MCP, or an eventual fleet
protocol. This prepares the API layers for the next Pumas release without
implementing distributed Pumas.

Scope follows the user's 2026-09-12 instruction and sections 1–3 of the
[intent/discovery/distribution brief](../../breif/intent-discovery-distribution.md).
The brief's distributed proposals are architectural context, not requirements
for this plan. The [agent brief](../../breif/agent-interaction-mcp.md) supplies a
future consumer perspective, not authority to implement MCP.

### Included

- A small intent interface in `pumas-core`, accessible without a GUI or runtime.
- Validated model/artifact requirements, resolved identities, availability
  outcomes, and local desired/observed state.
- Deterministic local resolution and acquisition through existing Hugging Face,
  download, import, package-facts, and reconciliation capabilities.
- Durable local `ensure_model` and non-destructive `release_model` semantics.
- Thin projections through the existing same-device IPC client and loopback
  JSON-RPC server. This extends established local transports, not exposure.
- Compatibility evidence for existing operational methods and one native and
  one local process consumer example.

### Deferred until after the next release

Pumas node identities/capabilities, fleets, remote library discovery, machine or
fleet registry redesign, LAN exposure, remote authentication, peer acquisition,
artifact transfer protocols, distributed desired state, host-daemon deployment,
Kubernetes adapters, and renaming/restructuring the `network` domain for those
features. No placeholder node types, remote endpoint fields, fleet registries,
transport negotiation, or generic network adapter framework are needed now.

MCP tools/tasks/subscriptions, shared/bulk discovery caches, and unified inference
gateway development are outside this plan. The gateway's existing
[post-release deferral](../../breif/unified-inference-gateway.md) remains in force.
Existing outbound Hugging Face discovery/downloads stay usable. No new listener,
port, service, deployment mode, runtime loading, or inference routing is added.

## Current code and intended ownership

| Existing owner | Relevant behavior | Planned use |
| --- | --- | --- |
| `pumas-core/src/lib.rs`, `api/builder.rs`, `api/state.rs` | `PumasApi` construction and primary state | Compose one intent module under the existing owning instance |
| `pumas-core/src/api/models.rs` | Indexed `get_model`, search, package facts, execution descriptors, artifact load targets, import, update feed | Reuse operational behavior below the intent interface |
| `pumas-core/src/models/package_facts.rs` | Versioned `PumasModelRef` and artifact/package facts | Reuse identities and evidence; validate before intent decisions |
| `pumas-core/src/api/hf.rs`, `model_library/hf/` | Details, download admission, progress, recovery, shutdown | Own actual acquisition and worker lifecycle |
| `pumas-core/src/api/reconciliation.rs` | Coordinated index reconciliation and watcher integration | Observe and repair through existing authority |
| `pumas-core/src/model_library/download_store.rs` | Durable download state | Remain the download authority, distinct from desired intent |
| `pumas-core/src/ipc/{protocol,local_client}.rs` | Explicit same-device owner connection | Adapt the same intent contract without taking ownership |
| `pumas-rpc/src/{contract,handlers,server}.rs` | Validation, public error projection, JSON-RPC dispatch | Decode, invoke core, project bounded outcomes |

Paths above are relative to `rust/crates/`. Confirm exact call sites at each
milestone; this is a source survey, not a completed behavioral audit.

## Binding design decisions

### 1. One deep intent module

Add a public `intent` module in `pumas-core` and an accessor conceptually shaped
as `api.intent()`. Its methods own selection and orchestration. Keep the current
operational methods available to administrative and existing consumers.

In particular, existing `PumasApi::get_model(&str)` returns an optional indexed
record. Do not change that method into an acquiring call. The intent namespace
makes its stronger side effects and outcome contract explicit.

The domain types depend on domain values, not HTTP status, JSON-RPC envelopes,
SSE events, MCP handles, database rows, or transport connection state. Serde
representations may live with types where appropriate, but deserialization alone
must not bypass semantic validation. No new crate or general command bus is
required unless implementation reveals a present dependency constraint.

### 2. Initial language and interface

Names below are proposed public names; M1 settles concrete Rust signatures and
records them here before publication. Semantics are binding.

| Concept | Meaning and constraints |
| --- | --- |
| `ModelRequirement` | Explicit local model reference or upstream repository selector, optional revision, artifact constraints, acquisition policy |
| `ArtifactRequirement` | Supported format, optional quantization and selected artifact; reject contradictory or unsupported constraints |
| `ArtifactIdentity` | Existing stable model/artifact identity and resolved revision where available; no display-name or absolute-path identity |
| `ModelHandle` | Resolved identity and local artifact access information with verification evidence; availability observation, not a file lease or loaded runtime |
| `DesiredModelState` | Durable local requirement for `Present`, pinned to the resolved immutable revision before acquisition |
| `ObservedModelState` | Explicit missing/resolving/acquiring/verifying/available/blocked/failed state with bounded reason and progress where meaningful |

Reuse `PumasModelRef`, package facts, and existing format/quantization types when
their semantics fit. Do not introduce parallel identity systems. Distinguish an
upstream repository selector from a local indexed model ID. Unknown revision or
integrity evidence stays unknown; never fabricate a hash or claim verification.

| Intent operation | Observable promise |
| --- | --- |
| `query_models` | Read-only local candidates and match evidence; no acquisition or automatic repair |
| `get_model` | Return a verified available handle, accepted pending acquisition, or a typed unsatisfied/failure outcome; do not create durable desired state |
| `get_model_status` | Read a snapshot by stable model/requirement identity; optional operation correlation must not be the only recovery path |
| `ensure_model` | Persist local desired state before acknowledging acceptance, then reconcile toward availability |
| `release_model` | Remove the caller's durable requirement; never imply file deletion, runtime unload, or cancellation of another consumer's work |

Initial selectors are explicit identities and structured artifact constraints.
Open-ended capability ranking, automatic substitutions, and free-text model
recommendation are not promised. Multiple valid candidates return an explicit
ambiguous result; lack of evidence does not mean a match. A provided revision is
honored; an unpinned upstream request resolves to an immutable revision before
mutating. A durable ensure remains pinned until explicitly changed.

Use an explicit acquisition policy: `LocalOnly` or `AllowUpstream`. A local-only
miss performs no upstream calls. Upstream acquisition uses existing provider
credentials and policy; clients cannot submit arbitrary destination paths or
bypass managed storage, gated repository access, or verification.

### M1 concrete native interface

- `PumasApi::intent(&self) -> IntentApi<'_>` borrows the current owner.
- `query_models(&ModelRequirement)` returns typed candidates/match evidence.
- `get_model(&ModelRequirement)` returns typed observed availability or the
  reason the requirement cannot currently be satisfied.
- `get_model_status(&ModelRequirement)` performs a fresh read-only observation
  with the same domain-state meanings.
- `ModelSelector` separates a local `PumasModelRef` from an upstream repository
  selector. `ArtifactRequirement` carries optional format, quantization, and
  selected-artifact constraints; every entrypoint validates them.
- M1 explicitly rejects `AllowUpstream` as unsupported. It registers no ensure
  or release methods. The handle is a local observation, not a runtime lease.
- Indexed readiness alone is insufficient: compare current source evidence
  through the existing fingerprint owner and check current filesystem shape.
  Missing/stale facts return incomplete state without cache writes or repair.

### 3. Availability and lifecycle

`Available` means the selected local artifact satisfies the requirement and the
existing authoritative checks needed for access passed. It does not promise a
loaded model, runtime installation, GPU fit, inference readiness, or perpetual
file availability. Consumers needing a fresh access decision re-resolve it.

Represent invalid requirements, unsupported constraints, ambiguity, not found,
policy refusal, transient unavailability, failed verification, and operation
failure distinctly. Map these outcomes through existing public error machinery
without leaking credentials, raw upstream responses, or internal diagnostics.
A transport timeout must never be interpreted as proof that acquisition failed.

Acquisition is asynchronous when needed. Accepted work belongs to the existing
Pumas lifecycle owner; dropping a request or losing a local connection does not
detach its task or cancel shared work. Correlation IDs can be returned for
observation, but domain identity remains independently queryable.

Deduplicate only after requirements resolve to the same artifact/revision and
compatible policy. Independent requests may join work; conflicting revisions
or policies must not be silently collapsed. Reuse download admission, recovery
tickets, generation checks, importer ownership, and shutdown handling. Never
add an independent downloader or automatically retry mutations with uncertain
outcomes. Retry transient reads within bounded policy; settle/observe admitted
mutations before deciding whether another attempt is safe.

### 4. Local desired state has one durable owner

An ensure declaration belongs to a stable local consumer key and normalized
requirement. The consumer key is application-managed identity, not a security
credential, transport session, or node ID. The same consumer and declaration
is idempotent; different consumers may retain the same resolved artifact.
Release addresses that consumer's declaration and is idempotent. Existing local
owner authorization still governs access; do not imply tenant isolation.

Persist declarations and their resolved targets in the existing library's
SQLite persistence ownership, with explicit schema migration and transaction
boundaries. Reuse the existing index connection/migration mechanism if suitable;
M3 must identify the exact store before editing schema. Desired state does not
copy download progress or redefine indexed artifact truth.

Reconciliation runs while the existing owning Pumas instance is running and
resumes after restart. No always-running daemon is implied. Wake reconciliation
from relevant existing model/download changes, coalesce wakeups, and use bounded
backoff for retryable failures. Startup reconciliation and snapshot reads cover
missed notifications. Invalid or conflicting state remains observable and does
not enter an automatic destructive repair loop.

If a retained artifact disappears, ensure can reacquire it under the recorded
policy. Explicit administrative deletion must report a conflict while retained;
the caller must release declarations first. Release stops future maintenance
for that declaration; it does not delete bytes. Already admitted shared work is
observed to settlement through its existing owner. No new garbage collector or
retention scheduler is included.

Persisted migration failure must preserve original data and report failure;
never rebuild an authoritative library as recovery. Record backup/downgrade
behavior before shipping the schema change. Do not claim restart/crash safety
solely from an in-memory test or orderly shutdown.

### 5. Transport and consumer evolution

Native Rust is the canonical behavior contract. Same-device IPC and loopback
JSON-RPC adapt it. Use namespaced RPC names (proposed `intent_get_model`, etc.)
so existing operational methods retain their meanings. Settle receiver limits,
unknown-field policy, discriminants, numeric ranges, and error mappings before
registering each method. Unsupported future inputs must not silently default.

Keep domain semantic validation reusable; transport framing, request IDs,
authentication, timeouts, and safe error presentation stay in adapters. Polling
status is sufficient initially; no new event protocol is required. Existing
update feeds remain authoritative for their existing scopes.

New Electron/UI and native-language binding surfaces are not required for this
release preparation. Inventory actual consumers and verify that existing ones
still work. If a current consumer needs a new intent method, explicitly add its
whole producer/decoder/generated-contract/consumer path to a bounded milestone;
do not publish a half-supported binding or manually edit generated files.

## Composed-design review

**Applicability:** `applicable`; the intent module adds a public interface,
local desired-state persistence, and existing-transport adapters.

1. **Independent concerns:** Core owns requirement matching and orchestration on
   demand in the owning library; the existing download module owns byte transfer
   and worker completion; the desired-state store owns declarations across
   restarts; adapters own decoding/projection per request. Each changes for its
   own policy, lifecycle, persistence, or transport reason.
2. **Interleavings:** Identity resolution must precede deduplicated admission;
   durable declaration commit must precede ensure acknowledgement; readiness
   follows verification/index publication. Transport sessions and runtime load
   state must not enter those decisions.
3. **Required knowledge:** Callers know requirements, acquisition permission,
   stable consumer identity for ensures, and typed outcomes. They do not know
   search/download/import ordering. The composition root wires existing owners
   and drains owned work; it does not become a resolver. Peer modules see
   existing operational contracts and scoped state-change notifications.
4. **Change paths:** Changing artifact selection touches core matching/tests;
   changing JSON framing touches its adapter/tests; changing download recovery
   touches the existing download owner and intent integration evidence; changing
   desired-state schema touches its store/migration and restart evidence. None
   should force application orchestration changes.
5. **Dependencies:** Requirements and resolved identities carry stable values.
   Download admission and reconciliation carry real lifecycle obligations,
   which stay inside intent implementation. No raw DB rows, worker generations,
   recovery-token choreography, or transport versions escape to intent callers.
6. **Independent evolution/failure:** Adapters can be tested/replaced without
   changing core policy. Upstream failure produces a typed core outcome while
   local resolution remains usable. Desired-state storage failure blocks ensure
   acknowledgement without fabricating download success.
7. **Deletion tests:** Removing intent would push selection and acquisition
   ordering back into callers. Removing the desired-state store would lose the
   restart promise. Removing a local adapter would remove an actual local client
   path. A new registry, command bus, generator, peer adapter, or fleet schema
   has no current justification and is declined.
8. **Retained complexity:** One intent module plus local desired declarations
   and thin local adapters. Necessary matching, concurrency, persistence, and
   recovery complexity stays with its owner; existing downloader, index,
   reconciliation, and public-error machinery are reused.

Reassess composition if implementation requires callers to orchestrate retries,
import, recovery, or transport-dependent state transitions.

## Milestones

M1, M2a, M2, M3a, M3b, and M4 are `Accepted`. Full M3 is accepted within its recorded local evidence. Execute in dependency order. Write sets below are
allowed scopes, not permission for unrelated refactors; resolve new exact files
within those scopes when admitting a slice. Each milestone may update this
plan, its ledger, and issues. Changes outside a listed scope require an explicit
write-set update before editing.

### M1 — Native local resolution

**Status:** `Accepted` within the native Linux/file-artifact evidence below.

**Goal:** An embedding application can resolve a structured requirement to an
existing verified artifact through `api.intent()`, with no network side effects.

**Write set:** `rust/crates/pumas-core/src/intent/` (new),
`rust/crates/pumas-core/src/{lib.rs,api/mod.rs,api/models.rs,models/mod.rs}`,
`rust/crates/pumas-core/tests/intent_api_tests.rs` (new),
`rust/crates/pumas-core/examples/intent_model.rs` (new),
`rust/crates/pumas-core/src/model_library/{library.rs,package_facts/context.rs}`,
`rust/crates/pumas-core/README.md`.

The internal library/context scope is admitted only for a read-only adapter to
existing package-facts freshness checks; it must not duplicate fingerprint
algorithms, reconcile, or write caches. README scope documents the actual native
contract in the same slice.

Settle concrete signatures, reuse existing identity/facts producers, and deliver
query/get/status with `LocalOnly`. Keep unsupported acquisition unreachable or
explicitly unsupported until M2. No schema or transport changes in this slice.

**Gate:** Native integration tests cover exact match, missing artifact,
ambiguous candidates, revision/format mismatch, invalid reference, incomplete
facts, and a filesystem change after indexing. Existing indexed `get_model`
retains its lookup semantics. Example consumes the real interface without
search/download/import choreography. Supporting Rust checks pass.

**Re-plan if:** Existing identity or artifact evidence cannot represent a correct
local match without changing a published contract.

### M2a — Immutable-revision acquisition prerequisite

**Status:** `Accepted`; implementation and controlled Linux verification are in
the [M2a report](reports/immutable-revision-implementation.md). Production intent
acquisition and broader recovery/crash guarantees remain M2 work.

**Depends on:** M1.

**Goal:** Existing managed acquisition can retain one resolved immutable commit
through metadata, bytes, identity, import, and persisted resume. Preserve the
public operational `DownloadRequest` contract and its default-main behavior.

**Write set:** `rust/crates/pumas-core/src/model_library/` files
`artifact_identity.rs`, `download_store.rs`, `importer.rs`,
`download_recovery.rs`, `hf/metadata.rs`, `hf/types.rs`, `hf/download.rs`,
`hf/lifecycle.rs`, `hf/bundles.rs`, `hf/mod.rs`, and
`rust/crates/pumas-core/src/api/hf.rs`. Tests belong with those owners initially;
README and this plan's records document the actual compatibility result.
`src/tests.rs` is admitted only for the persisted-record fixture constructor.
`model_library/mod.rs` is admitted for crate-private revision helper visibility.

Use a crate-private validated revision context and pinned managed entrypoint.
Persist the commit through the existing store owner, with explicit v4-to-v5
migration and old-reader refusal. Record the public `PersistedDownload`
constructor compatibility effect before changing that lower-level type.
Identity/destination must distinguish commits for every artifact selection
kind; changing a hash that the destination never uses is insufficient.
Ticket recovery must preserve the pin or explicitly refuse pinned recovery;
never reconstruct it as main. This does not authorize a recovery lifecycle
rewrite or claim full crash recovery.

**Gate:** Controlled moving-main metadata/auxiliary/payload tests; equal-pin
coalescing and distinct-pin isolation; persisted resume/import identity;
malformed/pre-publication migration failure preservation, explicit uncertain
publication refusal, and downgrade refusal; existing default-main,
caller-drop, importer, and shutdown regressions. Exact evidence and source
findings are in [the prerequisite report](reports/acquisition-prerequisites.md).

**Re-plan if:** Pinning requires changing public request/binding semantics,
replacing store authority, or expanding unresolved recovery lifecycle work.

### M2 — Managed acquisition behind the intent interface

**Status:** `Accepted` for the bounded native scope; see the [M2 report](reports/managed-intent-acquisition.md).

**Depends on:** M2a and verified lifecycle prerequisites recorded in I2.

**Goal:** An allowed upstream miss becomes one owned acquisition and eventually
a verified local handle, without caller-managed download/import steps.

**Write set:** M1 scopes plus `rust/crates/pumas-core/src/api/{hf.rs,state.rs}`,
`rust/crates/pumas-core/src/model_library/hf/`,
`rust/crates/pumas-core/src/intent/acquisition_tests.rs` (new).
`rust/crates/pumas-core/examples/intent_acquire.rs` demonstrates native acquisition
and supplies a reproducible real-upstream verification path.
The source audit additionally admits `model_library/download_recovery.rs` only
for capability-bound byte verification, and `model_library/importer.rs` only
for canonical facts generation during pinned finalization before completion.
`model_library/mod.rs` is admitted only for crate-private observation visibility.
`api/mod.rs` is admitted only for a test-only native API fixture re-export.

The existing transfer path trusts some recorded hashes and existing files.
Before exposing acquired availability, add owned pinned byte verification:
rehydrate exact admitted filenames from their immutable commit tree on each
execution/resume, verify available LFS SHA-256 and size through held filesystem
authority before promotion/import, and fail closed on missing evidence or
unavailable required metadata. Regular auxiliary files without hash evidence
remain explicitly unverified; do not fabricate cryptographic assurance.
Reuse existing persistence and task ownership; no new schema or recovery owner.

Pending outcomes carry a pinned local requirement as stable correlation plus
optional operational progress. Query/status remain read-only; unresolved durable
custody must not be interpreted as absence or authorize another writer. Pinned
owned import produces canonical package facts before completion, so status can
observe availability without doing repair work.

Resolve explicit upstream selectors to immutable revisions/artifacts, reuse
existing admission/import/verification, and expose pending/status outcomes.
Do not rewrite recovery/storage machinery under this milestone. If a required
fix falls outside this scope, coordinate it with its existing plan owner.

**Gate:** Real core integration with an isolated controlled upstream exercises
miss → acquire → verify/index → available, concurrent same-target requests,
different revisions, requester disconnect/drop, gated/unavailable upstream,
failed integrity, resumed partial acquisition, and shutdown settlement. Prove
`LocalOnly` causes zero upstream requests. A representative real Hugging Face
acquisition validates the external path separately from fixtures.

**Re-plan if:** Safe acquisition depends on unresolved recovery/import ownership
or the actual downloader cannot honor immutable revision selection.

### M3a — Authoritative declaration-store prerequisite

**Status:** `Accepted` for the private storage boundary; see the [M3a report](reports/m3a-declaration-store.md).

M3 source inspection found cache-oriented SQLite settings, unversioned additive
schema initialization, cancellation-only runtime task shutdown, and unowned
filesystem deletion after index removal. These do not establish the durable
ensure contract. Split the authoritative persistence prerequisite from public
operation and lifecycle integration; M3a alone does not accept full M3.

**Write set:** `rust/crates/pumas-core/src/index/model_index.rs`, a new
`index/model_index/intent_declarations.rs`, and its nested `intent_declarations/tests.rs`.
`intent/{resolver.rs,mod.rs}` are admitted only to share the existing quantization
normalization helper with canonical declaration encoding (no behavior change).
The core README documents this private migration and its compatibility boundary.
The [store design](reports/m3-store-design.md) records the admitted migration,
key, transaction, and downgrade contract before schema implementation. The
[lifecycle design](reports/m3-lifecycle-design.md) and [deletion inventory](reports/m3-deletion-inventory.md)
retain M3b integration obligations. Existing library/index ownership remains singular.

**Goal:** A crate-private transactional declaration store retains normalized
consumer/declaration identity and immutable acquisition recipe independently of
indexed availability. Atomic retention and deletion-claim operations prevent
prospective deletion/ensure races at the persistence boundary. No public ensure,
release, scheduler, filesystem deletion rewrite, or transport is exposed here.

**Gate:** Fresh/upgrade schema, failed migration preservation, future/malformed
schema refusal, idempotency and conflicting declarations, two-consumer retention,
release isolation, competing deletion claims/ensure transactions, reopen after
forced process exit around commit, and survival of index clearing. Precise Linux
process-crash limits and unsupported historical downgrade behavior are explicit.
This gate does not accept M3's filesystem/admission/reconciliation crash points.

### M3b — Durable local desired state integration (M3 acceptance)

**Depends on:** M2 and M3a.

**Goal:** Ensure survives owner restart, restores a missing retained artifact,
and release removes only the relevant declaration.

**Write set:** `rust/crates/pumas-core/src/intent/`,
`rust/crates/pumas-core/src/index/`,
`rust/crates/pumas-core/src/api/{state.rs,builder.rs,reconciliation.rs,runtime_tasks.rs,hf.rs,mod.rs,models.rs,links.rs,migration.rs}`,
`rust/crates/pumas-core/src/model_library/{mod.rs,library.rs,merge.rs,mutation_authority.rs,download_recovery.rs,partial_download.rs,importer.rs,library/migration.rs,hf/download.rs,hf/lifecycle.rs,hf/types.rs}`,
`rust/crates/pumas-core/tests/intent_desired_state_tests.rs` and
`rust/crates/pumas-core/tests/intent_desired_crash_tests.rs` (new), and
`rust/crates/pumas-core/src/lib.rs` for explicit owned shutdown.

M3b admits the owner and deletion contracts in `reports/m3b-owner-contract.md`
and `reports/m3b-deletion-contract.md`: finite effects drain independently of
requesters; primary composition shares its existing download persistence even
when HF is disabled. Standalone libraries without configured mutation authority
refuse destructive operations; path-only destructive merge is consequently
unsupported until its source is explicitly composed. Deletion claims do not
expire or disappear on a failed partial operation. Pinned planning must compute
the actual canonical destination before binding and before artifact mutation.
Initial durable selectors are local references with LocalOnly and repositories
with AllowUpstream; other combinations return Unsupported. Retries reuse the
existing reconciliation coordinator and bounded backoff, never a new scheduler.

Before migration implementation, record exact schema/store ownership, normalized
key semantics, transaction/admission ordering, deletion guard call sites,
shutdown wiring, retry limits, and downgrade behavior. Add only the persistence
and lifecycle hooks required for local declarations.

**Gate:** Fresh/upgrade DB fixtures, failed migration preservation, duplicate
ensure, two-consumer retention/release, deletion conflict, unavailable upstream,
missing artifact reconciliation, orderly restart, and forced process exit at
commit/admission/publication points. Reopen real on-disk state and prove no
acknowledged declaration disappears, no duplicate artifact writer is admitted,
and no false available state is returned. Document supported filesystem/platform
evidence limits rather than generalizing a Linux-only result.

**Re-plan if:** A second library owner, destructive migration, independent task
scheduler, or remote identity becomes necessary.

### M4 — Existing local transports and consumer evidence

**Depends on:** M3b.

**Goal:** Native and local process callers observe equivalent intent semantics.

**Write set:** `rust/crates/pumas-core/src/ipc/`,
`rust/crates/pumas-core/tests/intent_ipc_tests.rs` (new),
`rust/crates/pumas-core/src/api/state.rs` for authenticated primary dispatch,
`rust/crates/pumas-core/src/api/reconciliation.rs` for expected root-contention deferral,
`rust/crates/pumas-rpc/src/{contract.rs,handlers/mod.rs,server.rs}`,
`rust/crates/pumas-rpc/src/handlers/intent.rs` (new),
`rust/crates/pumas-rpc/tests/intent_integration_tests.rs` (new),
`rust/crates/pumas-rpc/tests/integration_tests.rs` for persisted fixture compatibility,
`rust/crates/pumas-rpc/Cargo.toml` only for test dependencies if needed,
`rust/crates/pumas-core/examples/intent_local_client.rs` (new),
`rust/crates/pumas-core/README.md`, `rust/crates/pumas-rpc/README.md`,
`docs/ARCHITECTURE.md`, `docs/README.md`.

Add receiver validation and explicit outcome projection together with each
registered operation. Inventory existing native/binding/desktop consumers;
retain their operational contracts. Document side effects, pending outcomes,
release semantics, upgrade requirements, and deferred capabilities.

**Gate:** Real owner/client process evidence through both same-device IPC and
loopback JSON-RPC: resolve, pending acquisition, status, ensure, restart, release,
malformed/unsupported input, connection loss, and safe errors. Compare outcomes
with native fixtures. Existing RPC and operational API regression suites pass;
no network exposure or GUI/runtime dependency is introduced. Verify library-only
RPC build as well as the normal build.

**Re-plan if:** An existing consumer must migrate, generated schemas must change,
or exposure/authentication policy would expand. Amend the precise consumer write
set and corresponding evidence before proceeding.

## Objective acceptance claims

Every claim below is blocking for this plan's acceptance. A1–A7 are satisfied within
the recorded native/local evidence and explicit compatibility limits. Implementation
evidence is recorded in the [ledger](execution-ledger.md). Static
checks support these claims but do not substitute for behavior evidence.

| ID | Observable criterion | Kind | Environment | Mode | Status |
| --- | --- | --- | --- | --- | --- |
| A1 | Native requirement resolves a correct local artifact; misses, ambiguity, invalid/incomplete evidence remain distinct; local-only performs no upstream work | integration | simulated isolated library/files | automated | satisfied — M1 ledger |
| A2 | Allowed upstream acquisition reaches verified indexed availability with one artifact writer and owned cancellation/shutdown behavior | integration | simulated controlled upstream + real core persistence/workers | automated | satisfied — M2 report |
| A3 | A small real HF artifact can be acquired by requirement and resolved locally afterward | integration | required-real HF access, isolated root, sufficient disk | either | satisfied — M2 report |
| A4 | Committed local ensures survive restart/forced exit, repair missing availability safely, and release respects other consumers | system | representative processes and on-disk SQLite/filesystem | automated | satisfied — M3b report |
| A5 | Existing IPC and loopback JSON-RPC preserve native semantics and reject invalid inputs without leaking internals | contract + system | representative local owner/client processes | automated | satisfied — M4 report |
| A6 | Existing operational lookup/acquisition consumers retain documented behavior; native/RPC paths work without GUI or inference plugins | integration + contract | representative supported development host | automated | satisfied — M4 report |
| A7 | Public examples and final diff show callers only specify intent; no new node/fleet/networking or gateway implementation is introduced | contract/design review | not-applicable | manual | satisfied — M4 report |

Use targeted Cargo suites named above plus affected formatting/lint checks,
then the existing relevant API/RPC regression suites. Build with
`cargo build --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features`
for the library-only check. Derive final feature/platform matrix from supported
build contracts; do not claim all platforms from one host. Real upstream access
is required for A3 and the M4 pending/disconnect process gates; unavailability
does not stop independent local work but prevents marking the whole plan accepted.

## Coordination, blockers, and re-plan triggers

M1 is accepted. Its evidence covers coherent existing local file targets,
including Safetensors and header-inspected GGUF, and explicit non-ready states.
It does not claim inference/tensor validation or that every existing package
layout supplies a coherent load target. I8 records the existing HF directory
layout inconsistency, which safely returns `Incomplete`.

The existing
[current-standards remediation program](../current-standards-remediation-2026-09-03/plan.md)
retains authority over its in-progress download recovery/import lifecycle work.
It explicitly records incomplete recovery and hard-crash evidence. M2/M3 must
identify which guarantees they need and verify those checkpoints before relying
on them; this plan neither completes nor replaces that work. See [issues](issues.md).

The immutable revision re-plan trigger was reached during source inspection.
M2a completed the bounded prerequisite. M2 has now verified native orchestration,
ordinary persisted resume, owned shutdown, verified indexed availability, and one
real upstream acquisition. Broader hard-crash recovery remains unaccepted in I2;
M3b established its bounded durable-state guarantees with the recorded process-exit evidence.

No separate worktree or branch is required by this plan. Preserve unrelated
working-tree changes and coordinate serial edits to shared source/plan owners.

Re-plan if identity semantics conflict with real consumers, safe acquisition or
persistence cannot meet the contract, the scope needs new remote capabilities,
or the release needs a smaller deliverable. A release cut may ship accepted
milestones, but must omit unavailable operations and explicitly defer remaining
milestones; never relabel lookup-only behavior as completed `ensure_model`.

## Execution records and completion

- [Execution ledger](execution-ledger.md)
- [Issues and dispositions](issues.md)
- Detailed evidence belongs in `reports/` when produced; no report is required
  merely because that directory exists.
- No new ADR is created during planning. Accepted durable behavior will update
  the existing architecture/core/RPC guides; create an ADR only for a decision
  that needs a separate durable rationale.

The user authorized implementation of this identified plan on 2026-09-12.
Their request to continue implementation starts the previously planned work;
M1, M2a, M2, M3a, M3b, and M4 are accepted; this local implementation plan is complete. The canonical plan path is
`docs/plans/local-intent-api-2026-09-12/plan.md`. Subsequent invocations use
`continue` while active. Final transport and consumer evidence is recorded in
[the M4 report](reports/m4-local-transports.md).
