# Installed S3 pre-FilesReady process-loss qualification — frozen milestone

The original ten installed scenarios and exactly one process-loss case pass
on Linux x86_64 at source `81aea4c0d97aae2a27d1a0c0eebdd784e90f705d`, tree
`65cea0342055e4b76de0f0476a3a480fcce9cfdc`. The actual production binary hash
is unchanged. This adds one bounded AC05/AC17 observation, not gate acceptance.

Parent explicitly admitted one bounded installed Linux acceptance milestone on
2026-10-06. Separate branch `feat/s3-installed-process-loss-ed31639` starts at
frozen `ed31639c2b0d096e5a8bb33ec0eca327f842fb9b`, tree
`16f1b5b8d0eecf54dda84b149bdae02baa343606`. The original branch/ref, installed
archive, records and failures remain preserved for parent publication.

Exact admitted paths: `scripts/release/qualify-s3-installed.py`,
`scripts/release/test_s3_build_provenance.py`, this report and its matching JSON
evidence, `docs/plans/artifact-acquisition/{plan.md,execution-ledger.md}` and
`docs/plans/artifact-acquisition/reports/write-sets.md`. No Rust, production
store/schema, watcher/importer/manifest, SDK/retry/verification/credential,
ONNX, dependency, workflow or network-setting edit is admitted.

Reuse the qualified default-plus-S3 production binary SHA-256
`69df92ec6ece85cfc257e841cfd33f76d624e57a634fa7a319e8bd7a7e00ef57` and existing
build/notice/package verifier. Add exactly one authenticated token single-object
HTTPS fixture: hold GET after one byte; require actual written partial plus
running progress and exact schema-7 `transferring` custody without verified files
or receipts; SIGKILL/reap; reopen twice with no source replay, publication or
custody change. One same-demand cold submission must fail with retained work
before source I/O; a subsequent same-process submission is rejected. Cold task
getters are ephemeral and cannot substitute for durable evidence.

Focused mocked controls cover kill/reap/join, boundary refusal, changed custody,
source replay/model/receipt refusal and secret scanning. Run the existing
16 controls plus additions, pinned CI Ruff, clean-source provenance builder,
the existing ten installed scenarios plus this single boundary, and integrity
checks. Existing release settings and exact binary hash must stay unchanged.
Failure evidence is retained and each child/listener has bounded drainage.

Scope: one Linux process-loss boundary before FilesReady, not power loss,
resumable transfer or a fault campaign. Provider credentials/services,
desktop/platform/inference, hosted acceptance and full AQ-S3/Q4 remain separate.
Parent owns PR publication/reviews/merges. The harness is implemented; all 24
controls (16 existing plus eight boundary/custody/process controls), pinned CI
Ruff lint/format and diff checks pass. Read-only source review found no
substantive issue. Actual clean-source installed execution passes 11/11 with
the fixed locked/offline builder and original release settings. Only Python
harness/tests and owning documentation differ from ed31639; no Rust changes.

## Actual boundary and cold observations

Demand `ff190dc6-0ac2-4abd-8218-30563139c722` retained acquisition
`387da144-007f-4ea7-8cfd-6d2871487b0c` in `transferring`, with no verified files
or consumer receipt. Public progress reported live `acquiring` and one byte.
The actual ordinary partial was one byte, device/inode `27/1706911`, SHA-256
`333e0a1e27815d0ceee55c473fe3dc93d56c63e3bee2b3b4aee8eed6d70191a3`.
Selected identity remained `models/shared`, `weights-v1`, `weights.gguf`,
size 24, caller SHA-256
`a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99`.

The backend was killed as its owned process group and reaped with status `-9`;
its bounded output reader joined, and the held source connection closed. The
canonical store hash before kill and after both cold owners remained
`02000d511a853f7b9aab2d98eb26f1b3f4d80465f810784a77bf8f7a35ce7e34`.
The entire acquisition/manifest/workspace row, physical workspace/partial,
partial size/hash, sentinel and empty logical model rows matched exactly.
SQLite was opened read-only without `immutable=1`, including WAL visibility;
no byte-identical SQLite-file claim is made.

Both cold owners returned exact `not_found` for the former operation and `idle`
without an ID. Both exposed an empty successful public model map and drained
gracefully with status 0. The second owner's same-demand submission first ran,
then returned `failed`, `retained_work=true`, `published_model_id=null`, with
the non-sensitive invalid-request error `-32005`; its next same-process
submission was rejected. Custody still matched after drainage. The source
observed exactly one HEAD and one GET, both pinned to weights-v1, and no request
after the kill. No completed input, model output/index row or publication receipt
was created. The measured cold windows are bounded, not an indefinite promise.

No production timeout/retry/identity/verification or recovery policy changed.
The prefix is process-loss evidence only; it was not synced to assert power-loss
durability. The test neither resumes transfer nor manufactures a persisted task
result. The disposable root stays retained under the external qualification
output, including on failure; safe failure metadata and bounded child/listener
cleanup preserve the original failure. Backend logs/requests with credentials
are never persisted. RPC outputs, captured logs, all root files, committed
evidence and uncompressed archive members pass synthetic-secret checks.

## Artifact, API and evidence

The new archive hash is
`65672d5d9a763b6f93a16da8404d80ed858fd22fe4d19764a2e64e0dcb799482`.
It contains the same binary/license/full 415-entry S3 attribution profile and a
new source-bound qualification record, with exact member/hash checks through
extraction. The old frozen archive remains unchanged. No new CLI flag or Rust
API is introduced: the existing installed harness now includes the one process-
loss case in its normal qualification sequence and retains its disposable root.

External evidence is `/workspace/scratch/s3-installed-process-loss/`: actual
`production-build.json`, Cargo `.jsonl`/`.stderr.log`, `installed.log`,
`installed/installed-result.json`, `installed/process-loss-result.json`, the
archive, retained `installed/process-loss-root`, 24-control and pinned Ruff logs.
An independent offline snapshot recomputation after all children exited exactly
matches the recorded custody. Full results and external hashes are committed in
[machine-readable evidence](s3-installed-process-loss-evidence-2026-10-06.json).
This is unsigned local evidence. Subsequent docs-only commits do not change the
qualified harness or production binary.

## Next documented feature and blocked gates

The next documented implementation feature is **Q2: exact approved wheel-file-
set acquisition and local-only consumption in the existing Torch installer**
(plan AD7 and milestone table). It requires accepted AQ-HTTP, whose authoritative
[gate record](dependency-gates.md) still says not ready. Do not open Q2 or Runtime
R1 from this local S3 fixture result.

No further unblocked S3 feature is named. The next S3 work is actual-provider
Q3/AQ-S3 acceptance, followed by full Q4. It needs authorized version-enabled
AWS and named non-AWS fixtures with exact pins/digests/approved model set and
owner-supplied scoped access, plus approved MinIO inputs or a preinstalled
versioned TLS service. None are supplied, and this task forbids obtaining or
provisioning real credentials. The prior ordinary MinIO installation denial is
not bypassed. Desktop and Windows/macOS acceptance need supported hosts/packages;
deployment needs independent-client, retained-root, old-writer and rollback facts.
Hosted/review/publication remain parent-owned. This milestone is closed without
expanding into additional fault cases, native changes or inference downloads.
