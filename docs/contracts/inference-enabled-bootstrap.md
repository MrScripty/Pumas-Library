# Inference-built local HTTP bootstrap

The Linux `pumas-rpc --attach-or-start-local-http --launcher-root EXISTING_ROOT`
path can initialize the existing generic HTTP control plane in a binary compiled
with `inference-plugins`. It selects no model, loads no native inference SDK,
provisions no runtime, and downloads nothing. A ready HTTP owner is not a ready
model. Consumers must independently check the selected owner's fenced
`GET /v1/capabilities` for their intended model and operation.

This is a bounded local candidate contract. It does not qualify an inference
shipping archive, real model execution, a supported host binding, non-Linux
reservation, physical-store recovery, or a v0.8 release. Pin the exact producer
binary hash and source/tree used by the qualification cohort. Product version,
compiled features and parser schema advertisements do not grant runtime readiness.

## Admission and ownership

The selected root must already exist and be a directory. Root, loopback host and
HTTP shutdown policy are checked before bootstrap registry/constructor effects.
The existing [local start authority](local-start-authority.md) remains the only
admission path: native physical-root lease, exact pending primary claim, then the
existing core builder. The captured canonical root is used by runtime metadata,
plugin configuration and authenticated readiness observation.

A compatible existing HTTP owner is authenticated and acknowledged as
`ownership: "borrowed"`; the selection process exits without creating inference
initializers or stopping that owner. Claiming, incompatible, tokenless,
unreachable and stale rows fail closed. No free-lock/dead-PID takeover, alternate
registry, automatic fallback start or recovery is admitted.

An admitted cold start keeps HF client, legacy process manager and connectivity
probe disabled in the reserved core builder. Managed runtime profiles remain
separate owners with their existing execution admission. Enabling a compiled
feature does not enable the legacy manager or grant authority to any external
child, installed runtime or model.

## Constructor custody and readiness

After the reserved core exists, an opaque `LocalStartupCustody` receipt records
initializer effects in its existing external-effect coordinator. Initialization
runs in a process-owned task; observing a stop signal does not abort that task.
One actual owning `PumasApi` is shared through `Arc` and held by server state
through shutdown. No core clone or replacement ownership registry is introduced.

Bootstrap runtime `VersionManager` or `PluginLoader` constructor failure is fatal.
Successfully constructed managers are all observed through installation shutdown,
then core shutdown is observed. There is no warning-only partial manager success
or shared temporary plugin fallback on this path. Existing optional size-cache
behavior and per-file malformed-plugin warnings remain their existing policies;
this contract does not introduce strict validation of every plugin document.

The same HTTP supervisor owns the initializers and listener. It settles custody
only after first-polling the real accept loop, before publication or any wait on
core shutdown. Failure or abandonment before this handoff is a sticky cessation
failure and retains the exact unresolved generation. This ordering avoids waiting
for core cleanup while holding the receipt that cleanup must observe.

Readiness requires actual HTTP publication and authenticated description. The
existing `RPC_PORT=` output and schema-1 `PUMAS_LOCAL_ACCESS=` acknowledgment are
fallible writes with observed flush. Description, serialization or output failure
enters observed owner shutdown before propagating the error. No new endpoint,
launcher, build identity or acknowledgment schema is introduced.

The shared stop latch is observed before consuming an unstarted reservation,
before publication, and before readiness writes. A signal during construction
waits for the owned constructor to settle and suppresses readiness. A stop
latched before listener publication returns a nonzero cancelled-start result
(`HTTP startup cancelled`) after successful drain; no owner readiness was granted.
A signal after readiness uses the same server shutdown receipt and returns zero
only after successful cessation. A signal arriving between two readiness writes
can leave `RPC_PORT=` visible without an owned acknowledgment; consumers must
require the complete authenticated acknowledgment and their existing retention
contract. There is no atomic output-and-signal boundary.

Arbitrarily blocked filesystem/native work has no guaranteed shutdown deadline.
Forced process termination is not observed cessation and retains unresolved
ownership. Passive consumer retention is not model-effect custody or crash
reclamation.

## Runtime shutdown and truthful inference

Shutdown revokes HTTP admission and drains the existing HTTP, catalog, source,
download, conversion, installation, acquisition and managed runtime owners. It
also observes the real `OnnxSessionManager::shutdown`: close operation admission,
wait for outstanding permits (30-second permit-drain grace), then unload sessions.
Managed-profile and ONNX drains are both attempted and their failures aggregated.
The existing external cessation receipt reports any failure before core shutdown;
only complete observed success releases the exact primary generation.

An empty real ONNX manager performs no native SDK/model load. Empty-library
authenticated RPC, missing-model capability/operation refusal, controlled permit
gates and synthetic backend tests qualify their stated lifecycle/parser scopes
only. They do not prove loaded-session disposal, model numerics or inference.
Production audio availability remains false until its separate installed-runtime
and model-read policy is admitted.

## Minimum real inference inputs

Real ONNX acceptance requires an operator-selected trusted `libonnxruntime.so`
supporting C API 24 (qualification pin 1.24.2), its complete native dependency
closure and per-file hashes. Install it beside the exact executable or select its
absolute path through `ORT_DYLIB_PATH`. Cargo and Pumas do not download this SDK.
Also provide an explicitly selected compatible ONNX embedding model package,
`tokenizer.json`, configuration with the required hidden-size metadata, all
external tensor members, immutable source/content hashes, selected library/model
identity, and compatible I/O options. No default model is inferred from a test's
optional fixture path.

Vision separately requires a selected compatible runtime, model and matching
projector. Audio requires an authorized full-revision model and a trusted
installed runtime with an admitted model-read closure policy. Without those
inputs, report real load, inference, numerics and loaded-resource disposal as
unrun. Preserve historical refusal evidence and qualify any successor separately.
