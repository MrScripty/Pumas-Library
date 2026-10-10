# Conditional installed audio session ownership

The private Linux x86-64 session producer consumes an already qualified
`AudioRuntimeOwner` and the original `Arc<PreparedArtifactUse>`. It cannot
qualify an installed recipe, accept a caller executable, or enable a model.
Shipping admission remains closed. There is no public session constructor.

Before launch, the prepared source must belong to the retained library's
physical root. The source correlation ID is `pumas-cohere-owned-v1:` followed
by the prepared manifest digest. This is a comparison value, never authority.

The blocking session worker owns child supervision independently of the caller
waiting for startup. It retains the library through every spawn/setup failure
and until confirmed process-tree drainage. Runtime/model ownership is also
attached by the confined child constructor. Registry runtime retention and
child attachment occur atomically before publishing private protocol pipes;
a refused pre-child launch does not leave an unresolved runtime in a registry.

The session creates the original private channel, binds its runtime identity,
loads the original prepared allocation, and registers the resulting endpoint.
A startup guard requests shutdown on timeout, cancellation, or failure. The
caller must retain the returned session owner; an endpoint clone alone cannot
keep admission open after that owner is dropped.

Stopping first closes registry admission and quarantines the channel. The
independent worker retries bounded drain attempts while preserving the exact
child and all byte owners on uncertainty. Completion is published only after
the child tree is drained and its diagnostic reader has joined. Diagnostic EOF
and diagnostic read/thread failure are distinct outcomes. Diagnostic bytes are
consumed without unbounded buffering or exposure to callers.

Controlled regressions cover pre-spawn cancellation, spawn refusal, attachment
and diagnostic setup failures, lost startup waiters, uncertain drain followed by
recovery, atomic retention, and library-root mismatch. These are lifecycle
fixtures, not ASR. Actual hello/load cancellation, final profile lifecycle
integration, source-owned Python/model policy, and real-model execution remain
qualification gates. A successful dependency import probe does not satisfy them.
