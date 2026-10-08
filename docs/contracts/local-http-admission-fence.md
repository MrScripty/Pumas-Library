# Local HTTP admission fence, schema 1

The RPC producer advertises `pumas.http-admission-fence` version `1` in its
existing shared `PumasBuildInfo.schemas`. Authenticate the selected owner with
`LocalDiscovery::borrow_http_service` or the existing
`pumas-rpc --describe-local-http --launcher-root EXISTING_ROOT` before using the
description. CLI consumers must require that schema advertisement; an older
`pumas.local-http` version 1 producer can ignore these headers. Neither an
arbitrary HTTP response nor `--build-info` authenticates a running owner.

`HttpServiceDescription::admission_fence()` returns the typed
`HttpAdmissionFence` only when schema 1 is advertised. For Python, C#, Node and
other HTTP consumers, send these two headers on every request that must target
the authenticated generation:

| Header | Exact description field |
| --- | --- |
| `Pumas-Instance-Generation` | `instance.generation` |
| `Pumas-Service-Generation` | `service_generation` |

Preserve the values exactly. Each header must occur once and contain a nonempty
visible ASCII token of at most 256 bytes, without commas. A partial, repeated,
combined or malformed pair returns HTTP **400**. With both headers absent, the
existing local HTTP behavior is preserved. Browser preflight allows the two
headers for the existing permitted local origins.

The fence applies to all existing routes, including `/rpc`, model operations,
events, health and the HTTP description. After existing Host/Origin admission,
it compares the requested generations with both this listener's retained bind
identity and the current generation/token-fenced registry advertisement before
body decoding or handler admission. A live current listener with a mismatched
request returns HTTP **412**. A closing owner, unavailable/corrupt registry
advertisement or advertisement that no longer matches this listener returns
HTTP **503**. Fence refusals have not entered the route handler. Responses are
visible to the existing permitted browser origins through CORS.

On 412/503, observe and authenticate again before deciding what to do. This
contract does not authorize replay of a previous request with an unknown
outcome. A fence does not provide authentication, model availability, startup
authority, a transferable lifetime lease or permission to reclaim an owner.
An already admitted operation retains the existing effect/custody/shutdown
rules. The existing registry claims and physical lifetime lock continue to
exclude cooperating owners; dead/stale rows remain unresolved after a crash.

The native process tests run actual Pumas core, IPC and HTTP server code in
inference-disabled child test executables. They cover competing startup,
cancelled shutdown waiters, orderly restart at the same URL, stale requests and
process-loss refusal after registry reopen. Registry-advertisement replacement
and the held external-custody receipt are explicitly controlled fixtures.
They do not qualify an inference-enabled archive, real model inference,
Windows/macOS lifetime custody or automatic crash reclamation.

The existing distribution build producer removes both RPC-only HTTP schemas
(`pumas.http-advertisement` and `pumas.http-admission-fence`) when projecting the
RPC build descriptor to the core descriptor. Keep the complete actual RPC
descriptor in HTTP descriptions and archive compatibility records. This
projection keeps unrelated core schemas and leaves the full RPC descriptor
unchanged. Controlled archive tests and an actual CLI-produced descriptor check
cover this adaptation; it does not qualify a v0.8 release.
