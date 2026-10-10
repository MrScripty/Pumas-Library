# Verification and delivery procedures

Authority: [plan](../plan.md) and [claims](acceptance-matrix.md). These procedures
are prospective. They have not been run for the proposed implementation.
Existing investigation tests are supporting baseline evidence only.

## Admission and smallest design observation

1. Inspect Git state, current main/PR base, worktree ownership, pinned toolchains,
   dependency manifests, `.venv` ignore behavior and available disk. Preserve the
   original checkout's dirty tracked/untracked content. Do not install undeclared
   packages or run against the live library as an exploratory shortcut.
2. Reroute coding standards MCP for actual changed concerns. Reconcile the bounded
   consumer inventory in [design](design.md) with concrete fresh entry points.
3. Before UI expansion, prove the proposed HF partition representation in a tiny
   real envelope-7 fixture: new decoder reads HF-5, one owned transaction publishes
   HF-6, the retained old HF decoder and actual Lanternwake/Eidetic pinned Pumas
   consumers reject direct reads without metadata writes, and generic acquisition
   updates round-trip the opaque partition. Obtain exact Git revisions from the
   consumers' manifests/lockfiles, retain source identity and test their real
   entry points. Separately exercise those versions' RPC clients against the
   upgraded server using their existing methods and decoders; record tuple-level
   compatibility and unsupported methods. Do not infer wire compatibility from
   a storage rejection test or from server startup. Missing consumer sources/pins
   block this gate. Confirm existing native grant and
   publisher behavior on the target filesystem. If these observations contradict
   the design, change the representation/ordering before expanding implementation.
4. Record concrete metadata byte/file-count bounds, synchronous lock order,
   request latency/admission budget and unknown-outcome behavior in the M1 contract.
   Reuse existing owned limits where applicable. A budget expiry cannot abandon
   an in-flight blocking effect or assert that publication failed. No automatic
   mutation retry; explicit operation identity reconciles unknown outcomes.

The stopping condition for this design observation is evidence for the selected
one-publication representation and exclusion contract. It is not a substitute
for final UI, interruption or package evidence, nor an invitation to investigate
the whole repository before starting the bounded implementation.

## Fixtures and independent observations

Author tiny disposable libraries with valid root identity and exact schema,
admission, repository, selected-file and catalog records. Cover the incident
shapes: two-file GGUF, three-file GGUF plus auxiliary asset, and 33-file bundle.
Old parent paths are absent; current family/catalog paths contain all selected
files. Include two unrelated paused admissions and a modern pinned completed
artifact with a genuine fixture import receipt. Do not copy live databases,
download documents, weights, configuration or credentials into committed tests.

Assert behavior against the owning contract and authored fixture intent, not a
second call to the implementation's matching/projection helper. Observe:

- real file identity, selected-file presence, inode/device where applicable,
  size and content for tiny fixtures; old path remains absent;
- exact original Error snapshot and immutable admission in resolution history,
  paired released admission and no obsolete active record;
- unrelated paused state, opaque acquisition partitions and receipt identities;
- actual HTTP fixture request count and private effect-boundary operations;
- current API reports, decoded RPC/IPC outcomes, refreshed UI and reopened state.

Catalog progress and JSON agreement are useful consistency checks, not upstream
integrity or completion oracles. Tiny synthetic weights prove lifecycle/IO
semantics, not that the user's actual models can perform inference.

## Composed transfer and sibling custody

Use the actual HF client and library against a local multi-file HTTP producer.
Pause delivery at deterministic barriers after a file/stub publication and
before the next file open; activate real watcher/reconciliation and request a
classification change that would historically rename the family directory.
Assert no move under custody, continued selected-file transfer at the original
admitted workspace, no missing-file error, correct importer completion proof,
and eventual deferred reconciliation after normal release. Observe every selected
file, not merely a successful first request or final progress percentage.

Use explicit notifications/barriers and bounded condition waits, not timing
sleeps, to control interleavings. Pair controlled reconciliation tests with a
native actual watcher integration case so injected events do not claim OS watcher
coverage. Repeat affected qualification on existing supported native CI runners.

Inspect sibling migration/move, merge, copied import/publication and deletion
entry points. For each record its exact authority path and either a targeted
observable busy/preserve-files regression or a sufficient shared construction
proof, including old/new targets and post-terminal behavior. Do not invent
duplicate per-method validators when the existing construction already proves
the same invariant.

## Resolution, rejection and interruption

Exercise the public core operation and real persisted transaction before the
desktop consumer. Resolve all three complete fixture shapes with explicit local
acceptance and observe preserved history, released custody and unchanged files.
Observe the same result immediately, via a fresh client and after real reopen.

Restore the legacy failure through the real startup path so it holds its exact
dormant destination-queue claim. Before resolution, prove that claim blocks an
otherwise-authorized transfer to the same original destination. Resolve through
the public operation, then start a distinct authorized download there in the
same process. Observe its producer request and file effect at a deterministic
barrier without closing/reopening the library. A JSON assertion or queue count
alone is insufficient. Reapply captured stale inventory and race an old
generation/refresh with settlement: the released attempt cannot reserve again.
Both unrelated paused downloads remain paused with their exact claims/order and
no requests. Include lost reply and interruption after durable publication but
before live settlement; retry/reopen must complete settlement from durable truth.
Pair this integration case with the existing owner-local stale-inventory test in
`hf/lifecycle.rs`; that existing unit test alone does not prove reconciliation.

Build negative fixtures from the valid case, altering one precondition: missing
selected file, `.part` state, conflicting metadata, wrong repository/selection,
missing expected-file list, wrong root/attempt, traversing/symlinked path,
ambiguous candidates, changed preview, active/hidden admission, modern acquisition,
deletion/intent claim, quarantine, unknown version and malformed resolution.
Assert the exact typed diagnostic and distinguishing structured fields. Prove
the fixture reached that boundary; a generic exception is invalid evidence.

Use existing owner-local publisher fault seams for pre-publication failure,
rename/parent-sync uncertainty and visible-but-unconfirmed state. Add a private
seam only if an actual required boundary is otherwise unreachable. Use a real
child process and disposable library to interrupt before and after publication,
then reopen with the qualified existing fixture ownership lifecycle. Do not
weaken production failed-claim takeover to make a test reopen possible.

Race duplicate requests with the same operation meaning; assert one resolution
and release. Reuse the key with a different attempt/candidate/mode and assert
conflict. Lose the reply and re-query by operation/attempt; do not blindly retry.
Drop notification delivery, refetch a snapshot and observe convergence. Cancel
or close a request during blocking publication and show the existing owner retains
and drains effects before release/shutdown.

Resolution/idempotency history is retained with the released admission; it has
no automatic expiry in this scope. An explicit future archival/retention change
must define its supported retry window and expired-operation behavior before
removing proof. No history is deleted to simulate a successful retry.

## Developer and packaged desktop workflow

The implementer or named operator uses a disposable fixture library and isolated
configuration through normal supported Electron startup. Record OS/session,
display, sandbox, graphics/shared-memory facts, source/backend identity and
bounded lifecycle observations. A special headless smoke configuration can
support diagnosis, but does not prove the normal interactive runtime.

1. Open the fixture library. Observe existing local files and the old failed
   attempt as separate facts. The old path has no model row: reach Inspect from
   the retained-failure entry point even with model filters applied. Inspect the
   report and selected-file coverage; also cover a row-attached failure.
2. Activate **Keep existing files and resolve old download** with keyboard as
   well as pointer. The dialog explains that historical upstream completion is
   unverified, files stay in place and no model backup/download is performed.
   Before the first upgrade, it also warns explicitly that older direct-library
   applications cannot read the new HF format until their Pumas dependency is
   updated. Cancel and observe unchanged metadata/format. Confirm once and observe
   the format change; test the direct-consumer and upgraded-RPC-server paths
   separately using the exact pinned versions from the C5 inventory.
3. Confirm the explicit action once. Observe the resolved attempt and unchanged
   local artifact in the same window without manually closing Pumas or editing
   JSON/SQL. Refresh and verify there is no duplicate transfer/network traffic.
4. Close normally; confirm child/tasks drain. Reopen the same fixture and verify
   the same resolved disposition and healthy UI. Inspect raw fixture metadata
   for preserved original Error and no fabricated completion receipt.
5. Repeat representative missing/ambiguous/busy/stale/uncertain cases. Show a
   concrete reason, refresh/reinspect or existing supported recovery action;
   never enable acceptance with inadequate facts. Restore focus and announce
   outcomes accessibly. A busy result must not terminate the app.

M1 runs this path from the developer build. M3 repeats it with the rebuilt `.deb`
using its actual packaged renderer/preload/backend. Automated core/RPC or mocked
component tests alone cannot satisfy C7/C10.

## Full suites and release

Use the canonical [local QA](../../../../RELEASING.md#local-qa) and
[development commands](../../../DEVELOPMENT.md), with current pinned toolchains.
Run all applicable Rust workspace unit/integration/doc and feature-isolation
checks, frontend tests/lint/types, Electron tests/lint/types, launcher/release
tests, generated freshness/semantic conformance and Python unittest/Ruff gates.
The documented Rustler exclusion is an unsupported Erlang host-binding release
tuple, not a suppressed failing app test; identify it in scope evidence. Existing
native hardware/model suites retain their qualification and cannot be claimed
from synthetic fixtures. Record any required unavailable gate as blocked.

Use the repository `.venv` for Python QA and release helpers; declare/install
development requirements from `torch-server/requirements-dev.txt` before tests,
using the pinned Python version. Do not silently create a `/tmp` environment.
Use frozen pnpm/Cargo inputs. Record the execution inventory, selected filters,
test counts and any documented environment qualifications; zero selected tests
or skipped required qualification does not pass.

Typical native release path, after the canonical QA gates:

```bash
./launcher.sh --build-release
.venv/bin/python scripts/release/stage-rpc.py
.venv/bin/python scripts/release/smoke-rpc.py electron/resources/bin
pnpm --dir frontend build
pnpm --dir electron build
pnpm --dir electron exec electron-builder --linux deb --publish never
```

Resolve `.venv` to the existing original-repository environment when the build
worktree does not contain it. Use a new ignored repo output directory for this
review candidate; never overwrite a retained candidate or published artifact.
Capture all packaged backend/frontend inputs from the same candidate content,
using the existing release provenance/attribution/version-alignment gates.

The M3 helper `scripts/acceptance/download_location_reconciliation.py` prepares
authored fixtures and exercises the packaged RPC path with bounded child
lifecycle and standard-library IO. Reuse the existing package smoke and Debian
install/extraction verifiers for their distinct claims; do not replace them with
a custom installer framework. Verify the package's new recovery workflow with
the actual desktop procedure above. Compute SHA-256 over final package bytes,
record source OID/build inputs and exact artifact path. Treat unchanged manifest
version as a distinctly hashed review candidate, not a newly published release.

## Standards and closure

At milestone review, inspect the actual diff, exact authority/call paths,
public/generated semantic closure, async effect lifecycle and all scoped MUST
requirements from the rerouted snapshot. Re-run the eight-part composed artifact
probe against what was built. Record findings, their owning standards/claims and
closed dispositions; do not assert universal repository certification.

Accept a slice only when its declared gates pass. M1 closes C2/C3/C5/C7 and local
resolution portions of C4/C6; M2 closes their remaining composed/process portions
and C1/C8; M3 closes C9/C10 and all outstanding required evidence. Partial claim
evidence is linked but is not marked satisfied prematurely. Before final
acceptance, all C1–C10 must be satisfied and Git/resource/PR dispositions recorded.
