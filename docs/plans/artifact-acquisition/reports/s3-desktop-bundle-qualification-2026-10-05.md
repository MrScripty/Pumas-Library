# Pinned multi-file S3 desktop qualification — 2026-10-05

## Exact lineage and accepted composition

Branch: `feat/s3-desktop-bundle-d30412e9`.

| Milestone | Commit | Tree |
| --- | --- | --- |
| Frozen authenticated base | `d30412e94e867a9561afbbcb3d713d6ac6c88ae1` | `245b5fb0d544a1cd103bb757501e43487424bcf0` |
| Tested core preflight/progress prerequisite | `dd90f416a958013b29949b278aa72e9765befa69` | `1fd179a61a2c2c3f8e31040b8b229dd0ebf00bed` |
| Tested desktop/RPC bundle implementation | `339032ff4acb53e43aa36c6edf96739101121620` | `8d08ba41f12a358ed07a19095a96161847a6f191` |
| Accepted main / PR40 | `838eb2990905144a59830f1a16fe91b4e1105d4d` | `f6d6c1d3c5ba5be0fb998a6fd157fcb31c63dc72` |
| Explicit normal composition merge | `d9d907cfe0e57b1e2bc7e296eff1327de835bc5e` | `82511113e7e82a44ee526f91a8ce7f480282e0ec` |
| Qualified empty-member refusal fixture/text follow-up | `5104a21ed4d6162e3506db0e3da43cf78201741f` | `a763ed439181876e591501ec616305dcd6d97bee` |

The composition's ordered parents are **339032ff then 838eb299**. Accepted main's
ordered parents are **05717338 then 1e811936**. Main was fetched and exact head,
tree and parents verified before the coordinator-authorized normal merge; no
cherry-pick, history flattening or force push was used. All frozen reader,
native workflow, anonymous, review-fix and authenticated commits remain separate
ancestors and their branch refs remain unchanged. Original PR40 `63123fd8`, tree
`163ef2442695b44555ac0e42ff79a6a88de9e817`, is preserved in the lineage.

Before composition, every PR40 repair path was byte-identical to frozen d30412e9.
The merge introduces only accepted PR40 publication/discovery changes and its
report, without modifying their implementation. Desktop/RPC/dependency bytes are
identical between 339032ff and the merge. Final integration now includes PR40's
accepted repairs; it must retain this composition. `composition-state.json` and
`composition-paths.txt` bind the exact evidence. A following documentation commit
records this report; `final-state.json` binds its final head/tree and remote match.

## Concrete scope and APIs

Scope was reported before implementation: reuse the existing native explicit
manifest, acquisition consumer, GGUF-plus-auxiliary importer, model registration,
receipt and cancellation owners; add only complete-set preflight and accurate
observation before exposing the desktop inputs.

The reader adds pure `S3Reader::validate_manifest_entries(&[S3ManifestEntry])`.
It reuses the reader's exact object validator and shared ArtifactFile/Manifest
validators, including exact key/version evidence and the 16 KiB encoded revision
budget. The temporary source label grants no selection/access/verification
capability; native selection remains authoritative. Its existing private
`s3/manifest.rs` implementation is untouched by this feature. Namespace, staging
aliases, duplicates, prefix/case collisions and conflicting same-version digest
pins refuse before worker/workspace admission. Different versions of the same
key remain distinct. Actual object existence, size and digest correctness are
established by the existing resolver/transfer, never by preflight.

`AcquisitionHost` gains default no-op `file_started(index)` and
`file_acquired(index, bytes)` observations around its existing sequential loop.
There is no additional transfer/retry/custody writer. Native
`S3ModelImportControl::subscribe_bundle()` and root-exported
`S3ModelBundleProgress` add a separate safe watch channel. Existing native request,
progress struct and subscribe APIs are unchanged. Explicit member boundaries
track acquired staging bytes plus the current-file attempt, without mistaking
retries or zero/cached counters for another file. Total size is known only after
complete resolution. None of these counters prove complete-set verification,
publication, registration or settlement.

Additive commands:

- `start_s3_model_bundle_import`: common explicit source/model facts, an exact
  primary GGUF basename, and 2–32 `{key, version_id, logical_path, sha256}` members.
  Other members are portable relative paths in the existing inert extension set:
  json, txt, md, model, tiktoken, vocab and merges.
- `start_authenticated_s3_model_bundle_import`: the same source request plus the
  existing closed one-use credential DTO. HTTPS, bounds, no ambient discovery,
  no refresh/fallback, static errors and ephemeral lifetimes remain unchanged.
- `get_s3_model_bundle_import`: same optional UUID read, returning existing
  `outcome` plus safe running `bundle_progress`. Decimal strings preserve u64
  bytes, including counts beyond JavaScript's integer precision. Existing
  cancellation is reused with the same UUID.

Both starts return the existing S3ImportOutcome. Single-object anonymous and
authenticated commands and response schemas remain compatible. Builds without
S3 return typed unavailable, including the new getter wrapper. One existing RPC
worker retains only safe UUID/control/progress/results; credentials never enter
its Current snapshot. New methods receive the same privileged diagnostic
containment, response bounds, JSON-RPC correlation, proxy/redirect constraints
and generated receiving decoders as the frozen authenticated milestone.

The existing dialog adds up to 31 explicit auxiliary rows, with separate exact
pins and logical output paths. It observes aggregate staging progress and uses
the original pre-finalization cancellation. Reopening observes the latest job
without resubmission. Draft row IDs are excluded from wire values. Credential
inputs still clear before awaiting admission, on close/replacement/unmount, and
remain outside React task state/defaults/receipts/telemetry. Lost authenticated
acknowledgement observes the same UUID without replay or anonymous fallback.
Possible publication IDs and retained custody remain visible on failed results.
The established loopback HTTP/IPC control plane and HTTPS-only S3 policy remain;
secure memory erasure/privileged inspection resistance are not claimed.

## Verification and commands

Linux x86_64; Rust/Cargo 1.92.0, Node 24.19.0, pinned pnpm 10.33.0, four Cargo
jobs, dev/test debug/incremental disabled, task-owned XDG config. The standards
route remains the authenticated slice's 27 selected standards at revision
`188beda1fa477d21d576c233dd8c7f4c4c267d23`; no AGENTS/local .agents skills were
present. Route/read inventory is retained in `/workspace/scratch/s3-desktop-auth/`.
Current raw logs are in `/workspace/scratch/s3-desktop-bundle/`.

| Check | Result | Log |
| --- | --- | --- |
| Core prerequisite S3 unit/regressions | 12 tests plus one isolated signing child pass, including pure preflight and retry/file counter behavior | `core-s3-tests.log` |
| Core prerequisite S3 integrations | 41 acquisition, 6 native workflow, 13 reader pass; HTTPS child also passes | `core-integration.log` |
| Composed acquisition lifecycle subset | 171 pass | `composed-acquisition-tests.log` |
| Composed model-library/importer/watcher subset | 832 pass; 5 existing ignored; isolated FIFO child also passes | `composed-model-library-tests.log` |
| Composed reconciliation subset | 35 pass, including temporary discovery and unavailable publication observation | `composed-reconciliation-tests.log` |
| Composed S3 acquisition/import/read integrations | 41 + 6 + 13 pass, including real watcher/receipt/cold proof and HTTPS child | `composed-s3-integration.log` |
| Composed full S3 RPC | 202 unit + 16 integration + 2 intent pass; 12 existing ignored/live | `composed-rpc-tests.log` |
| Final empty-member/bundle fixture | 2 focused pass; eight HTTPS scenarios in isolated child | `composed-empty-member-qualified.log` |
| No-S3 source regressions | 4 pass, extended to both bundle starts and getter | `rpc-no-s3-tests.log` |
| Frontend full suite | 863 tests / 130 files pass | `frontend-tests-all.log` |
| Final composed DOM/hook suite | 20 tests / 3 files pass after help-text-only follow-up | `composed-frontend-source-tests.log` |
| Electron Node full suite | 242 pass; one existing native sandbox smoke skipped | `electron-tests-qualified.log` |
| Contract generator/check | Eight generator tests pass; composed canonical check passes | `generator-tests.log`, `composed-contract-check.log` |
| Types/lint/builds | Frontend/Electron pass; final default/library-only Vite and bundled preload built | `frontend-types-qualified.log`, `composed-frontend-lint.log`, `composed-frontend-build.log`, `composed-frontend-build-library.log`, `electron-build-final.log`, `electron-lint-qualified.log` |
| Composed core Clippy | Strict all-target S3 pass | `composed-core-clippy.log` |
| Final RPC Clippy | All targets pass with inherited dead-code lint explicitly excepted | `composed-rpc-clippy-final.log` |
| Format, diff, frozen ancestry/normal composition | Pass | `fmt-qualified.log`, `diff-qualified.log`, `frozen-state.json`, `composition-state.json` |

Scope counts overlap; they are not a deduplicated full-core total. Desktop,
RPC and lockfiles were unchanged by the composition, so their previously passing
Node/UI suites are retained; affected Rust lifecycle suites and full RPC were
rerun on the composed tree. The final follow-up adds only the empty-member test
case and help/docs text: its focused Rust/DOM, lint and both builds pass.

Core commands use `cargo test --locked --offline -p pumas-library
--no-default-features --features s3 --lib <acquisition|model_library|api::reconciliation>`
and `--test s3_acquisition --test s3_model_workflow --test s3_reader`.
RPC uses `cargo test --locked --offline -p pumas-rpc --no-default-features
--features s3`; no-S3 `source_` and final `source_bundle` focused runs are separate.
Core Clippy is `--all-targets -- -D warnings`; RPC adds features
`s3,export-contract,test-support` and `-A dead_code`. Generator `--check`,
pinned pnpm frontend test/types/lint/build tasks, compiled Node tests, and its
eight generator tests qualify the receiving boundaries.

The synthetic HTTPS fixture selects two versions of one key into exact GGUF and
nested JSON outputs. Anonymous, explicit credentials without/with session token,
stalled second HEAD and second GET cancellation, reflected missing-member errors,
digest mismatch and empty-member refusal are checked. During held second-file
transfer the getter reports 1/2 files acquired, 2 acquired bytes, 1 current byte,
3/26 total observed bytes and file index 1. Ready metadata, exact bytes and
manifest/demand/owner receipts are checked; failure/cancellation publishes no
model and blocks implicit replay. All owned files and DEBUG stdout/stderr are
scanned for synthetic credentials, while captured signed headers are checked in
fixture memory. Existing source HTTPS trust remains normally verified using the
repository's synthetic localhost certificate in isolated children.

Raw initial failures are retained: one script first ran from the wrong directory;
one RPC exhaustive serialization arm was added before qualification; schema
copying briefly reintroduced an unnormalized addressing reference and was fixed
by retaining the already normalized enum schema; initial TypeScript build ran
before generation completed; one Node assertion expected ordinary nested
prototypes instead of generated copied values; and an unused test import was
removed after strict lint identified it. Final passing logs are identified above.

## Material limits and next planned slice

The existing S3 reader's nonempty range path does not transfer zero-byte source
members. The new negative fixture resolves both HEADs, admits a manifest with a
zero-size member, then proves a failed retained Transferring record, no verified
member receipts, no GET, no publication and no replay. The dialog/docs say to
select nonempty objects. The counter unit's zero-size fast-path bookkeeping is
not a zero-byte source transfer acceptance claim. No reader/identity/retry/
verification policy was silently changed to mask this limitation.

Browser focus/layout/keyboard and packaged/platform evidence remain separate:
the previously recorded sandboxed Chromium launch fails due installed helper/
crashpad configuration. Default inference Clippy remains blocked by ort-sys's
normal CDN HTTP 403; fully strict RPC headless lint retains existing Torch/fixture
dead-code failures. No security setting/bypass, real credential, bucket mutation,
paid service or external reviewer contact occurs. Real AWS/non-AWS/MinIO and
credential refresh acceptance remain separate; AC13/AC14 and AQ-S3 stay pending.
Parent owns independent review, hosted/provider/browser qualification and PRs/
merges/Library delivery.

Next existing-plan implementation candidate is S3 zero-byte selected-member
handling through the existing acquisition/reader boundary, with explicit digest
and complete-set evidence. This requires its own bounded scope; it is not part
of this frozen bundle milestone. Provider acceptance and Q4 installed/native
qualification remain separately required.
