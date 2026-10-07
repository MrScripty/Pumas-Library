# Local discovery and bootstrap — v0.8 incremental slices

Base: main `5e114f6d8e4559e0a4d67e56000b423120a0fde0`. Scope is local
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
The modality lane owns inference request/result/capability types, and packaging owns
assets/manifests; neither needs to create a competing build schema.

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
checks the implemented HTTP protocol/schema. The borrowed result has no shutdown
operation. Absence, incompatibility and unreachable listeners remain errors, with no
startup/reclamation fallback. Existing explicit `attach_or_start` provides local core
bootstrap; choosing/extracting/launching a distributed HTTP binary is packaging work.

HTTP shutdown revokes advertisement admission first. A separate core-owned finite
custody receipt remains until the HTTP supervisor observes accepted HTTP requests,
catalog/source workers, installations and other external owners. This separate receipt
avoids a cycle with RPC's existing core finite-work drain. Successful external cessation
then permits ordered core shutdown/release. A failed/abandoned receipt retains unresolved
core authority. Dropping a registration is revocation plus failed custody, never evidence
that its external service stopped. Call `complete_shutdown` only after owned effects settle.
The same-registry support boundary and physical-store/namespace limitations remain.

## Slice 3: durable identity and qualified lifetime custody

Define one on-disk library identity/migration and a physical-store primary lifetime
lease, distinct from finite mutation grants. Preserve explicit legacy-unknown outcomes;
never manufacture historical exit receipts. Generation-fence recovery transitions.
Qualify competing-owner, crash, namespace/reused-PID/inaccessible-owner, child lifetime,
and filesystem replacement behavior natively on each supported OS. No shared-store or
real cluster safety claim follows from local SQLite serialization.

## Reserved paths and integration dependencies

This slice owns `pumas-core/src/discovery/`, `src/registry/library_registry.rs`,
`src/ipc/{local_client,protocol}.rs`, `src/api/{builder,state}.rs`, `src/lib.rs`,
`examples/local_discovery.rs`, and this plan. `src/api/hf.rs` has one test fixture field.
No inference gateway, HTTP server, contract exports, dependency manifests/locks,
packaging scripts, release assets or generated reports are changed. Test logs live
outside Git. The next HTTP slice needs narrow coordination on RPC server/startup
files and the assigned shared build/protocol descriptor.
