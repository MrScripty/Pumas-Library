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
fixtures, not ASR. Regression fixtures also cover cancellation while the original
child is waiting to answer hello, lost async workers during uncertain drainage,
and retained endpoint clones through retirement/restart. Actual installed-model
startup/load cancellation and real-model execution remain qualification gates.
A successful dependency import probe does not satisfy them.

The runtime-profile service now retains conditional installed sessions in its
private lifecycle owner. It reserves the original profile, model and generation
before polling the deferred installed-byte producer, then consumes only the
qualified opaque runtime and original prepared allocation produced by owned
blocking preparation. It returns the exact generation with the endpoint. It does not replace Torch's
HTTP provider or add a public runtime factory. Binary and audio launches share a
monotonic generation namespace. The profile operation guard remains held through
startup, operation and confirmed drain, excluding overlapping launches, edits,
and deletion. The independent blocking child owner retains that guard even if
the async executor drops its lifecycle task.

Ordinary profile stop first checks installed ownership; generation-specific stop
refuses a mismatched generation. Composed shutdown closes both owners before
waiting and joins startup cancellation as well as live sessions. Dropping a
startup waiter or the lifecycle owner requests stop without discarding the
independent drain obligation. Controlled registry tests cover stale stops,
cancelled waiters, close-before-join and retried joins. These are lifecycle
regressions, not real-model execution qualification.

## Production producer connection

The existing Torch serving adapter dispatches audio library records to the
installed-audio producer; existing image/HTTP providers keep their own path.
The app manager retains the source-pinned interpreter, dependencies, native
libraries and embedded sidecar. Core's `serve_installed_audio_for_operation`
accepts a deferred producer of that opaque byte owner and the original model
selector. Admission and the shared profile guard precede every preparation await,
so stop/edit/delete cannot pass an unregistered startup. Core resolves the indexed
artifact itself and prepares the original held model allocation. A stop during
preparation closes the original admission immediately and joins already admitted
preparation before refusing child launch.
Finite preparation is registered with the primary runtime-task owner, including
its blocking copy/capture and potentially large recipe/model validation, so cancellation cannot make shutdown skip that work.
Only the source-pinned interpreter member is selected; no executable path,
source ID or qualifier is accepted from RPC. The installed recipe/model policy
still refuses unqualified execution before child launch.

Successful load publication requires both the current serving-load receipt and
the current running audio generation. Status uses the private endpoint, with no
HTTP URL; HTTP-only operations refuse that active profile. Unserve stops the
captured original audio generation. The session worker removes serving rows and
retires only its exact endpoint after child/diagnostic join, before releasing the
profile-operation guard. Old endpoint clones remain closed, cannot block a
confirmed-drained successor, and cannot retire that successor. Lifecycle task
panic follows the same cleanup path; task loss is an explicit error after the
independent child drain, never a successful stop observation. Unresolved worker
loss retains the pending profile guard fail-closed; normal terminal publication
explicitly releases it so retained stop waiters cannot block a successor.
