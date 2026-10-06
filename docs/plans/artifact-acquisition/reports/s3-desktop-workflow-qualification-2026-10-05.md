# Anonymous desktop/RPC S3 workflow qualification — 2026-10-05

## Exact scope and ancestry

Branch `feat/s3-desktop-workflow-eabc3959` starts exactly at frozen native workflow
`eabc395966f2f3f155b298144bc81c4941121225`, tree
`ec838d4ebb4a0538b0c2822b4a5a3bf9ee73cb3b`. Tested implementation commit is
`04cb954c3cd226144fcd5dde2d19ac040bda4d78`, tree
`011160b67e12840d33481f760b16c1e78224f928`.
Its ancestry preserves reader `ffecce07220f6504fb1db2e348759473bd284de6` and
PR40 `63123fd8f9f866064a8315096ab3fb1fc81e8f0f`, whose tree remains
`163ef2442695b44555ac0e42ff79a6a88de9e817`. Accepted main remains
`05717338c2aea737483fb4b28c3ed4053a65de96`.

The coordinator froze the native workflow for independent review and admitted
the next Q3 direct source-facing desktop/RPC composition. Inspection selected
one anonymous pinned GGUF as the concrete small scope, reported before changes.
Authentication is deliberately absent from this wire/UI: a qualified ephemeral
secret boundary across generic RPC/IPC request handling needs its own slice.
There are no credential fields, defaults, discovery, account setup, telemetry
serialization or persisted source settings. Native callers retain the already
frozen explicit in-memory authenticated API. No real credentials or accounts
were used; all endpoints and fixtures are synthetic/disposable.

No AGENTS.md or local `.agents/skills` exists in this checkout. CONTRIBUTING's
canonical Coding-Standards checkout is pinned at
`188beda1fa477d21d576c233dd8c7f4c4c267d23`. The complete route and snapshot
cover frontend/library, Rust API/async/security, TypeScript/async, IPC/interop,
accessibility, ownership, verification, documentation and commit requirements.
Raw routing evidence is retained alongside test logs.

## Composition and public surface

RPC adds optional `s3 = [pumas-library/s3, cap-std, same-file]`; default inference
feature selection is unchanged. The two production dependencies and test-only
native-tls already exist in Cargo.lock. Resolved name/version/source/checksum
identities remain exactly equal to the base. Notice regeneration independently
produces identical package records and notice bytes; only RPC Cargo-manifest and
lockfile input hashes change. Four dependency graphs establish that object_store
appears only with explicitly enabled RPC S3, with and without inference defaults.

Three closed commands expose the existing native operation:

- `start_s3_model_import`: canonical UUID, explicit HTTPS origin, region, bucket,
  path/virtual-hosted addressing, exact key, immutable VersionId, ASCII GGUF
  filename, expected SHA-256, family and official name. All facts are required;
  credentials, extra fields, plaintext opt-outs and origin access material are
  refused before I/O. No Debug/Serialize derives exist on request facts.
- `get_s3_model_import`: optional/null UUID, observing one process-local job or
  its retained safe result. Reads neither start nor replay work.
- `cancel_s3_model_import`: exact UUID; `{accepted, outcome}` separates
  cancellation admission from owned completion. Finalization refuses cancellation.

The typed outcome distinguishes unavailable, idle, not found, rejected, running
and finished. Running is native phase plus decimal-string current-file bytes;
the count can reset and is not verification, percentage or registration proof.
Finished distinguishes registered model ID, cancelled/retained custody, and
redacted failure with retained custody and any preserved published model ID.
No-S3 builds expose the same wire with typed unavailable outcomes.

One process-owned RPC worker awaits `PumasApi::import_s3_model`. The existing
acquisition consumer owns selection/transfer/verification and the existing model
importer owns classification, validation, registration and exact receipt
settlement. RPC supplies a held-identity directory reservation under
`launcher-data/.s3-import-<UUID>`, outside discovery. Layouts containing that
root in the model library are refused before allocation. Creation and successful
cleanup run under the existing bounded blocking scope. Settled inputs are cleaned;
failed/cancelled inputs remain available for exact reconciliation. No second
transfer lifecycle, persistence schema, signer or publication protocol is added.

Desktop-specific finite defaults are three acquisition attempts, 600 seconds
elapsed and 30 seconds per source operation. Existing native policies are
unchanged. Duplicate UUIDs, active jobs and retained failure/cancellation refuse
admission. Existing native Using custody blocks new desktop work. Results are
latest-only and process-local, not restart history: missing exact-ID observation
requires library inspection/reconciliation, never an automatic new UUID replay.
Server shutdown closes admission, requests cancellation and awaits this worker
before shared acquisition drainage. Dropped HTTP requests/dialogs stop only
observation. A poisoned observation lock still revokes admission on shutdown;
it does not reopen service or claim repaired state.

Rust remains the canonical schema/export owner; generated frontend/Electron
types and decoders are produced by the existing generator. IPC validates exact
request shapes; bundled preload decodes both directions. Model Manager adds an
anonymous dialog with labelled required fields, explicit addressing choice,
honest observation/cancellation/result text and existing modal semantics.
One renderer observer polls serially at 500 ms only while running, fences
superseded responses, clears timers on close and can reopen retained results.
An ambiguous admission observes the same UUID without another start. Known
finished results clear an earlier unknown-acknowledgement warning.

## Local supporting evidence

Environment: managed Linux x86_64; Rust/Cargo 1.92.0, four Cargo jobs; Node
24.19.0 and project-pinned pnpm 10.33.0. Existing installed pnpm 11 attempted a
home-directory store and was unsuitable; the pinned tool uses a workspace store.
Installation honors the unchanged lockfile and ignores install scripts.
Dev/test Cargo debug info and incremental compilation are disabled to bound
task-generated disk use. Registry/configuration roots are disposable.

| Check | Local result | Evidence |
| --- | --- | --- |
| RPC package, no inference defaults + S3 | 195 unit, 16 integration, 2 intent tests pass; 12 existing ignored/live cases | `rpc-tests-all-final.log` |
| Isolated production HTTPS/RPC child | Pass; single import, stalled HEAD/GET cancellation and HEAD shutdown | Same log, additional child execution |
| No-S3 command regressions | 3 pass, including typed unavailable and zero acquisitions | `rpc-headless-source-tests-final.log` |
| Headless RPC compile | Pass without S3 | `rpc-headless-check.log` |
| Full frontend suite | 856 tests / 130 files pass | `frontend-tests-all.log` |
| Final focused renderer regressions | 8 pass after the final finished-result warning correction | `frontend-source-tests-qualified.log` |
| Full Electron Node suite | 234 pass, 1 existing native sandbox smoke skipped | `electron-tests-all-final.log` |
| Frontend/Electron types, lint and builds | Pass; default/library-only Vite and bundled preload builds | `frontend-types-complete.log`, `frontend-lint-qualified.log`, `frontend-build-default.log`, `frontend-build.log`, `electron-types-3.log`, `electron-lint.log`, `electron-build.log` |
| Canonical contract generator/check | Fresh outputs; 8 generator tests pass | `contract-check.log`, `contract-generator-tests.log` |
| Clippy no-default + S3/export/test-support | Pass with the existing dead-code lint excepted; all other warnings denied | `rpc-clippy-existing-dead-code-exception.log` |
| Formatting, diff, feature/ref/frozen-file checks | Pass | `fmt-qualified.log`, `diff-final.log`, `feature-boundaries.json`, `frozen-state.json` |
| Attribution generation/check | Pass; identical notices/package entries | `attribution-generation.log`, `attribution-check-final.log` |

The full frontend run precedes the small final warning correction; the final
hook/dialog suite and lint are rerun afterwards. The final no-S3 fixture compile
and pass are separately recorded. Earlier fixture compile errors, request/error
assertion corrections, initial stale attribution refusal and lint corrections
remain in the raw logs; the table identifies the passing results.

The HTTPS source uses the existing synthetic localhost PKCS#12 fixture and PEM
trust anchor in an isolated child. Normal TLS verification stays enabled. Actual
RPC HEAD/GET requests preserve VersionId and remain unsigned even with synthetic
ambient AWS values. Ready metadata, public model lookup, exact output bytes,
matching demand/manifest/owner completion receipt and cold library lookup are
checked. Cancellation drains stalled connections and leaves no model; server
shutdown also drains an owned request. All owned files are scanned for synthetic
ambient credential values. No production HTTP/TLS policy is weakened.

Renderer tests use DOM fixtures, not a browser proof. They cover required source
fields, anonymous-only content, exact facts/UUID, pre-admission refusal, duplicate
start suppression, cancellation acknowledgement, stale status fencing, no polling
after unmount, reopening without admission, ambiguous admission without replay,
correlated results, retained-work refusal and unavailable builds. Electron tests
exercise actual compiled preload and IPC decoders, including closed credentials
refusal, decimal counts and typed results.

## Concrete limits and next planned feature

Real UI execution is blocked in this environment: launching `/usr/bin/chromium`
with `chromium_sandbox=True` aborts because the installed SUID helper is not
properly configured (requires root ownership/mode 4755). The crashpad path also
reports missing configuration. No sandbox flags, permission change or security
bypass is used. `browser-capability.log` contains the exact failure. No browser
focus, keyboard, visibility, geometry or packaged Electron claim is made.

Default inference Clippy cannot build because ort-sys's normal prebuilt artifact
download from cdn.pyke.io returns HTTP 403; `rpc-clippy-default.log` records it.
Strict headless Clippy without an exception is blocked by unmodified Torch DTO
dead-code and the existing integration diffusers fixture helper, recorded in
`rpc-clippy.log`, `rpc-clippy-export.log` and `rpc-clippy-bin-final.log`. These
are not corrected in the frozen/unrelated write sets. The passing run explicitly
uses `-D warnings -A dead_code`; it does not establish a fully strict/default
matrix. Headless compilation and behavioral tests remain useful local evidence.

Core/app-manager, all reader/manifest/signing, discovery/watcher/importer,
reconciliation, staged VersionId repairs and hosted workflow bytes are unchanged
from the exact base. Frozen reader/native branches and main are preserved.
No old saved environment seed, real credentials, paid service, network/security
bypass, external reviewer contact or PR/merge action is used. Parent owns review,
hosted/default/platform/browser qualification, PRs/merges and Library delivery.

The next existing-plan Q3 feature is authenticated direct-source RPC/desktop
configuration with a qualified ephemeral secret boundary. Live AWS, one non-AWS
compatible provider and local MinIO acceptance remain separately required, as
does Q4 packaged/independent/native qualification. Native publication/discovery
corrections remain on their independently coordinated branch. AC13/AC14 and
AQ-S3 remain pending.

Raw evidence is under `/workspace/scratch/s3-desktop/`, including standards route,
command/log status inventory, hashes, branch/ref checks and commit/push records.
The subsequent documentation-only commit records this report; its exact final
head/tree and remote equality are saved in `final-state.json` after publication.
