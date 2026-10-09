# Explicit local start reservation

The [callable CLI consumer workflow](local-http-consumer.md) delegates optional
bootstrap to this authority and holds the resulting owner's existing HTTP
retention contract through request/context cleanup.

`prepare_local_access(registry, existing_root, requirements)` provides an opaque,
process-local `LocalStartAuthority` when no instance is registered for the selected
root. It reuses the existing native physical-root lease, atomic pending primary
claim and builder custody coordinator. It creates no second ownership registry.
The new reservation path is qualified on Linux with cooperating native locks.

If a row already exists, preparation authenticates and checks compatibility with
that owner. A ready compatible owner produces `PreparedLocalAccess::Borrowed`.
A claiming, stale, unreachable, tokenless or incompatible owner produces an error;
it never falls through to a new start. Choose the same persisted registry across
restarts. An empty alternate registry carries no historical custody evidence.

The authority cannot be cloned, serialized or reconstructed from a discovery
description. It retains the native lease and exact pending claim before any
constructor effect. Its selected canonical root, held lease root and claim root
must agree before admission; the existing physical-current check also applies.
These checks are observations, not protection against arbitrary hostile path
mutations during later ambient filesystem operations.

Consume `start().await` to transfer the held reservation into the existing builder,
with HF client, legacy process manager and connectivity probes disabled. The
builder validates the exact pending claim before effects and keeps the existing
effect lifetime shares and shutdown coordinator. Consume `cancel()` to delete
only this exact still-unstarted pending claim while its lease remains held. Once
start is consumed, cancellation is unavailable. Dropping an unstarted authority,
failed construction or a cancelled start future retains unresolved ownership.
A free lock or dead PID never authorizes deletion or takeover of that row.

For cross-language callers, the Linux `pumas-rpc` binary adds:

```sh
pumas-rpc --attach-or-start-local-http --launcher-root EXISTING_ROOT --port 0
```

The process either owns and runs the existing HTTP server, or authenticates an
already running HTTP owner and exits successfully without stopping it. Read the
single `PUMAS_LOCAL_ACCESS=` JSON line among startup logs. It contains
`bootstrap_schema_version: 1`, `ownership: "owned" | "borrowed"`, and the existing
authenticated HTTP `description`. A borrowed result returns the owner's existing
endpoint; requested host/port do not create a replacement listener. Retain and
supervise an owned process until orderly shutdown, and use the advertised HTTP
generation fence for requests. The JSON is an acknowledgment, never a transferable
start token or consumer lifetime lease. Consumers must pin a binary supporting
this CLI mode; its schema is not a new core/RPC build descriptor export.
Require `pumas.http-admission-fence@1` in the returned description before using
generation-fenced requests; an acknowledgment can describe a compatible older
borrowed owner that lacks that optional fence.

Simultaneous cold starts admit one reservation. A contender can fail while the
winner is claiming; callers may observe again, but there is no automatic takeover,
wait policy or retry of a request with unknown outcome. If a borrowed owner has
no compatible HTTP service, the CLI fails instead of starting another owner.

Inference-built binaries use the same authority and HTTP endpoint under the
[inference-built bootstrap contract](inference-enabled-bootstrap.md). They can
initialize the control plane without selecting or loading an inference model;
consumers must check actual model capabilities separately. Constructor custody,
startup signals and runtime shutdown are part of that bounded contract.

Actual Linux consumer processes exercise held reservations, competing bootstrap,
authenticated borrowing without owner shutdown, explicit pre-start cancellation,
clean restart and killed/reaped owner refusal after registry reopen. Queued
constructor cancellation, registry replacement and mismatched captured roots are
controlled fixtures. Historical CLI smoke evidence uses the actual inference-disabled debug
binary and a disposable existing library with a sentinel. This does not qualify
real models, inference shipping bundles, Windows/macOS, consumer lease transfer,
historical external-child custody, crash reclamation or a v0.8 release.
