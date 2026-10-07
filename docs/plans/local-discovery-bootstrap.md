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
successor's token. Startup cleanup is claim-token fenced; ready-row release is
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

## Slice 2: one shared build/protocol descriptor and HTTP advertisement

Proposal for parent assignment: one additive core `PumasBuildInfo` with release version
and supported protocol name/version pairs. Discovery can own this type if assigned;
packaging adds build/artifact identity through the same type, not a parallel schema.
The modality lane owns inference request/result/capability types. Discovery should
reference those types only after their contract lands.

Publish a loopback-only typed HTTP advertisement only after the listener is bound
and ready, scoped to library and owner generation. Revoke it by that exact generation
on ordered owner shutdown. A read-only HTTP description must distinguish protocol
support, inference capability and model readiness using the modality lane's schema.
Do not expose core IPC as an HTTP endpoint or infer an HTTP port from a core IPC row.
Add a bootstrap client/extraction harness integration with packaging: attach before
launching, preserve selected library/model context, never stop a borrowed process.

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
