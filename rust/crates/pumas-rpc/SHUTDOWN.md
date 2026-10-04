# RPC shutdown ownership

A `shutdown` response with `status: shutting_down` acknowledges an idempotent
request to the process-owned supervisor. It does not claim cleanup is complete,
and does not return unobserved managed-process counts. The requesting HTTP
handler must not await the supervisor receipt, which includes its own response.

The server keeps accepted HTTP responses alive through graceful completion and
ends all five SSE feeds when shutdown is requested. Domain owners drain
concurrently with HTTP, preserving independent failure observations. An OS
signal and an RPC request converge on the same shared, repeatedly observable
receipt; cancelling one waiter does not cancel the supervisor. `main` observes
that receipt and exits successfully only on success. Windows clients should use
the RPC path rather than treating process termination as a graceful signal.

## HTTP lifecycle policy

`--http-shutdown-grace-ms` controls the common deadline for accepted HTTP/1
connections after shutdown starts (default 10,000 ms; accepted range 1 through
3,600,000 ms). This is not a normal request or generation timeout. Axum's enabled
protocol was already HTTP/1 only; Hyper now supplies explicit connection
ownership without enabling or claiming HTTP/2.

The listener stops accepting connections. Health, SSE and gateway routes refuse
new work at headers; RPC also checks after body decoding and only allows the
idempotent shutdown acknowledgement. Existing SSE bodies end normally. Accepted
connections get graceful completion until the shared deadline, after which only
their exact transport future/socket is closed. A timed-out or failed response
transport is reported as incomplete; it cannot produce a successful receipt.

Admitted handler futures live in a separately retained request task set, so
closing their socket never drops an unfinished core operation. Request admission
uses a bounded channel. The supervisor joins both request and connection task
sets and all existing domain owners, aggregating failures. A stuck domain handler
can still delay process exit beyond HTTP grace: there is deliberately no claim
that transport closure proves all effects have ceased. The external process
owner may escalate under its separate policy, but must report forced or
unconfirmed cleanup.

## Availability and failure receipts

An isolated request-handler or response-connection panic is logged and retained
in the final failed receipt, but does not request server-wide shutdown. Other
clients keep serving; a failed handler receives HTTP 500 if its response can
still be delivered. This does not erase domain-owner failure or establish that
an unknown operation ceased. Existing domain custody and drain owners retain
their independent outcomes and still participate in final shutdown.

Accept errors are classified from structured I/O state. Aborted/reset/refused
connections and interruption retry without a resource delay. Resource or
unclassified errors retain the observed listener and retry after one second;
that absolute backoff does not block admitted request/connection work or delay
an explicit shutdown. ErrorKind alone cannot establish that the listener is
unusable: even permission/unsupported errors can describe a rejected pending
socket. Invalid accept state (EINVAL/WSAEINVAL) or a failed observation of the
held listener stops serving and stays in the final failed receipt. No string matching or empty-group assumption is
used. Repeated identical task failures are deduplicated in the receipt rather
than growing an unbounded history.

Native tests cover isolated handler and response-body panics, subsequent healthy
requests, transient-then-success accept sequences, resource backoff, immediate
shutdown during backoff, ambiguous pending-socket errors and unobservable
listener failure. The same outcome collector records explicit connection
failures in active serving and drainage, including a shutdown-observation race. Accept
failures are injected around a real loopback listener without changing process
resource limits or operating-system settings.

Hyper and hyper-util were already locked and included in shipped attribution.
The RPC crate now directly owns their HTTP/1 server and Tokio adapter features;
no package version or license selection changed. Tests cover partial request
bodies, blocked response writers, held handler ownership beyond socket closure,
benign keep-alive completion, and closed request admission.

The cross-platform real-process regression observes complete acknowledgement,
all applicable SSE end-of-body results, and successful process exit without any
OS signal. It runs explicitly on native Linux, macOS and Windows CI. The owned-transport unit regressions also execute explicitly on all three native
hosts. These deterministic local-process tests do not qualify a distributed
cluster, packaged Electron, or real model execution.


## Shared acquisition integration

The RPC supervisor drains local intent/copied imports and HF downloads, accepted
HTTP responses, conversion owners, managed runtimes and native installation
consumers concurrently. Each native manager retains installation work through
cancellation and cleanup. Only after these consumer drains settle does RPC close
the shared source-neutral acquisition supervisor. Consumer failure does not skip
that final observation; both errors remain in the repeated shutdown receipt.
HTTP grace controls transport cessation, not a deadline that abandons native or
blocking filesystem owners. No acquisition receipt substitutes for copied model
publication or native workspace cleanup evidence.
