# Proposed design and composed-design admission

Authority: this report details D1–D8 in [the plan](../plan.md). It is an
implementation proposal, not a claim that the interfaces below already exist.
Future lasting semantics belong in `docs/contracts/download-location-reconciliation.md`
and `docs/adr/0003-download-location-reconciliation.md`, admitted with M1.

## Evidence, alternatives and selected tradeoff

The [investigation](../../../investigations/download-path-discrepancy-2026-10-10.md)
records three absent old destinations and three populated new catalog paths.
Historical logs show renames between completed files and the next file-open
failure. Current code already denies competing workspace mutation under custody.
Catalog progress and `incomplete=false` are local observations, not receipts.
No exact immutable upstream revision exists for these historical attempts.

| Alternative | Consequence | Disposition |
| --- | --- | --- |
| Rewrite destination paths or set attempts Completed from catalog/size agreement. | Erases the distinction between failed attempts and current files; may bypass lease, file-selection and completion proof. | Reject. |
| Move directories back and retry, or download every file again. | Changes valid current locations, may duplicate terabytes and repeats ownership hazards. | Reject for this recovery. |
| Verify current upstream files and call the historical attempts successful. | Mutable current upstream cannot establish the original revision; payload hashing/network costs do not solve missing historical authority. | Reject as historical proof. |
| Keep strict prevention only; ask the user to edit JSON/SQL manually. | Future mutation guard remains sound but the observed product gap remains. | Reject. |
| Explicitly retain the local artifact and retire the exact old failed attempt, recording both facts atomically. | Files stay in place; original Error remains historical truth; requires one durable metadata disposition and a clear user action. | Select. |

## Single semantic owners

| Fact or transition | Canonical owner | Reference/projection and change reason |
| --- | --- | --- |
| Admitted transfer workspace, selected files and attempt identity | Existing HF download lifecycle and HF persistence facade | Acquisition engine owns its independently scoped acquisition records/leases; adapters do not redefine transfer policy. Changes with download lifecycle. |
| Custody exclusion for a model workspace | Existing `LibraryMutationAuthority`, exact native root/destination capability and shared task owner | Mutators must present authority; a path string or Error status is insufficient. Changes with effect ownership. |
| Live FIFO destination claim and released-identity fencing | Existing HF `DestinationExecutionOwner` in `hf/lifecycle.rs` | Restored failures can own dormant claims with no runnable generation. Durable JSON release does not remove these live claims. Changes with destination scheduling. |
| Current model path and local file facts | Model library/index and existing package-facts owners | Catalog and UI project observations. Classification may change metadata without authorizing a live move. Changes with artifact inspection/classification. |
| Actual modern HF completion proof | Existing importer and completion receipt path | Keep pinned manifest, exact acquisition/use lease and selected-file proof unchanged. Changes with import completion obligations. |
| Legacy failed-attempt resolution | HF facade, internal `download_resolution` module, HF model partition | Stores original attempt/admission and explicit local acceptance. Changes with historical attempt disposition, not inference readiness. |
| Shared JSON locking/publication | Existing acquisition store/atomic publisher | Packages independently owned partitions; does not interpret local artifact availability or legacy resolution meaning. Changes with storage durability. |
| Wire representation and parameter validation | Existing RPC contract/decoder, Electron request registry and generated consumers | Invoke core policy; keep method exposure/trust-boundary duties. Changes with transport contract. |
| User interaction and feedback | Existing frontend download flow | Shows authoritative reports and invalidates/refetches projections. Changes with usability/accessibility, not domain state. |

Do not change the meaning of `DownloadStatus::Completed` or add a misleading
terminal value to the existing public status enum. New resolution DTOs and methods
keep existing host bindings and normal progress consumers unchanged.

## Small public interface

Add transport-independent operations under the existing core API:

- `inspect_retained_download(download_id)` — read-only typed report; its result
  cannot authorize writes. It may return a resolved historical disposition.
- `resolve_retained_download(download_id, exact_attempt_id, candidate_model_id,
  expected_observation, operation_id, KeepExistingLocalFiles)` — explicit finite
  owned operation. Reconstruct authority and revalidate all facts server-side.

The names are proposed domain names; M1 may choose established repository naming
without changing semantics. A caller supplies identities and intended policy,
never raw arbitrary destination paths, a serialized filesystem grant, receipt,
or caller-authored completion proof. The expected observation is a stale-input
precondition, not an authorization token. Bind idempotency to operation identity,
attempt identity, selected candidate and acceptance mode; conflicting reuse fails.

Typed reports distinguish `not_applicable`, `existing_local_candidate`,
`missing_files`, `ambiguous_candidates`, `busy`, `stale_observation`,
`unsupported_history`, `invalid_metadata`, `unavailable_capability`,
`publication_uncertain`, and `resolved`. Keep validation, unavailability,
unsupported history and a resolved local disposition distinguishable on the wire.
Transport failures are not success and are not automatically replayed.

Eligible recovery is a retained legacy Error with an absent old destination,
exact persisted attempt/admission, known selected file list, no modern completion
receipt, and no outstanding effect, revocation or quarantine requiring another
cleanup owner. The two unrelated paused downloads are not eligible and stay
unchanged. Modern/incomplete histories continue through their existing owner;
the new operation cannot substitute for their import or failure cleanup proof.

Candidate discovery is bounded to catalog/metadata identities and the selected
file list for that one attempt. Match repository and artifact-selection evidence,
then validate an exact candidate under the same physical library root. A basename,
family label, repository label or aggregate size is a hint, not authorization or
an integrity proof. Multiple plausible candidates require explicit user selection
and a new exact preview; do not choose the first path. Contradictory selection or
unavailable expected filenames prevents this action. Reject escaped/symlinked
selected paths, unexpected non-regular files and unresolved partial selected files.
Use metadata-size limits and existing validated path types; no full-tree scan.

The report distinguishes observed selected-file presence from verified byte
integrity and historical provenance. The selected action means “accept these
existing local files and resolve the old failure.” It does not promise that the
original upstream download completed, that weights are correct, or that inference
can load them. Existing package-facts/readiness policy remains authoritative.

## Reachable desktop recovery

Mount a retained-failure entry point in the model manager using the authoritative
download snapshot, before joining downloads to current catalog rows or applying
model filters. An absent old directory/model row must not hide the Error or its
Inspect action. `RetainedDownloadFailures` is a presentation component using the
same core report and resolution dialog as row-attached failures; it owns no
matching or storage policy. Deleting it would make orphan failures unreachable,
so it serves a demonstrated UI need. After resolution, refresh both download and
model snapshots; remove the obsolete active failure only on confirmed backend
success. History remains in the core store and inspect-by-exact-ID interface.

## Resolution ordering and persistence

1. Admit the finite operation through the existing task owner. Establish the
   exact Error/attempt eligibility and absence of active effects. Do not cancel
   unrelated workers or infer quiescence from Error, PID absence or a free lock.
2. Acquire the existing physical root grant and held candidate destination
   capability. Recheck old and candidate identities, model mutation/intent claims,
   selected-file observations and other custody. A private, narrowly typed
   resolution capability permits only settlement of this exact retained admission;
   it cannot authorize payload or model/index mutation. Do not weaken the ordinary
   `require_no_download_custody` rule or call `acquire` with a broad bypass flag.
3. Take the existing canonical store transaction in documented root-before-store
   lock order. Re-read the authoritative partitions and compare the attempt and
   observation generation. No synchronous guard crosses an async suspension.
   File/JSON work runs through the existing owned blocking-effect path.
4. Publish **one** validated HF partition update: immutable resolution record
   containing original Error snapshot, exact admission, chosen candidate identity,
   observed file evidence and local-acceptance policy; exact admission transferred
   to the existing released-admission history; obsolete active record removed.
   Preserve unrelated admissions, acquisition records, receipts, revocations and
   quarantines. Do not manufacture an acquisition lease or completion receipt.
5. After confirmed durable publication, settle the exact restored **live** claim
   in `DestinationExecutionOwner`: match physical destination, download/attempt
   identity, domain and generation if present. A restored dormant claim has no
   runnable generation; add a narrow owner-local settlement operation for that
   exact verified claim rather than inventing a worker or clearing the queue.
   Maintain the existing released-identity fence so stale inventory, old task
   generations and refresh cannot reserve it again. Wake the next authorized
   waiter using the existing owner. Preserve unrelated paused claims and FIFO
   order. Existing generation-scoped release alone cannot settle a dormant claim.
6. Update in-memory download state and invalidate/refetch projections, then
   acknowledge success only after both durable disposition and exact live claim
   settlement. An authorized new download to the **same original destination**
   must run immediately in this process without a restart. A lost acknowledgement
   or interruption after publication reconciles the same durable resolution and
   finishes owner-local settlement idempotently; never restore the old claim.
   If settlement has not finished, do not acknowledge success or resume the old
   attempt. Keep the operation owned and report an explicit unresolved outcome.
7. On reopen, history plus released admission reconstruct resolved disposition.
   Before publication, the old Error remains actionable. After publication, the
   old attempt is not resumed. Visible-but-not-confirmed-durable publication is
   uncertain; retain exclusion and use the existing durability recovery path.

No candidate metadata, SQLite row or model file is rewritten by this operation.
That removes a cross-store commit obligation: SQLite already records the current
artifact. The resolution only references it. If implementation discovers a
required model/index publication, stop and revise this transaction design rather
than append an uncoordinated second write.

## Format and compatibility scope

Current shared acquisition envelope is schema **7**; its model partition is
reconstructed as HF format **5**. Keep acquisition envelope and receipt versions
unchanged: the new history changes HF semantics, not every acquisition consumer.

Introduce HF partition format **6**, identified by the partition-owned explicit
`hf_model_partition_version` discriminator in the shared document. An absent
discriminator on a supported schema-7 document means the already-defined format
5, not guessed new semantics. Acquisition storage projects/preserves the explicit
partition discriminator but leaves partition interpretation to the HF facade.
Replace its hard-coded format-5 reconstruction at every HF partition read/write.
Strict format-5 and format-6 decoders are separately validated; unknown versions,
duplicate keys and incomplete resolution records fail closed.

Admitted overlap: the new application reads existing envelope-7/HF-5 and current
envelope-7/HF-6. A lossless metadata-only conversion occurs inside the first
admitted resolution transaction under existing native exclusion; it requires no
manual restart. Original snapshots and unrelated partitions must round-trip.
Old HF-5 readers must reject the new discriminator/history without writing or
downgrading it; prove that using the retained old decoder fixture and the actual
Pumas revisions pinned by Lanternwake/Eidetic, with source/lockfile identities
recorded. Test direct library consumers against the upgraded fixture separately
from older RPC clients using the upgraded server. For the latter, exercise the
existing methods and response decoders they actually use; new recovery methods
are not assumed available to older clients. Record supported/unsupported outcomes
for each exact producer/consumer tuple. A local decoder copy alone does not prove
consumer compatibility. If the consumer revisions are unavailable, C5 is blocked.
Existing generic acquisition envelope-7 operations may preserve the opaque HF partition
without interpreting it; prove that round-trip separately. If either safety
property fails, revise representation/version placement before expanding M1.

Before the first format-changing confirmation, say explicitly: “Resolving this
download upgrades this library's HF storage format. Older applications that open
the library directly will no longer be able to read it until their Pumas
dependency is updated.” Name affected pinned consumers in supporting documentation.
Do not imply that every older RPC client breaks, or that an upgraded server makes
every client compatible. Show the compatibility notice with the provenance
limitation and local-file policy, not after publication; cancelling does not
upgrade anything. Subsequent confirmations accurately report current format.

Already-supported outer legacy formats 4/5/6 retain the explicit existing upgrade
path. Extend its converter's output to the current partition without dropping
custody. This is distinct from the current-format resolution transaction. Unknown
future formats are rejected with a usable diagnostic. No backward writer or
automatic downgrade is promised. If metadata backup/rollback is offered, it is
bounded to the exact metadata document and requires ownership/quiescence; never
roll back a store underneath an admitted worker.

## Shared-workspace family and consumer dispositions

The finding is systemic: historically classification and transfer independently
mutated one workspace. The bounded population is the set of mutators/readers
sharing destination custody, attempt disposition or local-availability promises.

| Consumer / sibling | Disposition and required evidence |
| --- | --- |
| HF worker, queue, pause/resume/cancel, startup restoration | Retain exact immutable workspace and selected-file admission. Error at an absent destination stays visible until explicit resolution; settle its exact dormant/live queue claim after durable release, preserve released-identity fencing, and prove immediate authorized same-destination transfer. No automatic retry or recreated directory by reconciliation. |
| Watcher/reconciliation and forced refresh | Use existing authority. Busy opportunistic work keeps dirty state and retries after custody ends; forced requests return typed busy. No fatal startup/shutdown effect. |
| Reclassification and directory rename | Guard both old and new targets before mutation; classification cannot edit a running transfer's identity. Verify deferred post-terminal relocation keeps model/index identity consistent. |
| Library migration/move, copied import/publication, merge and deletion | Reuse the same authority and existing exact import/deletion capabilities. Inspect all actual entry points reachable in `library.rs` and importer; give each a test or sufficient construction proof. No separate skip-custody rule. |
| Managed partial and final import | Preserve existing exact capability and completion-claim rules. Partial local visibility cannot trigger a conflicting reclassification; final proof remains required for Completed. |
| SQLite catalog, metadata projection, package facts and local model rows | Present artifact facts without deriving old attempt success. Resolution does not change readiness or import receipts. |
| RPC request decoder/handler, Electron allowlist/preload, generated TS and frontend | Thin adapters for the same core interface; negative parameters and all outcomes preserved; no JSON/SQL edits or guessed candidate paths in the renderer. |
| Existing native/local intent and host-binding consumers | Preserve existing methods, status enum and readiness semantics. No new resolution promise for a host binding or local IPC method in this slice. Compile applicable existing surfaces; change in their semantics reopens this disposition. |
| Root startup/upgrade and packaged runtime | Preserve conservative claim handling and metadata-only upgrade. UI recovery refreshes in-session; normal close/reopen stays healthy. Unknown failed claims are not automatically removed. |

At M1 admission, reconcile this inventory with fresh source and list the actual
entry points in evidence. At M2 closure, every selected consumer must have a
non-blocked tested/preserved disposition. This is not a repo-wide refactor audit.

## Eight-part composed artifact probe

1. **Independent concerns.** Transfer scheduling owns what files/attempt, why and
   when effects run; mutation authority owns who may change an exact workspace;
   model/index owners own where current files are and what is locally observed;
   HF resolution owns when an obsolete attempt can be settled and how its history
   persists; publisher owns durable storage; transport/UI own delivery and user
   choice. They share explicit identities, not each other's private representation.
2. **Interleavings.** Required: exact runtime quiescence → root grant → canonical
   transaction → durable resolution/release → notification. Accidental historical
   interleaving, classification rename between worker files, is excluded. Family
   classification no longer acts as authority to move an admitted workspace.
3. **Caller knowledge.** Callers know download/attempt/candidate identities,
   observed precondition, acceptance mode and typed results. They need not know
   lock order, store schema, path normalization, queue predecessor records, SQL
   layout or receipt construction. The composition root reuses its existing HF
   client and task/root/store resources; it adds no second long-lived owner.
4. **Change locality.** Candidate matching policy changes core resolution and its
   focused tests; durability changes existing publisher/transaction evidence;
   classification changes classifier and shared-guard tests; UI wording changes
   the component; wire representation changes its declaration and generated
   adapters. A semantic policy change must not require frontend storage logic.
5. **Dependencies.** Stable values carry exact attempt/artifact identities and
   typed outcomes. Physical capabilities stay internal. Explicit partition
   adapters contain version knowledge. No UI/parser consults raw JSON fields or
   serializes a use lease as recovery permission.
6. **Independent evolution/failure.** Core resolution is testable without Electron;
   HTTP/IPC can fail without replaying committed resolution; lost notifications
   recover by snapshot; model readiness evolves without changing resolution
   history; generic acquisition storage preserves the HF partition opaquely.
   Persistence evolution is tested against actual retained reader combinations.
7. **Deletion test.** Remove the internal resolution module and necessary proof
   and ordering would spread across handlers, store and UI, so keep it deep.
   Remove history and durable distinction/idempotency disappears, so retain it
   with original snapshots. Remove the HF version and reader safety disappears,
   so keep its scoped discriminator. RPC/IPC adapters are required by current
   process/trust boundaries; generated validators already exist and prevent
   hand-maintained wire copies. Delete proposed alias registry, daemon, new SQL
   journal, generic repair engine and public receipt factory: no required behavior
   is lost, so do not add them. Test fault seams stay private to the owning module.
8. **Inherent complexity and cumulative machinery.** Exact custody, incomplete
   historical proof and crash durability are inherent. Contain them in one HF
   operation with one JSON publication, one HF format transition and two typed
   interface operations. Reuse root grants, task draining, atomic publisher,
   released-admission history, snapshots and generation. No cross-store commit,
   payload backup, automatic relocation or indefinite compatibility writer is
   introduced. Review the implemented artifact, not just this proposed split.
