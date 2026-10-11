# Silent model deletion refusal

The user held the delete control to completion for the 8.11 GB
`CLM-v0.1-8B-GGUF` entry. The operation left the row visible without an error.
This repair concerns deletion outcome reporting and exact model identity; it
does not change acquisition custody, locking or the reconciliation plan.

## Observed identities and state

Read-only SQLite, download-registry and directory observations distinguish two
artifacts at upstream revision `c97b04a092bc76b152faf6b57b522b766e159bec`:

| Expected payload | Catalog bytes | Artifact identity component |
| --- | --- | --- |
| `Qwen3-8B-Q8_0.gguf` | 8,709,518,976 (8.11 GiB in the current GB-labelled UI) | `files_9ee061e1214b` |
| `Qwen3-8B-Q8_0-outq2.gguf` | 8,252,495,488 (7.69 GiB) | `files_715cae0a2180` |

Both are recorded separately under `reranker/clm-v0_1/`, despite sharing their
display name and Q8_0 label. At observation, the selected 8.11 GB directory
contained only `metadata.json`; its earlier acquisition was withdrawn and its
queue admission released. The other directory contained a partial payload and
had current acquisition/admission custody. Catalog size is the expected payload
size, not evidence of bytes currently present.

The live `pumas-rpc` process held the native exclusive lock on the library-root
directory. There were no CLM intent declarations or deletion claims, unsettled
CLM admission attempts, or lifecycle quarantines in the observed stores. Two
nearby log entries reported RPC conflict code `-32011`; their method labels were
generic, so timestamps alone do not prove which requests produced them.
These observations describe a point in time, not current or future file state.
No real model files or metadata were changed, copied, hashed or deleted.

## Reproduction and cause

`pnpm exec vitest run src/hooks/useModelLibraryDelete.test.tsx` reproduced the
silent refusal through the real hold control, library-action hook and local
model list: the API received only the selected model ID, but an exception
produced no accessible error. The same-name sibling remained visible. The
minimized fixture uses a partial catalog entry without recovery information,
so an unrelated recovery warning cannot count as deletion feedback.

Ranked hypotheses were retained cancellation custody, cross-destination
contention and another library mutation. Read-only custody excludes the first
for the observed target. `LibraryMutationAuthority::acquire` requests a native
root-wide execution grant before checking destination custody. An active
transfer retains that grant; deleting another artifact can therefore return
`DownloadRootBusy` even though artifact identities differ.

A disposable repository-local fixture exercised the actual release RPC. A
native library-root lock produced the exact `-32011` conflict and left both
fixture directories intact. Releasing that lock allowed immediate deletion of
the selected directory while preserving the sibling payload. This proves the
lock precondition and exact-target deletion behavior; it does not exercise an
actual network transfer or mutate the user's library.

The frontend caught both thrown errors and unsuccessful responses but only
logged them. Separately, RPC projected `DownloadRootBusy` to a generic conflict
message. Both projections prevented useful user feedback.

## Repair contract

The existing library-action hook owns deletion state per exact model ID. It
observes each started request, prevents duplicate pending requests for that
ID, clears its error on retry/success and refreshes models only on success.
Independent model requests retain independent state. The row renders a
dedicated accessible deletion alert, separately from partial-recovery errors,
and disables its delete control while pending. No name, repository or quant
label selects a deletion target or cancels another download.

RPC retains code `-32011`, class `conflict` and the same response shape. The
specific busy-library error uses a bounded public message explaining that the
owning download or operation must finish or be cancelled before retry. Other
conflicts keep their existing generic projection. No private path, credential,
model name or raw internal error is added to that message.

Root-wide exclusion remains an existing backend constraint. This repair does
not authorize concurrent deletion during transfer, add deferred destructive
work, or relax destination/acquisition protections. It does not satisfy the
separate reconciliation plan's C1–C10 claims.

## Verification authority

Coding standards MCP snapshot
`snapshot:v1:1106bd39-e9a8-48d9-897e-8a186918dd48` selected 33 policies with no
unresolved routing categories. Existing policy content was compared with
previous reads; additional TypeScript async and Rust release/dependency
profiles were read. Production edits are limited to frontend deletion state
and projection plus the RPC public-error projection. Dependencies, generated
contracts, persistent formats and mutation authorities remain unchanged.

Focused tests cover failed responses, exceptions, absent failure detail,
pending controls, duplicate requests, independent exact identities, retries,
successful refresh and clearing errors. Component evidence proves accessible
role/name/state and projected interaction semantics; it does not qualify native
pointer capture, keyboard behavior, foreign platforms or model inference.
Completed suite and candidate evidence belongs in the
[execution ledger](../plans/download-location-reconciliation/execution-ledger.md).

## Deletion-scope follow-up

After installing another Debian candidate, the user still observed no deletion
and no visible error for the 8.11 GB entry. Read-only inspection of the installed
frontend found `index-CTIWPst3.js`: its delete handler only logs errors and does
not contain `deletionStateByModelId` or the `Could not delete model:` alert. The
previously qualified resume-policy package contains `index-CThwja8r.js` with
both. Both packages advertise `0.8.0~rc.1`; that version alone cannot establish
which local candidate was installed. No installed file or model payload was
hashed to make this observation.

At follow-up observation, the selected original-weight directory still contained
only `metadata.json`; its old acquisition was Withdrawn and its queue admission
released. The outq2 sibling retained a 433,061,037-byte partial and current queue
custody. There were no intent deletion claims. The latest app run logged a generic
RPC conflict (`-32011`) near the attempted deletion, but the method label does
not independently identify that request. The backend subsequently was unavailable
at its last reported port and the root execution lock was free. These are point
in time observations, not permission to infer a successful deletion or download.
The user's library, app and installation were not mutated during diagnosis.

The existing disposable release-RPC probe reproduced exact root-lock refusal,
then successful exact-target deletion after releasing the external lock. The
new minimized command,
`cargo test --locked --offline -p pumas-library --lib deletion_of_other_weight_during_owned_download_root_grant`,
failed before production changes with `DownloadRootBusy` in 0.39 seconds. Its
real primary API shares two same-name model identities with a live HF task
holding the root grant; the selected metadata-only weight must be removed while
the sibling partial, index entry and canonical acquisition remain unchanged.
The final regression also exercises local IPC and exact-sibling custody refusal.

The earlier repair deliberately retained root-wide exclusion. HF transfers can
share their owner's live root grant, while model deletion previously tried to
acquire a separate exclusive grant before its exact destination checks. The
primary API and IPC now compose deletion with the same HF owner grant and retain
a private, exact-destination deletion execution lease. The library validates
physical root/destination identity, checks durable custody, obtains its existing
intent claim, and retains the lease through all owned deletion and settlement
effects. Foreign/native owners and direct standalone library calls still use
the existing native exclusion. No public caller supplies a grant or bypass.

Transfer effects and durable admission use separate locks for the same physical
destination identity. Deletion retains both. Admission takes its short gate and
rechecks intent custody before the canonical write, preventing an admission
from racing deletion without forcing ordinary queueing to wait for a transfer.
An added regression caught a policy refusal poisoning background shutdown; the
gate now reports ordinary pre-write refusal as a result while retaining actual
write failures, panics and observation failures. The regression requires clean
shutdown, preserved custody and successful queued admission after lease release.

No persistent schema, receipt version, checksum policy, successful-download
provenance or frontend behavior changed in this follow-up. The existing deletion
alert is included in the candidate; a distinctly named local installer will
identify this source slice. Other library mutation lock scopes and reconciliation
C1–C10 are not implemented by this repair. Coding standards MCP snapshot
`snapshot:v1:28e650df-0dc6-4f90-9b6d-75377680eebb` selects 33 policies and zero
unresolved categories; all selected content matched the previously read policies.
Final suites and installer qualification are recorded separately when completed.

### Accepted deletion-scope verification

The final default core package passed 2,333 tests with 30 existing opt-in skips
(unit suite: 2,105 passed, 14 skipped). The final RPC package passed 438 tests
with 12 existing opt-in skips, including all 410 binary unit tests. Its real
controlled HTTP source now proves that an unrelated metadata-only model can be
deleted while bytes are actively arriving; deletion of the active destination
returns the exact busy refusal, its canonical custody and partial are unchanged,
and the original transfer subsequently resumes and completes durable import.
Headless deletion regressions passed six tests. Strict default core/RPC
all-targets Clippy and formatting passed. Frontend source and dependencies are
unchanged; its earlier deletion-feedback behavior tests remain applicable.

An earlier RPC run failed an unrelated vision request-count assertion. Its
cause remains unresolved; the focused case, concurrent cohort and final full
run passed. The [separate diagnostic record](vision-request-count-2026-10-11.md)
preserves that limitation and the new request provenance assertion. No vision
validator or expected outcome was weakened. The first core run preceded the
final admission-gate refinement; the complete core package was rerun afterward.
Logs are in ignored original-repository `tmp/pr69-delete-scope-*.log`.

This local Linux release selects Electron Builder build number `2026101101`,
which its pinned FPM adapter expresses as a Debian package revision. Qualification
must confirm that the actual package version is newer than the observed installed
`0.8.0~rc.1`, and verify a distinctly named installer copy against the qualified
candidate. Project/RPC semantic version and release-attribution inputs remain
`0.8.0-rc.1`; no public release/version/tag or automatic update publication is
selected. Optimized backend and installer checks are pending at this source
commit boundary. No real library deletion or host installation occurred.

### Link-registry publication follow-up

Before packaging, review of the newly allowed overlapping deletions found that
`LinkRegistry::save` cloned its shared cache before dispatching an asynchronous
blocking atomic write. Two clones could capture different states and publish
them in reverse order. Atomic replacement of each file did not serialize their
snapshot/publication order. The prior independent native root grant normally
excluded overlapping library deletions; sharing the primary's HF grant makes
this registry boundary relevant to the admitted deletion change.

`cargo test --locked --offline -p pumas-library --lib overlapping_deletions_cannot_publish_stale_link_snapshots`
was red before serialization: a second cascade removal changed the registry
while the earlier write was deliberately held after snapshot capture. The
verdict took 0.02 seconds. This demonstrates the ordering hole; it is not an
observation of loss in the user's library. A test-only one-shot barrier controls
that exact owned file operation, without sleeps or timing-based ordering guesses.

The link registry now owns one mutation turn shared by its clones. Load, explicit
save, register, unregister, cascade removal and clear retain it across cache
observation/change and file publication. The blocking writer captures the turn,
so aborting its waiting task cannot admit a successor before the actual write
settles. Loading returns the turn with the file observation and retains it
through cache replacement. The data schema, error propagation and store lifetime
remain unchanged. Transfer work never takes this turn. The existing seven
registry tests, including overlapping cascade removals and an abandoned waiter,
pass; both new tests reload the canonical file and require both deleted model
entries to remain absent. Complete suite/lint evidence is recorded when done.

The first optimized backend attempt for `896a7e67` was interrupted while adding
this required persistence fix and is not an accepted installer producer. The
replacement candidate must be built from the committed registry follow-up.
No installed app, model payload, registry in the user's real library, or original
tracked checkout was modified by these fixtures.

The final combined source passed 2,335 default core tests with 30 existing
opt-in skips (2,107/14 unit), 438 default RPC tests with 12 opt-in skips
(all 410 binary tests), and seven headless registry tests. The previously
accepted six headless deletion regressions are retained from the unchanged
deletion implementation. Default core/RPC all-targets Clippy passed with
warnings denied, and formatting passed. The actual controlled HTTP transfer
and unrelated deletion fixture still passes. The earlier unresolved vision
request-count event did not recur; its separate diagnostic record remains
applicable. Logs are in original-repository `tmp/pr69-link-save-*.log`.
The new normal optimized backend and package checks remain pending at this
registry source boundary.
