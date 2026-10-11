# Model download terminal feedback

The user reported CLM-v0.1-8B-GGUF Q8_0, 7.69 GB appearing to complete, followed
by a dim download indicator instead of an enabled play action. Read-only
observations of the running backend establish that this transfer failed; they
do not establish an inference-ready model.

## Observed state

- Exact download: `c7c3ea45-2064-4c0b-9fe4-1ea84c43dfc4`.
- Selected file: `Qwen3-8B-Q8_0-outq2.gguf`, expected size 8,252,495,488 bytes.
- The live download snapshot reports `error`, 8,149,772,661 transferred bytes,
  and progress `0.9875525236129761`. Its public error is
  `The model download did not complete successfully.`
- The application log records an `artifact.http.response` validation failure:
  `HTTP continuation representation or resource changed`.
- The current catalog reports the exact selected artifact as partial. The
  separate 8.11 GB weight remains a different model ID; it was not selected.
- Laya has a complete catalog entry, adopted acquisition, and durable completion
  receipt. It is not the download identified by the user.

Queries used SQLite read-only mode and the existing localhost `get_models` and
`list_model_downloads` operations. No download command, model-file copy, hash,
deletion, restart, or live-library repair was performed.

## Reproduction and cause

`pnpm exec vitest run src/hooks/useModelDownloadLibrary.test.tsx` drives the real
download subscription hook, model catalog fetching/projection, completion
refresh, exact-ID activity merge, and rendered library controls. Before the fix,
the failure assertion fails because the selected row has no accessible error
alert. The genuine-completion scenario already passes.

`useModelDownloads` owns errors keyed by the exact download presentation key.
The merged catalog row retains that key, but `LocalModelRowState` previously
looked only for inactive partial-recovery errors keyed by model ID. Consequently
a transfer error could change the ring's appearance without displaying its
failure. An unassociated activity row also omitted its error.

The row now projects the error using its exact `downloadKey` while activity is
present. Both associated catalog rows and unassociated activity rows show an
accessible alert. Failure and command-error labels remain distinct; retries
use the existing exact download ID and authoritative subsequent snapshots clear
the error. Inactive recovery warnings retain their separate ownership.

## Verification and limits

The regression covers the observed failure, retry identity, a same-name sibling,
error clearing after a new transfer snapshot, and unassociated activity. It
also proves that 100% transfer progress alone does not enable play; confirmed
completion followed by a complete catalog enables play for only the selected
artifact. An incomplete refreshed catalog keeps play disabled.

Full frontend suite: 939 passed, one existing skip, 138 passing files. Typecheck
and lint passed; build/installer outcomes are recorded in the execution ledger
when completed. Coding standards MCP snapshot
`snapshot:v1:cb98848b-5ab6-414b-8087-f1cf62578483` selects 33 policies with no
unresolved routing categories; selected text matches the previously read
standards. No persisted or RPC contract changed.

The cause of the HTTP continuation mismatch is not established by this UI
reproduction. This change exposes the existing refusal and does not bypass it,
invent a completion receipt, or automatically retry the live transfer. Download
location reconciliation and library-lock scope remain separate pending work.
