# Acquisition prerequisites audit

Date: 2026-09-12. Source inspection only; no tests were run for this report.
Paths below are relative to `rust/crates/pumas-core/src/` unless qualified.

## Recommendation

Admit a bounded immutable-revision prerequisite before M2 orchestration. This
can extend the existing downloader and persistence owner without redesigning
their authority. Preserve current operational requests' default-`main` behavior;
require a resolved immutable commit for intent acquisition. Do not publish
`AllowUpstream` acquisition until its complete pinning path is verified.

Incomplete recovery evidence does not establish that ordinary live acquisition
is unsafe and does not prevent this prerequisite work. It does prevent claiming
the full M2 recovery gate or M3 restart/crash guarantees. Keep those distinct
from the concrete revision defects below. M1 remains independently deliverable.

## Concrete gaps in the current acquisition path

| Concern | Source evidence | Required behavior |
| --- | --- | --- |
| Request and metadata | `model_library/types.rs:524` has no download revision. `model_library/hf/types.rs:309` does not decode a repository commit. `hf/metadata.rs:40,161` fetch unversioned metadata and `tree/main`. | Resolve a supplied branch/tag or default selector to a validated commit before admission; retain it throughout preflight. |
| Byte transfer | `hf/metadata.rs:286,349` and `hf/download.rs:3724` construct `resolve/main` URLs. | All auxiliary and payload reads, retries, and resume use the same commit. Revision-aware cache keys must prevent reuse of another revision's metadata. |
| Artifact identity and destination | `model_library/artifact_identity.rs:84,240` hardcode `main`. `:204–228` selects bare quant, GGUF filename, or `full_repo` for several artifact IDs; changing the digest alone does not distinguish them. `api/hf.rs:370` uses that artifact ID in the destination. | Pinned revision participates in both identity and managed destination for every selection kind; legacy default-main paths remain deliberate compatibility behavior. |
| Deduplication | `hf/lifecycle.rs:651` pending identity and `hf/download.rs:2793–2821` active dedupe use destination, repository, filenames, sizes, and hashes, without revision. | Equal pins may join; distinct pins cannot collapse, including when file evidence coincides. No intent-owned second downloader. |
| Durable execution | `model_library/download_store.rs:29–48` persists `DownloadRequest`; that request currently cannot preserve a pin. | Persist the resolved commit before acceptance. Cold resume must not resolve a moving branch again. |
| Imported evidence | `model_library/artifact_identity.rs:111` projects the selected identity's revision into `upstream_revision`. | Import and marker reconstruction retain the exact commit that supplied bytes. |

## Proposed bounded write set

Resolve exact helper placement before edits, using these concrete owners:

- `src/model_library/types.rs`: request representation and reusable validation.
- `src/model_library/artifact_identity.rs`: pinned identity and destination naming.
- `src/model_library/hf/{metadata.rs,types.rs,download.rs,lifecycle.rs}`:
  revision metadata, URLs/cache isolation, admitted execution, and dedupe.
- `src/model_library/hf/bundles.rs`: its tree lookup and `model_index.json`
  preflight also use `main` (`:24,40`) and must use the same resolved revision.
- `src/api/hf.rs`: existing managed preflight and destination integration.
- `src/model_library/download_store.rs`: persisted execution validation and
  explicit format/upgrade/downgrade handling through the existing store owner.
- Existing tests in those files; a controlled upstream fixture at the narrowest
  existing test seam. Admit `hf/mod.rs` only if that fixture needs construction
  wiring, rather than adding a production endpoint override for testing.
- If adding a field to public `DownloadRequest`, mechanical constructor updates
  in `src/{api/builder.rs,model_library/importer.rs,tests.rs}` and in
  `rust/crates/pumas-rpc/src/contract.rs`,
  `rust/crates/pumas-uniffi/src/{bindings.rs,bindings/ffi_types.rs}`. These are
  compatibility updates, not authority to expose a new binding operation.

Importer production changes are needed only if inspection proves its existing
request-to-metadata path loses the new revision. Do not admit broad importer,
recovery, or storage rewrites under this slice.

## Compatibility decision required before implementation

Adding `revision: Option<_>` with a serde default preserves decoding of old JSON
requests, but it breaks Rust source compatibility for public struct literals.
Inventory and update in-repo constructors; record the external Rust compatibility
class. An internal resolved-download value is an alternative if preserving the
public struct is required, but must still carry its revision through durable
execution and importer evidence.

Serde defaults alone are not safe downgrade handling. The current download
store strictly requires schema version 4 (`download_store.rs:1489–1514`), while
its nested public request does not reject unknown fields. An old reader can
ignore a newly added request revision and execute its old `main` behavior.
Select a versioned representation that makes unsupported pinned state fail
closed on old readers. A store schema increment with an explicit validated
upgrade is a candidate; record downgrade refusal and preserve original state
on migration failure. This changes the persistence contract, not its owner.

Default-main operational requests must retain their documented behavior and
existing destination identity. Intent pinning must not infer that legacy
`upstream_revision = main` identifies an immutable commit.

## Required evidence for the prerequisite

1. Move `main` after resolution in a controlled upstream; metadata, auxiliary
   files, payloads, retry, and reopened resume still request the original commit.
2. Equal pinned requests join one writer. Different pins with identical file
   names/sizes/hashes remain distinct for GGUF, quant, full-repo, and file-group
   identity paths. Unsupported or invalid revision input mutates nothing.
3. Persist/reopen pinned execution and prove import metadata records that commit.
   Failed integrity cannot publish available state. Exercise absent or
   contradictory commit evidence and upstream refusal. M2a accepts a validated
   commit; production branch/tag/default-selector resolution and its failure
   cases belong to M2. Moving-main fixtures may resolve their initial commit in
   test setup before invoking the shared revision-aware acquisition entrypoint.
4. Validate current-store upgrade, failed migration preservation, and unsupported
   downgrade refusal; retain a default-main request/store regression fixture.
5. Re-run affected owner tests and Rust consumers after constructor changes.
   Existing examples include caller-drop tests (`hf/download.rs:7589,7660`),
   real importer ordering and shutdown (`:10020,10276`), shutdown waiter custody
   (`:13551,14714`), and byte-complete restore (`:16607`).

The full intent integration and representative real HF acquisition remain M2
gates after this prerequisite; the above tests do not replace them.

## Recovery guarantees still outstanding

`hf/download.rs:341–365` explicitly refuses active recovery custody, hidden
admissions, and unverified quarantine during restore. That is concrete
fail-closed behavior, not evidence of a duplicate writer. Automatic intent
reconciliation must observe that blocked state rather than retry admission as
if the old work did not exist.

The existing remediation plan records production restore/lifecycle work open
and hard-process crash recovery unproved (`current-standards-remediation-2026-09-03/plan.md:18–27`).
It separately accepts awaited managed importer ownership within its Linux
checkpoint; `api/builder.rs:484–489` installs the actual importer. Revision
support neither closes nor invalidates those existing evidence boundaries.

Before claiming M2/M3 complete, coordinate exact admission-outcome recovery,
persisted unresolved-custody reconciliation, resume/import ownership, shutdown
settlement, and forced-process-exit evidence with that existing owner. Do not
automatically repeat mutations whose durable outcomes remain uncertain.

For M3 planning, the existing `ModelIndex` connection is a plausible declaration
owner (`index/model_index.rs:250,271–334`, WAL and `synchronous=NORMAL`). Actual
administrative deletion runs through `api/links.rs:83` and
`model_library/library.rs:2092`; both require explicit M3 write-set admission
for a retention guard serialized against ensure. No desired-state persistence
or deletion changes are part of this prerequisite slice.

## Concrete design recommendation for M2a

Preserve public `DownloadRequest` and add a crate-private execution revision
value, with distinct legacy-main and validated immutable-commit cases. Place
the value beside the existing artifact identity helpers, rather than creating
a new public identity system. Existing operational entrypoints delegate with
legacy-main. A crate-private managed pinned entrypoint accepts the existing
request plus a validated commit; intent will resolve the selector before
calling it. Branch/tag text must not reach worker URL construction as a pin.

Use revision-aware private helpers behind existing metadata, bundle, download,
and artifact-identity methods. A resolved revision determines metadata/cache
lookups, every URL, artifact ID/destination, pending admission identity, state,
worker execution, markers, persisted snapshots, and restored execution. Do not
derive it later from display paths or patch metadata after import publication.

The existing public callback payloads (`hf/types.rs:69,94`) and public
`InPlaceImportSpec` can remain unchanged. Add revision-aware crate-private
importer entrypoints; their public counterparts delegate with legacy-main.
Thread revision into the private `import_in_place_with_mode` and
`import_library_owned_diffusers_directory` helpers and their existing metadata
construction. Pass it separately from public notification payloads inside
`import_completed_download` and the Aux mutation. Notifications remain the
same operational contract; indexed metadata records the pinned truth before
availability is published. `model_library/importer.rs` is therefore an explicit
production write file for this recommended design.

Persist execution revision with each snapshot, validating it everywhere the
store validates resumable, admission, quarantine, and revocation payloads.
Choose schema version 5 with an explicit v4-to-v5 upgrade through the existing
atomic store owner, mapping old records to legacy-main. Existing v4 binaries
then reject the new store instead of silently resuming pinned bytes from main.
Preserve all custody data; reject malformed upgrades without replacing the
original document. Adding a public `PersistedDownload` field still changes
Rust source compatibility for callers constructing that lower-level record;
record this separately. Avoid claiming that preserving `DownloadRequest`
preserves every public struct literal in the crate.

Migration shares the store's existing atomic-publication outcome contract:
malformed input and failures proven before replacement preserve original bytes.
A failure after replacement can leave the complete v5 document visible with
uncertain durability. Return the failure, preserve observed custody, and do not
claim success or roll back guessed state. Fault-injection evidence must cover
both outcomes; original-byte preservation is not promised after publication.

One additional boundary needs an explicit decision: ticket recovery's
`RecoverySnapshot`/`VerifiedDownloadRecovery` currently carries repository and
selected artifacts, but no revision (`model_library/download_recovery.rs:142–176,973–1061`).
It cannot recreate a pinned request safely by defaulting to main. Coordinate
that file with its remediation owner and either propagate the validated
revision through snapshot fingerprint, verified ticket, and recovered
admission, or explicitly refuse pinned ticket recovery until that propagation
is implemented. The latter is a bounded prerequisite limitation, not completed
M2 partial-recovery support. Native persisted resume must support the pin in
either case. Existing operational recovery must not overwrite a pinned
artifact through an unscoped default-main request.

For this choice, the production write set is
`model_library/{artifact_identity.rs,download_store.rs,importer.rs,download_recovery.rs}`,
`model_library/hf/{metadata.rs,types.rs,download.rs,lifecycle.rs,bundles.rs}`,
and `api/hf.rs`, with `hf/mod.rs` for controlled metadata/byte test fixture
wiring if needed. Tests belong with these owner modules initially. Public
request constructors in RPC/UniFFI need no mechanical changes. Admission of
`download_recovery.rs` is narrowly for revision propagation or fail-closed
refusal, not the unresolved recovery lifecycle program.

## Protocol verification during implementation

Checked the official [Hugging Face Hub client source](https://raw.githubusercontent.com/huggingface/huggingface_hub/main/src/huggingface_hub/hf_api.py)
on 2026-09-12. Revision-specific model metadata uses
`/api/models/{repo_id}/revision/{revision}`; repository trees use
`/api/models/{repo_id}/tree/{revision}`. A query parameter on the unversioned
model endpoint is insufficient evidence of pinning. Controlled fixtures must
exercise these actual routes and reject absent or contradictory commit evidence.
