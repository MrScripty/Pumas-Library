# Passive local owner retention, schema 1

The [callable CLI consumer workflow](local-http-consumer.md) composes authenticated
selection, actual held retention, fenced requests and owned/borrowed context
cleanup using these existing contracts.

This is a supported retention primitive for a consumer that has selected and
already authenticated a local owner. It retains the existing primary generation
and physical lifetime through the existing external-service custody task owner.
It creates no lock, ownership registry, durable token, renewal protocol or model
lease. It grants no startup, mutation, shutdown or recovery authority.

`PumasApi::retain_local_owner(&authenticated_instance_description)` returns an
opaque, non-cloneable, non-serializable `LocalOwnerRetention`. Admission requires
an exact current ready description and current held physical root, and uses the
existing synchronous custody close gate. An old, closing, changed or incompatible
owner cannot grant another hold. Fresh start admission remains exclusively with
`LocalStartAuthority`; discovery JSON and retention JSON are never start authority.

Drop/release ends only this passive hold. `release().await` observes its task result,
not global owner/effect cessation. Independent operations keep their own custody.
Ordinary API drop or shutdown closes availability and waits for retained guards
before exact registry-generation release. Cancelling a shutdown waiter leaves the
shared coordinator running. In-process callers must release their guards to finish
the drain; a guard does not keep a closing owner operational. Failure/loss of the
owner's task executor retains unresolved registry ownership.

## HTTP and CLI consumers

The Linux RPC producer advertises optional `pumas.http-owner-retention@1` in its
existing full build descriptor. Require that exact advertisement and authenticate
the HTTP description before opening:

```text
GET /.well-known/pumas/retention
Pumas-Instance-Generation: <authenticated instance.generation>
Pumas-Service-Generation: <authenticated service_generation>
```

Both exact headers are mandatory on this route. Missing headers return 428,
malformed headers 400, mismatched generations 412, and closing/unavailable/current
identity failure 503. Native local Host/Origin admission and existing CORS apply.
Each listener permits at most 64 active retention bodies; exhaustion returns 429
without admitting a guard. This is a transport resource bound, separate from
primary ownership and the existing request-handler concurrency bound.

The `text/event-stream` response has `Cache-Control: no-store`. Its first
`pumas-owner-retention` event acknowledges admission only after the guard and fence
are validated. Data has exactly these fields:

```json
{"retention_schema_version":1,"state":"retained","instance_generation":"...","service_generation":"..."}
```

Hold the response body open for the desired passive retention. There is no TTL or
renewal; keepalive comments aid eventual peer-loss observation and are not proof
of consumer or effect cessation. A body holds its actual core guard and stream
permit until disposal or revocation. Closing/aborting the connection requests
release; observation timing depends on TCP/OS/network state and has no fixed bound.
A disconnected consumer must treat retention and owner availability as unconfirmed
until it authenticates again, and must not replay uncertain operations.

Operator HTTP shutdown closes admission and emits the same event with
`state: "revoked"`, then ends the body and releases its passive guard. This does
not veto shutdown. An unpolled/downstream-blocked body remains held until it is
polled or disposed under the existing connection shutdown policy. The HTTP
supervisor observes those connections and all admitted effects before reporting
external cessation and allowing core row release. The stream never awaits its
own core shutdown/drain. EOF/error without a terminal revoked event is loss of
transport, not a cessation receipt or takeover permission. There is no automatic
reconnect, and a new owner/service generation always needs a new authenticated
connection. Acknowledgment JSON alone cannot transfer the live hold to another
process or revive it after exec/restart.

For command-line consumers of a pinned binary:

```sh
pumas-rpc --retain-local-http-owner --launcher-root EXISTING_ROOT
```

This read-only selection authenticates an existing owner and holds its HTTP
retention body. It never starts or stops the owner. Read the
`PUMAS_LOCAL_RETENTION=` JSON acknowledgment, then retain/supervise this process.
Local SIGINT/SIGTERM releases this consumer's body and exits successfully;
operator revocation emits the terminal JSON and exits successfully. Lost transport
or unsupported/changed identity fails, with no startup fallback, retry or reconnect.
The acquisition/first-acknowledgment phase is bounded to ten seconds; an admitted
hold has no elapsed/idle lifetime limit. HTTP consumers can implement the same
contract directly with fetch/readable-body cancellation or an HTTP streaming
client. Browser EventSource does not support the required headers and its automatic
reconnect is unsuitable. Consumers should keep the generation-fenced
operation connection policies separate from this passive retention connection.

The existing distribution projection strips this RPC-only retention schema from
the core descriptor alongside the two existing RPC-only HTTP schemas. Unrelated
schemas and the full actual RPC descriptor remain unchanged. There are no desktop
wire/exporter changes.

## Qualification boundary

This contract supports cooperating Linux local owners and the same persisted registry.
It does not qualify inference-enabled bundles, real model availability/inference,
Windows/macOS, hostile namespace/root replacement, shared filesystems, external
child custody or historical recovery. Killing the owner loses the live guard,
but leaves its exact registry row unresolved; a free lock, dead PID or disconnected
consumer never authorizes takeover. Killing a consumer releases only a passive
connection once the live owner observes it, without claiming independent effects
or surviving children stopped. These tests do not qualify external consumer
integration.
