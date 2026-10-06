# Authenticated S3 desktop/RPC qualification — 2026-10-05

## Exact candidate and preserved history

Branch: `feat/s3-desktop-auth-106a6ca4`.

- Frozen anonymous base: `106a6ca40ac41717854d847214ee3a1cf1e673b4`, tree `ff61e5b4df248fa31731abcaf2dbdccb17430860`.
- Separately published review-fix ancestor: `887308dc094570fe6072b162074294c3dc580cd0`, tree `89abc11e201b75083df47985dc7d8314d9ef88ec`. See [review corrections](s3-desktop-review-fixes-2026-10-05.md).
- Tested transport prerequisite: `0023bcf9954f99c2755395d82b8daa632f73bfcd`.
- Authenticated implementation: `39f7689ab20a10276839a8f5dff206879d53b008`, tree `01f49715a2b3637826591a078134db0bc59f62ca`.
- Original PR40 ancestor: `63123fd8f9f866064a8315096ab3fb1fc81e8f0f`, tree `163ef2442695b44555ac0e42ff79a6a88de9e817`.
- Main remains `05717338c2aea737483fb4b28c3ed4053a65de96`; frozen reader `ffecce07220f6504fb1db2e348759473bd284de6`, native workflow `eabc395966f2f3f155b298144bc81c4941121225` and anonymous refs are unchanged. Parent's separate PR40 branch advanced to `1e811936870635d8283e1a3709649877e4647130` at final read-only snapshot; this successor neither fetches/integrates nor alters it.

Core/app-manager, reader/signing/manifest, importer/watcher/discovery/native
repair, hosted workflow, Cargo manifests/lockfile and package/dependency bytes
are identical to the frozen anonymous base. No new dependency or feature policy
is introduced. The documentation milestone following the implementation records
this report; `final-state.json` binds its final head/tree and remote equality.

## Scope and public surface

The additive `start_authenticated_s3_model_import` command accepts a closed
`{source: S3ImportParams, credentials: S3CredentialParams}` object. Source facts
are unchanged. Access key and secret are required, with optional omitted/null
session token. Nonempty printable ASCII credentials are bounded to 4096 bytes
per value; access keys exclude SigV4 delimiters. The actual HTTPS-only native
constructor preflights before admission/workspace creation, using bounded
transient copies, then owned originals move into the existing native request.
Credential DTOs have no Debug/Clone/Serialize implementations. Three anonymous
start/read/cancel commands and their outcome schema remain compatible; builds
without S3 return typed unavailable for the new command too.

Rust exports the two new DTO schemas into the six canonical generated desktop
files; generated copies explain the validator diff size. Preload, IPC registry,
ImportAPI and the dialog hook gain the distinct authenticated entry. Credentials
are ephemeral arguments rather than task state, refs, defaults or recovery data.
Uncontrolled password inputs clear synchronously before awaiting submit, and on
close, mode replacement and unmount. Lost admission acknowledgement observes the
same operation UUID without credential replay or anonymous fallback.

Before this command was implemented, a separate prerequisite contained raw
bridge failures and decoded source responses in privileged main before IPC.
All source methods use static transport/validation/error text, a 64 KiB response
budget, exact JSON-RPC correlation, bounded existing deadlines, a direct Node
HTTP agent with proxy configuration disabled, and no redirect following. Existing
loopback HTTP and Electron IPC carry transient request copies; credentialed S3
reads remain normally verified HTTPS. Secure memory erasure and privileged
process-inspection resistance are not claimed. [Boundary trace](s3-credential-boundaries-2026-10-05.md)
records every serialization, error/log, persistence and frontend lifetime owner.

## Local verification

Linux x86_64, Rust/Cargo 1.92.0, Node 24.19.0, pinned pnpm 10.33.0, four Cargo
jobs, dev/test debug and incremental disabled, task-owned
`XDG_CONFIG_HOME=/tmp/pumas-s3-auth-config`. CONTRIBUTING routes to standards
snapshot `188beda1fa477d21d576c233dd8c7f4c4c267d23`; route and read inventory
are in `standards-route.json` and `standards-snapshot.json`. No AGENTS or local
.agents/skills files were present. Logs below are under
`/workspace/scratch/s3-desktop-auth/`.

| Check | Observed result | Passing log |
| --- | --- | --- |
| S3 RPC full suite | 199 unit + 16 integration + 2 intent pass; 12 existing ignored/live cases | `auth-rpc-tests-all-final.log` |
| Controlled authenticated production HTTPS/RPC | Success without/with token, stalled HEAD/GET cancellation, reflected provider failure; captured logs/owned files/safe snapshots and receipts exclude synthetic credentials | Same full-suite log; isolated child output captured and scanned by parent fixture |
| Existing anonymous production HTTPS/RPC | Existing VersionId import/cancel/shutdown regressions pass unchanged | Same full-suite log |
| No-S3 source regressions | 4 pass, including authenticated typed unavailable | `auth-headless-tests-final.log` |
| Frontend full suite | 860 tests / 130 files pass | `auth-ui-tests-all.log` |
| Final DOM/hook regressions | 17 tests / 3 files pass after final test-only lint correction | `auth-ui-tests-qualified.log` |
| Electron Node full suite | 239 pass; one existing native sandbox smoke skipped | `auth-electron-tests-qualified.log` |
| Actual local source transport | 3 pass: reflected/malformed/oversize errors, privileged containment, proxy-configured global agent ignored and redirect refused | `auth-transport-constraints.log` |
| Canonical contract check/generator tests | Fresh canonical outputs; 8 generator tests pass | `auth-contract-check.log`, `auth-generator-tests.log` |
| Types/lint/builds | Frontend and Electron pass; default/library-only Vite and bundled preload built | `auth-ui-types-qualified.log`, `auth-ui-lint-qualified.log`, `auth-ui-build-default.log`, `auth-ui-build.log`, `auth-electron-build-final.log`, `auth-electron-lint-qualified.log` |
| All-target S3 Clippy | Pass with existing dead-code lint explicitly excepted | `auth-clippy-final.log` |
| Format/diff/frozen-path/ref checks | Pass | `auth-fmt.log`, `auth-diff-check.log`, `auth-frozen-state.json` |

Commands: `cargo test --locked --offline -p pumas-rpc --no-default-features
--features s3`; `cargo test --locked --offline -p pumas-rpc
--no-default-features source_`; `cargo clippy --locked --offline -p pumas-rpc
--no-default-features --features s3,export-contract,test-support --all-targets
-- -D warnings -A dead_code` (from `rust/`). Frontend: pinned pnpm `test:run`,
`check:types`, `lint`, `build`, `build:library-only`, and final focused Vitest
hook/dialog/modal tests. Electron: `build`, `lint`, compiled
`node --test tests/*.test.mjs`, canonical generator `--check`, and its eight tests.

The existing synthetic localhost certificate/trust anchor is used in isolated
children with normal verification. HEAD/GET have SigV4 authorization with the
explicit synthetic access key; secret is absent from headers; a supplied token
appears in the sensitive token header and signed-header list, and omission has
neither. Synthetic ambient credentials are ignored. The frozen reader's
independent cryptographic signing fixtures remain separately qualified; this
successor verifies composition rather than replacing that oracle. Exact pinned
VersionId, GGUF bytes, Ready/registered model and manifest/demand/owner completion
receipt are checked. All disposable owned files and DEBUG tracing output are
scanned for explicit synthetic access/secret/token values. Provider failures
reflect these values in their fixture body; public diagnostics remain static.
Cancellation leaves no published model and owned operations are drained.

Earlier compile/test/lint failures are retained: initial fixture attempted to
serialize a deliberately non-serializable admission error; one UI expectation
included the new checkbox label; project lint refused non-null assertions/generic
errors in tests. Final logs identify their corrected results. The full frontend
run preceded only the final test assertion lint correction; focused tests,
types/lint/builds passed afterwards. The final Electron suite includes the last
proxy/redirect regression.

## Limits and next existing-plan feature

The frozen anonymous qualification already records sandboxed Chromium startup
failure (misconfigured installed SUID helper/crashpad), ort-sys's normal artifact
HTTP 403 blocking default inference Clippy, and pre-existing headless Torch/fixture
dead-code blocking fully strict Clippy. These remain unchanged; no bypass or
unrelated fix is attempted. Passing Clippy explicitly excepts dead_code. DOM
fixtures are not browser focus/layout/keyboard or packaged Electron evidence.
Parent owns supported hosted browser/default/platform qualification, independent
review, PRs/merges and Library delivery. No real account credentials, live provider,
provisioning, paid service or external reviewer contact occurs.

This completes the admitted authenticated single-object desktop/RPC slice.
The next existing-plan Q3 implementation candidate is explicit pinned multi-file
source selection through the existing native GGUF-plus-auxiliary workflow,
without changing model format or publication ownership. Real AWS/non-AWS/MinIO
acceptance, credential refresh and Q4 installed/native qualification remain
separate; AC13/AC14 and AQ-S3 are not advanced.
