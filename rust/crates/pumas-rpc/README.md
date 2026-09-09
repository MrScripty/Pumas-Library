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
probes. Setup does not enforce exclusion of conversions or external tool users;
callers must provide that exclusion. Setup completion is not GPU or conversion
readiness proof.

Contract export includes `StartBackendSetupParams`, `GetBackendSetupParams`, and
`backend_setup_request_probes` produced by the actual Rust command parser.
Desktop decoding and user-interface integration require their own consumer evidence.
