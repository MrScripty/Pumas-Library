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

This first lifecycle slice retains graceful serving until completion. It does
not yet bound a silent generation response, incomplete request body or blocked
socket writer. The separate connection-owner slice must add an explicit,
configurable HTTP shutdown grace policy and report forced/incomplete HTTP
settlement without discarding unfinished domain-owner tasks. Ordinary generation
requests must not gain a hidden timeout as a side effect.

The cross-platform real-process regression observes complete acknowledgement,
all applicable SSE end-of-body results, and successful process exit without any
OS signal. It runs explicitly on native Linux, macOS and Windows CI. This remains
separate from active-effect and bounded-connection regressions and does not by
itself certify the pending bounded-drain policy.
