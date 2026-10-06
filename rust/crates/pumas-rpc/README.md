# Backend setup RPC

## Explicit S3 model import

The optional `s3` Cargo feature composes the existing native
`PumasApi::import_s3_model` operation. It imports one GGUF with an
explicit caller pin, using the existing transfer, verification, model importer,
registration and receipt owners. Default feature selection remains unchanged.
Without `s3`, these commands return the closed `unavailable` outcome.

`start_s3_model_import` accepts only `operation_id` (canonical lowercase UUID),
`endpoint` (HTTPS origin without userinfo, path, query or fragment), `region`,
`bucket`, `addressing` (`path` or `virtual_hosted`), `key`, `version_id`,
`filename` (ASCII GGUF basename), `sha256` (64 hex digits), `family` and
`official_name`. VersionId and digest are required; neither is discovered or
substituted. Unknown fields, credentials and HTTP opt-outs are rejected before
I/O. This anonymous command remains source compatible.

`start_authenticated_s3_model_import` is a distinct additive command. Its closed
params are `{source: <the existing start params>, credentials: {access_key_id,
secret_access_key, session_token?}}`. A missing/null token selects static key
credentials; a supplied token must be nonempty. Each credential value is limited
to 4096 printable ASCII characters without whitespace; access-key IDs also
exclude `/`, `,` and `=`. The receiving owner validates the actual authenticated
reader constructor before admission, then consumes the credentials into the
existing bounded native operation. HTTPS is mandatory. No ambient provider,
profile, account defaults, refresh or anonymous fallback is introduced.

Credential wire types are Deserialize-only, without Debug/Clone/Serialize.
Credentials remain in ephemeral request/worker/reader memory and never enter
snapshots, receipts, model metadata, caches, telemetry or saved configuration.
The dialog uses uncontrolled password inputs, clears them before awaiting submit,
on close, mode replacement and unmount, and never stores them in React task state.
Lost acknowledgements observe the same UUID without replaying credentials.
Generated decoders and IPC make transient copies; secure memory erasure and
protection from privileged process inspection are not claimed.

The existing local Electron IPC/direct loopback HTTP control plane carries this
request; it does not become a remote or TLS RPC service. Credentialed S3 reads
use HTTPS with normal certificate verification. Source transport ignores proxy
configuration, refuses redirects, bounds responses to 64 KiB, correlates JSON-RPC
IDs and projects static failures before IPC. Reflected response/error text never
reaches renderer diagnostics. No listener, TLS trust or security settings change.

`start_s3_model_bundle_import` adds a complete explicit file-set path. It accepts
`operation_id`, `endpoint`, `region`, `bucket`, `addressing`, `family`,
`official_name`, `primary_logical_path` and `files`. The set has 2–32 members,
each exactly `{key, version_id, logical_path, sha256}`. The primary must be a
selected ASCII GGUF basename; other paths must be portable relative paths with
extensions `json`, `txt`, `md`, `model`, `tiktoken`, `vocab` or `merges`, matching
the existing native bundle importer. One key may select different versions;
conflicting evidence for the same key/version, duplicate/colliding paths,
staging aliases and prefix collisions are refused by shared manifest validation.
The actual reader's pure preflight also enforces exact object-key semantics and
the existing 16 KiB encoded revision limit before job/workspace admission.
Importer-owned reserved roots (including metadata/overrides), descendants and
normalized aliases fail before job admission, staging or source I/O. The native
workflow and final copy plan share that exact importer preflight; a corrected
request remains admissible.
This is an explicit set, without prefix enumeration or atomic snapshot claims.
A pinned HEAD with an explicit zero length supports empty auxiliary members
without GET or a byte-range request. The shared writer and SHA-256 verifier
produce the empty file and exact receipt; missing/unknown length is not empty.
The primary still must pass existing GGUF format validation.

`start_authenticated_s3_model_bundle_import` takes
`{source: <bundle start params>, credentials: <the same credential params>}`.
It preserves the one-use credential and HTTPS rules above. Both bundle starts
return the existing `S3ImportOutcome`; existing single-object starts remain
unchanged. Cancellation uses the same `cancel_s3_model_import` UUID command.

`get_s3_model_bundle_import` takes the same optional UUID as the existing getter
and returns `{outcome: S3ImportOutcome, bundle_progress: <progress or null>}`.
It can observe either a single-object or bundle job from the same owner. Only a
running outcome includes aggregate progress: zero-based `file_index` (null
between files), `files_total`, `files_acquired`, decimal-string `bytes_acquired`,
`total_expected_bytes` (null until the complete selection resolves) and
`total_bytes_observed`. File indices follow logical-path order. Total observed
bytes combine acquired staging bytes with current-file attempt bytes; retries
can lower the latter, so this is not a monotone network-byte counter. Acquired
files/bytes are individual staging observations; complete-set verification,
publication, registration and receipt settlement remain separate. Neither full
byte counts nor cancellation acknowledgement mean the model was published.
Terminal results preserve the existing possible publication ID and retained-work
semantics. Progress includes no keys, source endpoints or credentials.

The dialog adds up to 31 explicit auxiliary rows and observes aggregate progress
through the additive getter, including when reopened. Removing a row changes
only the unsubmitted draft. Every selected file needs its own immutable pin and
expected digest; credentials still clear before any admission wait.

`get_s3_model_import` accepts optional/null `operation_id`; omission reads the
one retained process-local job. `cancel_s3_model_import` requires its exact UUID
and returns `{accepted, outcome}`. Cancellation acknowledgement is not proof
that work has stopped. Finalization closes cancellation admission. Closing the
dialog or dropping an HTTP request stops observation only; server shutdown
closes admission, requests cancellation and awaits the operation before shared
acquisition drainage.

Outcomes are `unavailable`, `idle`, `not_found`, `rejected`, `running` or
`finished`. Running progress contains native phase and a decimal-string
`downloaded_for_current_file`, preserving u64 counts without claiming percentage,
verification or completion. Finished results are `completed {model_id}`,
`cancelled {retained_work}` or `failed {error, retained_work,
published_model_id}`. Errors are the existing closed redacted public projection.
Completed means the native receipt settled and the owned input reservation was
cleaned. A failure may preserve a published model ID; inspect the library before
any recovery. Acknowledgement, final phase and byte counts are separate evidence.

One process-owned worker admits one operation at a time and retains only the
latest safe result. Repeated UUIDs, active work and retained failure/cancellation
refuse resubmission. Results are not restart-persisted history: after restart or
replacement, an exact-ID read can return `not_found`; check the library and
reconcile existing custody rather than replaying with a new UUID. Existing
`model.s3.workflow` Using custody also blocks a new desktop import.

The RPC caller selects finite defaults of three attempts, a 600-second
acquisition elapsed budget and a 30-second source-operation timeout. Existing
native retry and verification policies are unchanged. Staging is reserved under
`launcher-data/.s3-import-<UUID>` through held directory identities, outside model
discovery. Broad custom model-library layouts containing launcher-data are
refused before allocation. Failed/cancelled stages are retained for exact
reconciliation; the dialog does not offer an unsafe reset or implicit replay.

Rust owns the wire schemas; generated Electron/frontend decoders enforce closed
request/response shapes. The renderer polls serially every 500 ms only while
running, fences stale responses when a command or replacement observer wins,
and can reopen a retained result without another start. Controlled Linux HTTPS,
RPC and DOM/preload fixtures provide local support; real-browser, packaged,
default-inference, cross-platform and live-provider acceptance remain separate.

## Backend setup

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
