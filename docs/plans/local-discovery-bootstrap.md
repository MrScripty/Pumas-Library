# Local discovery and bootstrap — v0.8 incremental slices

Initial design base: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`. Scope is local
application discovery, not a LAN/fleet daemon. This applies the intent/distribution
and capability-discovery briefs and the namespace-instance-custody audit.

## Slice 1: local observation and explicit attach-or-start

`discovery::LocalDiscovery` opens the existing platform registry read-only, without
creating/migrating it, claiming a library, cleaning rows, or updating last access.
A missing/inaccessible/unsupported registry is an error, not a fabricated empty
machine. `snapshot()` separates registered libraries from tracked instance rows.
A tracked `ready` row is an advertisement, not proof of live readiness.
`probe()` returns authenticated, compatible live descriptions or unresolved rows.
The example `local_discovery` prints discovery and local-model observations as JSON.

`describe_instance` is an additive token-authenticated operation over the existing
length-prefixed local IPC. It reports discovery schema 1, registry library ID/root,
start generation, release version, supported `pumas.local-ipc` protocol versions,
model-reference and selector schema versions, and existing model capabilities.
Release equality is not required. Required schemas/capabilities and the implemented
protocol must match. Tokens remain private registry credentials and are absent from
public descriptions/observations. This schema does not define inference capabilities,
request/result types, model routes, HTTP advertisements, or build artifact identity.

`attach_or_start(registry, selected_root, requirements)` returns `LocalAccess::Borrowed`
or `LocalAccess::Owned`. It requires an explicitly selected existing root. For a
tracked owner, it verifies the token, library registry ID/root, generation, schema,
protocol and required capabilities. A startup claim, legacy row, unreachable service,
failed handshake, or incompatible service blocks startup; no failed attachment falls
through to taking ownership. Dropping a borrowed client has no shutdown authority.
For an unoccupied root it preflights compatibility, then uses the registry's atomic
claim transaction. A concurrent winner is retained. This startup disables HF,
process-manager initialization and connectivity probes; it performs no model download
or runtime installation. Existing library-owner initialization/reconciliation remains.

`local_model_snapshots()` inspects every registered library's existing index read-only,
retaining the library context and the existing versioned model references. Missing
indexes remain unavailable observations. `LocalAccess::query_local_models()` forces
local-only acquisition policy through the existing intent API. These primitives let
consumers search local libraries before an explicitly authorized acquisition; they
do not silently select a similarly named model or flatten equal model IDs across roots.

The owner retains its immutable ready row. Authentication compares that retained
credential/generation to the current row; an old service cannot authenticate a
successor's token. Failed/cancelled startup retains its claim; ready-row release is
root/start-generation/token fenced. Promotion captures its ready row within the same
SQLite write transaction. `mark_instance_ready` retains its existing public signature.

Compatibility changes: `try_claim_instance` now refuses **any** existing owner row;
`cleanup_stale` is a compatibility no-op; owning `PumasApi::discover()` remains owning
and no longer cleans rows. A crashed historical owner therefore requires explicit
custody reconciliation. Automatic crash reclamation is deliberately unavailable until
independent lifetime custody is qualified. Existing administrative register/unregister
methods are not a safe crash-recovery protocol and are not used by discovery/bootstrap.

Supported exclusion is among cooperating clients of the same registry. This slice
**does not** establish a physical-store lifetime lease across distinct registries,
shared hosts/filesystems, namespace-isolated deployments, or store/lock replacement.
The custody audit remains open. Numeric PIDs and failed loopback probes never supply
cessation evidence. The registry UUID is explicitly a cache-entry identity, not an
invented durable physical-library identity.

### Ordered local owner shutdown follow-up

Independent review identified that generation-fenced deletion alone did not prove
predecessor cessation: ordinary `PumasApi` drop released the row while admitted
finite effects could still run, and the stored IPC handle/accept-task state formed
a lifetime cycle. The follow-up adds `shutdown_instance()` and
`LocalAccess::shutdown_owned()`, with a shared independently owned coordinator.
Borrowed shutdown is a no-op. Ordinary owned drop closes background admission and
starts the same coordinator; the registry row remains until finite work, IPC,
conversion owners, managed runtime profiles and acquisition have settled. Any
failed drain or lost executor retains unresolved ownership.

The coordinator takes the IPC handle out of primary state, closes admission,
observes accepted dispatches, and stops blocked response delivery only after its
domain operation has settled. Accepted-dispatch panics remain archived even when
completed handles are reaped. Failed or cancelled waiters cannot delete a row. Failed/cancelled
construction also retains its startup claim: an initializer running on a blocking pool cannot be
assumed to have stopped just because its caller disappeared.
Deterministic tests gate a real index mutation, test blocked successor admission,
check the actual former listener and connection, and retain stale-generation and
borrowed-service tests. Explicit shutdown is required to observe completion before
exiting a host runtime; drop alone is not a synchronous cessation receipt. This
repairs the same-registry local lifecycle only. Physical-store, shared-filesystem,
namespace and historical-owner custody qualification remain open.

## Slice 2: one shared build/protocol descriptor and HTTP advertisement

`build_info::PumasBuildInfo` is the single additive shared build descriptor.
It carries schema version 1, component/package version, optional build/source/target
provenance, actual namespaced compiled features, and supported protocol/schema
identities. Optional provenance comes from `PUMAS_BUILD_ID`,
`PUMAS_SOURCE_REVISION`, and `PUMAS_BUILD_TARGET` at compile time; absent data stays
absent. Compiled features do not assert runtime inference capability or model readiness.
The first-slice `ProtocolAdvertisement` import remains available, and
`InstanceDescription.build_info` is optional for old peers.

`pumas-rpc --build-info` prints this typed producer identity without constructing a
runtime, opening a library or starting a listener. The RPC producer includes actual
RPC and linked core features. Release strings do not substitute for negotiation.
Model-operation contracts define inference request/result/capability types.
Distribution assets and manifests use this shared build schema.

A prepared `HttpServiceRegistration` is invisible until the listener/router owner
explicitly publishes it. RPC binds the actual loopback listener and polls its accept
loop before publication. The additive registry table fences publication by retained
core generation/token/library context and revocation by that identity plus the HTTP
service generation. A live service cannot be replaced by another publisher within
its owner generation. Old publishers/revokers cannot overwrite/delete successor data.
Read-only registry observation tolerates an absent legacy HTTP table without migration.

`GET /.well-known/pumas` reports HTTP advertisement schema 1, a service generation,
a numeric-loopback base URL, the authenticated core instance identity and the HTTP
producer build descriptor. The router retains its bind generation; an old listener
cannot describe a successor. The existing Host/Origin admission and shutdown gate
apply. Responses use `Cache-Control: no-store`. Core IPC is never presented as HTTP.
The `pumas.local-http` protocol version 1 identifies this local rendezvous contract;
HTTP inference capabilities/model readiness must come from the modality lane.

`LocalDiscovery::borrow_http_service` authenticates compatible core IPC first, then
fetches the advertised description with no proxy/redirect, a bounded body and timeout.
It requires the full advertised descriptor and authenticated instance to match, and
checks the implemented HTTP protocol/schema. Initial attachment, HTTP observation and
final core reauthentication share one bounded bootstrap deadline. The borrowed result
has no shutdown operation. Absence, incompatibility and unreachable listeners remain errors, with no
startup/reclamation fallback. Existing explicit `attach_or_start` provides local core
bootstrap; choosing/extracting/launching a distributed HTTP binary is packaging work.

HTTP shutdown revokes advertisement admission first. A separate core-owned finite
custody receipt remains until the HTTP supervisor observes accepted HTTP requests,
catalog/source workers, installations and other external owners. This separate receipt
avoids a cycle with RPC's existing core finite-work drain. Successful external cessation
then permits ordered core shutdown/release. A failed/abandoned receipt retains unresolved
core authority. Dropping a registration is revocation plus failed custody, never evidence
that its external service stopped. Call `complete_shutdown` only after owned effects settle.
Completion is terminal even for a never-published registration; publishing another service
requires a fresh registration and its own unsettled custody receipt.
The same-registry support boundary and physical-store/namespace limitations remain.

### Request admission follow-up to slice 2

The additive [HTTP admission fence](../contracts/local-http-admission-fence.md)
binds opted-in requests to the already authenticated core and HTTP service
generations. RPC advertises schema `pumas.http-admission-fence@1` through the
existing shared build descriptor. Older peers remain observable, but the typed
fence helper refuses them. Header-boundary admission checks the retained
listener identity and existing registry/token fence before any route handler.
This closes stale URL reuse for fenced requests while leaving startup authority,
physical lifetime custody and crash reclamation at their existing boundaries.
Distribution's existing RPC-to-core descriptor projection now removes both
RPC-only HTTP schemas while preserving unrelated core schemas. Native process
evidence is recorded
separately from the controlled custody and registry-replacement fixtures.

## Slice 3: durable identity and qualified lifetime custody

Define one on-disk library identity/migration and a physical-store primary lifetime
lease, distinct from finite mutation grants. Preserve explicit legacy-unknown outcomes;
never manufacture historical exit receipts. Generation-fence recovery transitions.
Qualify competing-owner, crash, namespace/reused-PID/inaccessible-owner, child lifetime,
and filesystem replacement behavior natively on each supported OS. No shared-store or
real cluster safety claim follows from local SQLite serialization.

### Internal physical-root lock groundwork (recovery remains disabled)

`platform::store_lifetime::PhysicalStoreLease` is an internal primitive, not a
new owner API. Its Linux/macOS implementation uses the existing `fs2` dependency to
lock an independently opened root directory. Same-process independent opens,
symlink aliases and separate rendezvous registries cannot acquire that same
physical directory concurrently. Clones share the held descriptor, without an
explicit unlock or raw-descriptor escape. It creates no lock file or ownership
marker and does not read PIDs or mutate registry rows. Acquisition on other platforms
fails explicitly until a native equivalent is implemented and qualified.

`retain_for_effect` moves a lease share into the actual blocking closure, so
canceling its requester or shutting down the async runtime does not release a
still-running effect. Dropping the final share closes this process's descriptor;
a descriptor inherited by a concurrent fork can extend exclusion until it closes,
normally on exec. Observed native lock reacquisition is not a historical cessation
receipt or permission to replace a registry row.
The existing `PumasApi` now acquires this primitive on Linux/macOS before
constructing model directories or starting owner effects. Creating a missing
launcher root is the only pre-lease filesystem mutation. The existing registry
claim is still required, and failed construction, failed shutdown and process
loss retain their existing generation. No additional public owner API exists.
Other platforms preserve their existing conservative registry behavior; they
have no qualified physical-root lifetime implementation.

The lock identifies a physical directory, not a pathname or an entire tree.
Renaming a held directory preserves its exclusion, while `require_current`
refuses a captured pathname that is missing or names a replacement. A replacement
is a different physical object and can have its own lock. That check is only an
observation: downstream writes need captured filesystem capabilities and their
own namespace validation. An advisory lock cannot exclude non-participating
writers. Separate launcher roots that share a symlinked model/storage subtree,
network filesystems, namespace/host boundaries, and hostile replacement are not
qualified by this primitive.

This private integration retains the lease through constructor directory/model/
HF-cache/client workers, `ModelLibrary`, independently cloned `ModelIndex` and
`LinkRegistry`, `AcquisitionStore`, acquisition blocking workers, `RuntimeTasks`
and its actual nested closures, migration/report writes, HF search/tree cache
writes, mapper directory/link/copy/removal leaves, direct and internal-dispatch
broken-link unlink leaves, external-diffusers registration directory creation,
runtime-profile configuration/session workers, and legacy process-manager
clones/child-observer threads. SQLite connection fields are dropped before their
lifetime share. No lifetime share has an explicit unlock operation.

`shutdown_instance()` drains the existing owners and may release its exact
registry row, but the physical lock remains held until the `PumasApi`, escaped
mutable handles and actual retained effects are dropped. Callers must drop those
handles before constructing another owner, even after shutdown returns. A clean
shutdown receipt does not revoke an escaped direct index/store handle.

This is live exclusion integration, not full recovery qualification. The full
inventory and remaining authority gates are:

1. Acquire before constructor effects. Retain directly in the model/index
   initializer, HF cache/client initialization and filesystem worker closures;
   dropping a construction future is not evidence those workers stopped.
   Implemented for current constructor leaves; root creation is synchronous.
2. Retain in every independently escaping model writer: `ModelLibrary`, cloned
   `ModelIndex` and `LinkRegistry`, importers/mappers and their path-only effects.
   The lease must outlive SQLite finalization/checkpoint effects. Cover projection,
   migration/report, link and cache writes, including read APIs that regenerate
   durable projections. Implemented for model/index/link, projection and
   migration/report workers; standalone legacy constructors carry no ownership
   qualification and remain outside the `PumasApi` composition contract.
3. Retain in `AcquisitionStore`, consumers/use handles and real task-custody
   effects. Compose it with existing workspace/root execution grants. A service
   reference held only by an async observer is insufficient. In particular,
   `RuntimeTaskContext::run_blocking` must capture it inside its actual closure;
   acquisition blocking custody now carries this lifetime independently of its
   existing effect grant. Public service/store and consumer handles retain it.
4. Join startup orphan adoption, download restoration, intent/watcher
   reconciliation and runtime-profile state writes to these effect owners.
   Existing `RuntimeTasks`, acquisition task custody and ordered instance
   shutdown are the coordination points and now retain this lease. Runtime
   profile sessions and legacy child observation retain in-process shares. The
   native watcher's idle callback holds only a weak primary reference; admitted
   event tasks upgrade and retain the owner. This
   does not prove child cessation after the process itself dies.
5. Explicitly qualify or persistently refuse independent child/external custody.
   Disabling the legacy process manager does not disable managed runtime profiles,
   conversions/setup/probes, launcher update/restart or external HTTP owners.
   Arbitrary acquisition callbacks may themselves start independent writers;
   their successful return does not establish child lifetime custody. The native
   lock is not deliberately inherited across exec and cannot stand in for it.
   Conversion/setup/probe descendants, third-party acquisition callback effects,
   ambient path-only system/update effects and external HTTP owners therefore
   remain unqualified. Legacy `ProcessManager` is not in the current
   `shutdown_instance()` stop list; its observer may retain this lock indefinitely
   while the process remains live.
6. Persist a versioned physical identity and bounded ownership qualification.
   Under an actually held exclusive lease, atomically generation-fence registry
   replacement and reject legacy, unknown and unqualified ownership. Add full
   constructor failure/cancellation, admitted-effect, real process-loss, stale
   generation and child-custody integration fixtures before enabling recovery.
   This is still pending: an alternate empty registry has no historical-owner
   evidence. A free native lock after process loss is never sufficient to infer
   whether an old owner left a surviving external writer. No PID-based or
   lock-only registry replacement has been added.

Native primitive tests exercise same-process contention, alias/different-registry
exclusion without row changes, final-share release, missing/non-directory/FIFO roots,
rename/replacement, panic and blocking work surviving requester/runtime shutdown.
The Linux process fixture observes and reaps SIGKILL before reacquiring the native
lock, then verifies the old registry row still blocks primary admission. These
are primitive-level tests, not `PumasApi` restart qualification. The additional
`physical_owner_lifetime` target exercises the actual `PumasApi` builder, escaped
library/index/link/store handles, cancelled acquisition waiters, and clean restart
only after final-share release. Its real-process fixture verifies that SIGKILL
frees the physical lock but still cannot replace the exact retained registry
entry. The feature-gated `physical_mapper_lifetime` target gates the actual
mapper creation/overwrite, direct cleanup, internal-dispatch cleanup and external
registration directory leaves, cancels their callers and stops their runtime before
releasing each effect. The cleanup dispatch fixture exercises the existing internal
branch; it adds no typed IPC wire operation. Its negative control removes only the
shared leaf lifetime capture while preserving the effect gate.
Native execution status is recorded separately; source presence is not
qualification. macOS native
behavior, Windows implementation, full shared-store identity and full-owner
process-loss recovery remain pending.

### Passive consumer retention follow-up

[`local-owner-retention.md`](../contracts/local-owner-retention.md) defines an
opaque generation-bound passive guard using existing external-service tasks and
physical lifetime shares. Mandatory-fenced HTTP streaming and a read-only CLI
holder make it usable by external consumers. Exact row release waits for guards;
operator HTTP shutdown revokes bodies before core drain. Availability after
shutdown, external-effect cessation and crash/exec transfer are not promised.
The optional RPC-only schema uses the existing build descriptor and distribution
projection. Native process tests and controlled
fixtures qualify different behavior; neither establishes runtime model readiness
or a release qualification.

## Explicit startup and distribution projection

The follow-on explicit selected-root reservation is documented in
[`local-start-authority.md`](../contracts/local-start-authority.md). It transfers
the existing held native lease and exact pending claim into the current builder,
and adds an inference-disabled Linux CLI acknowledgment for owned/borrowed HTTP
access. This closes a bounded typed start-admission gap; automatic historical
bootstrap and dead-owner recovery remain pending. Passive consumer retention
cannot transfer across processes. The existing
`scripts/release/headless_inference_build.py` generator excludes the three RPC-only
HTTP schemas from the core descriptor, with unrelated schemas retained.
The actual RPC descriptor remains complete. Desktop/schema exporters are unchanged.
