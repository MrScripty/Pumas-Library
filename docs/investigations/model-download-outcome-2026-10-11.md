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


## Resume policy follow-up

The user subsequently resumed the same transfer in the still-running older
installed process and observed it starting again. The original failed transfer
reported 8,149,772,661 bytes; the later paused partial had 91,437,494 bytes. The
selected file remains `Qwen3-8B-Q8_0-outq2.gguf`, not the 8.11 GB sibling.
These observations do not recover the overwritten prefix or establish why the
original server representation check changed.

The cause of the destructive restart is established separately in
`acquisition/service.rs`: a continuation checkpoint holds a weak reference to
one workspace capability. HF pause/error recovery reconstructs that workspace.
A missing or rejected checkpoint selected offset zero, and a successful full
response called `open_part(..., false)`, truncating the retained partial. The old
HTTP adapter also accepted a matching-validator full response to a range request
as permission to replace the prefix. The contract and existing tests explicitly
required these behaviors.

The user revoked that discard policy. Resume now binds an actual regular-file
partial through the current held workspace and uses its observed offset when
the exact selected manifest supplies a whole-file SHA. The assembled download
must verify the supplied SHA before publication. Local prefix observation is
not a successful download receipt, historical validator, or verified-chunk
claim. Valid live checkpoints retain conditional HTTP validation. S3 continues
its exact version/conditional selection and supplied digest checks. An ignored
range, invalid range, absent continuation evidence or oversized partial is
refused with bytes retained. Fresh remote writes use an empty append descriptor,
so they cannot truncate an existing or late-arriving partial. Owner-materialized
local installer staging is not a remote download and retains its own behavior.

HF reports a bounded continuation-refusal message through existing progress
projections; raw response diagnostics stay local. No RPC shape, store schema,
receipt version, checksum retention or already-acquired-file policy changes.
No HTTP validator/access URL/prefix sidecar is persisted, and no execution lease
is retained just to keep a checkpoint alive.

The initial actual HF immediate-resume regression failed against the old code:
the captured request omitted `Range` instead of requesting `bytes=5-`. The
updated fixtures observe exact HTTP/S3 range offsets and exact final/retained
bytes across pause, interruption, retry and owner restart. Negative acquisition
fixtures refuse ignored/wrong ranges and corrupt assembled contents without
issuing a consumer receipt. The public HTTP RPC fixture includes cold recovery,
interruption, pause/resume, final range transfer and durable importer publication.
Verification results and rebuilt installers are recorded in the execution ledger
when completed. Standards snapshot
`snapshot:v1:8232afb5-be9d-4ee8-8099-ce42f109e2ea` selects 33 policies with no
unresolved routing categories.

The running installed application and actual library were not restarted,
retried, repaired, copied, or hashed during this implementation. The rebuilt
candidate must replace the old process to change its policy. Already overwritten
bytes are not recovered by this fix. Download-location reconciliation and root
lock scope remain separate pending work.


### Accepted resume verification

- Default core package: 2,331 passed, 30 existing opt-in tests ignored across
  unit, integration and documentation suites (the unit suite is 2,103/14).
- Headless core unit suite: 2,074 passed, 13 existing opt-in tests ignored.
- S3 acquisition unit coverage: 215 passed, two opt-in tests ignored; S3 reader
  and acquisition integration suites: 58 passed.
- Default RPC package: 438 passed, 12 opt-in tests ignored, including all 410
  binary unit tests and the controlled public HTTP resume/import flow.
- Default S3 RPC selection: 17 passed. The separately gated headless S3 facade
  suite also passed 17 tests, with its existing browser-coordinator test ignored.
- Default core/RPC all-targets Clippy, the CI-selected headless S3 core Clippy,
  and default-feature S3 RPC all-targets Clippy passed with warnings denied.
  Formatting and production frontend/Electron builds passed.

An extra combined no-default core/RPC S3 all-targets Clippy experiment reported
11 dead-code warnings in existing test-only inference contract declarations.
Their feature guards are unchanged from the admitted slice baseline
`6761244b`. The repository's strict S3 CI lint gate selects the core package
separately; default-feature RPC lints pass. No warnings were suppressed. Strict
headless RPC inference-test gating remains separate cleanup, and is not claimed
qualified by this resume change.

Initial broad debug builds exhausted the filesystem and produced link/write
failures; those runs are invalid acceptance evidence. Only task-generated
compiler output was removed. Final verification used two compiler jobs, disabled
incremental compilation, and debug level zero for development/test artifacts;
the optimized installer backend retains the normal release profile. Logs are
in ignored original-repository `tmp/pr69-resume-policy-*.log`. No runtime
inference or native Windows/macOS execution was performed.
