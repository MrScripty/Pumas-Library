# Backend setup RPC

The RPC adapter exposes the standalone library's existing setup owners. It does
not own another installer lifecycle.

- `get_backend_setup`: requires `backend`; reads the latest owner-local snapshot
  without installing, probing readiness, or touching disk.
- `start_backend_setup`: requires `backend`, with optional/null
  `expected_previous_operation_id`. Omission attaches or starts only when no
  record exists. Only the current terminal operation's canonical UUID authorizes
  a retry; stale IDs return the selected owner's current record.

Backend values are exactly `python_conversion`, `llama_cpp`, `nvfp4`, and
`sherry`. These new methods reject legacy aliases and extra fields. Malformed
IDs and retry tokens without a selected owner-local record are invalid parameters
(`-32602`). Closed setup admission returns the existing cancellation code (`-32004`).

Responses reuse the validated conversion setup started/status outcomes, including
redacted failure text. They do not echo the backend: callers retain the selected
backend with their correlated request and snapshot. Snapshots are latest-only,
not persisted history, and remain readable after setup admission closes.
`success: true` acknowledges a valid request/observation, not a successful
installation; inspect `setup.status`. A null status snapshot is not readiness.

Existing base-Python setup methods remain unchanged. The server's existing
shutdown drain closes and observes built-in installers and asynchronous readiness
probes. Setup and managed conversions share exclusive root-level environment
access through cleanup and publication. Contention fails the operation in its
setup snapshot or conversion progress, without queuing or automatic retry.
Direct backend execution, independent readiness probes and external tool users
remain caller-coordinated; see the [core contract](../pumas-core/README.md).
Setup completion is not GPU or conversion readiness proof.

`get_backend_status` reports llama.cpp not ready while a recorded incomplete
native setup remains, including after process restart. Quantization also refuses
that environment. Only successful explicit setup clears invalidity; reading
status does not install or repair. Payloads and setup receipt semantics are
unchanged. See the [core contract](../pumas-core/README.md) for marker ownership,
inspection errors, direct-use coordination and persistence limits.

`check_conversion_environment` uses the retained base Python readiness owner.
Missing interpreter and normal nonzero import exit produce `ready: false`;
infrastructure, signal, deadline and cleanup failures produce the existing
redacted operation-failure error (`-32003`), not successful false or an internal
I/O error. Closed/cancelled reads produce `-32004`. Successful payload shape is
unchanged. Server shutdown drains these probes even if their request was dropped.

Contract export includes `StartBackendSetupParams`, `GetBackendSetupParams`, and
`backend_setup_request_probes` produced by the actual Rust command parser.
Desktop decoding and user-interface integration require their own consumer evidence.


## Local Intent RPC

The existing loopback server projects the native core intent service. Requests
use the existing JSON-RPC envelope and these method-specific `params` objects:

| Method | Params | Typed result |
| --- | --- | --- |
| `intent_query_models` | `{ "requirement": ModelRequirement }` | `QueryModelsOutcome` |
| `intent_get_model` | `{ "requirement": ModelRequirement }` | `GetModelOutcome` |
| `intent_get_model_status` | `{ "requirement": ModelRequirement }` | `ObservedModelState` |
| `intent_ensure_model` | `{ "request": EnsureModelRequest }` | `EnsureModelOutcome` |
| `intent_release_model` | `{ "reference": ModelEnsureRef }` | `ReleaseModelOutcome` |
| `intent_get_ensure_status` | `{ "reference": ModelEnsureRef }` | `GetEnsureStatusOutcome` |
| `intent_list_declarations` | `{}` | `ListModelDeclarationsOutcome` |

For example, a local observational request is:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "intent_get_model",
  "params": {
    "requirement": {
      "selector": {
        "kind": "upstream_repository",
        "repository_id": "ggml-org/test-model-stories260K"
      },
      "acquisition_policy": "local_only"
    }
  }
}
```

Results are the serialized domain outcomes directly in the JSON-RPC `result`.
A valid request can report invalid requirements, missing/incomplete artifacts,
ambiguity, blocked work, or unavailable upstream access. Inspect the outcome;
a successful transport response does not prove model availability. Malformed
wire shapes are invalid parameters; infrastructure failures use the existing
redacted public errors.

Query and status are local observations. Get may admit or join a managed download
when its requirement allows upstream acquisition; `Acquiring` carries a pinned
`resolved_requirement` for subsequent status queries. Ensure durably records a
consumer requirement before convergence. Retain the returned declaration
reference to read its status or release its exact generation. Release neither
deletes the model nor cancels downloads. Disconnecting a client does not release
its declarations or cancel admitted work. The server's existing shutdown drains
the composed intent owner and downloads.

See the [core contract](../pumas-core/README.md#local-intent-interface) for supported
single-artifact layouts, retry/restart behavior, retention and upgrade limits.
The same domain types are available through native Rust and existing authenticated
IPC. Operational RPC methods and desktop allowlists retain their existing
contracts. This adds no production listener, node, fleet, discovery, or remote
control feature.
