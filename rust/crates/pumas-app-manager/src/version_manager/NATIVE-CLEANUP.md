# Native staging custody and cleanup

## Scope and writer boundary

`NativeVersionsLock` is the permanent coordination lock for cooperating Pumas
writers. One `ReservedDirectory` captures the root and every reserved relative
component once; acquisition and native cleanup share its handles, physical
identity, revocation, and lease. Reads and writes through acquisition use held
capabilities and refuse an observed pathname replacement. Observed acquisition
subdirectory replacements are checked before cleanup mutates content. Existing
native output extraction/publication helpers retain their path-based interfaces,
with grant checks at their effect boundaries; this slice does not replace those
helpers or claim adversarial path-race safety for them.

The contract excludes arbitrary concurrent mutation inside the owned tree by an
actor with identical filesystem authority. In particular, Unix cannot atomically
compare a directory identity and unlink that name. Empty-only final removal is
under the writer lock and rejects replacement observed before unlink; it is not
an adversarial-TOCTOU guarantee. Inode/file-ID reuse after cold removal is also
outside this physical-binding proof. No root is migrated or silently adopted.

Windows reservation handles omit `FILE_SHARE_DELETE`, pinning the root and child
while they are traversed. The existing rename-permitting opener is unchanged.
Descendant pins survive revocation and settled-use validation; each observed
acquisition pin and recursive child handle is held through traversal and released
only before that child's empty-only removal under its still-held parent. The final workspace handle is
released only after all acquisition/grant clones drop. A sharing or identity
failure retains staging, with no ambient recursive-delete fallback. Windows
reparse points are conservatively retained for reconciliation; Unix symlinks
are unlinked without following their targets.

## Persisted attempts

New attempts exclusively create a fresh random leaf, capture its binding, sync
the leaf/root/parent, and atomically persist version 2 before acquisition starts.
Version 2 includes root/component physical binding and a cleanup-pending reason.
A creation-before-persistence crash leaves an unclaimed orphan; a persisted
active attempt with missing staging is never recreated. A retained old attempt
must be reconciled before a new record replaces it.

Version 1 remains readable, and removal may still persist its existing revoked
flag. A present v1 leaf has no physical custody proof and is retained; the v1
record plus present leaf is durable reconciliation evidence, projected as
cleanup-pending status on adopted recovery. An absent v1 leaf is a harmless
cleanup no-op. No fabricated binding, implicit upgrade, or downgrade is written.
Old readers reject v2 by their schema-version/unknown-field checks and must not
be used to reclaim v2 staging. Mixed-version writers are not supported.

## Ordering and outcome

Cleanup has two phases: revoke and clear contents through held directories, then
remove only the empty shell after acquisition handles have dropped. Cancellation
persists attempt revocation before clear. `withdraw_after_cleanup` removes the
unreceipted Using state only after clear succeeds. Failed clear keeps durable
acquisition custody; cancellation and cleanup causes are both returned.

`Removed`, `Absent`, and `Retained` report staging disposition.
`DiagnosticPending` means staging is already absent but its durable status update
failed; the visible message distinguishes this from physically retained staging.
Installed output and its receipt are verified independently. Successful publication remains
successful if staging is retained, with the attempt's durable pending diagnostic
and the existing progress current-item/Setup message identifying cleanup pending.
The error field remains unset so the existing UI shows a successful installation.
Delayed status clearing is fenced by tag/start/completion identity and preserves
this native-owned pending status; successful reconciliation clears only its own
matching warning. A diagnostic failure after incomplete cleanup preserves both
causes; creation-time pending state survives if root replacement prevents an
update.
Existing progress-file persistence is asynchronous and is not the authoritative
durability proof. Removal/reinstall require a truthful successful cleanup report.

## Why these lifecycle changes are necessary

The shared grant prevents native custody and acquisition from opening different
objects behind the same name. Persisted v2 binding prevents restart from treating
a nonce-only path as ownership. Clear/remove separation permits cancellation to
withdraw only after input deletion while respecting Windows open-handle rules.
The small cleanup report preserves valid installed output without hiding failed
staging reclamation. The acquisition store and its receipt protocol are unchanged.

## Required verification

Focused tests cover live root/child/nested and same-name-empty replacements,
original/replacement sentinels, outside symlinks, benign cleanup and clone order,
Windows root/child pinning, cold v2 match/mismatch/missing, v1 present/absent,
creation/persistence crash-state replicas, cancellation withdrawal order, and
retained cleanup diagnostics. Linux integration fixtures additionally cover
adopted verified output with v1/v2 retained staging and failed cancellation clear
with durable Using preservation. Crash-state fixtures are not power-loss tests.

Hosted Linux, macOS, and Windows focused/package gates remain required; local
formatting or source review does not establish cross-platform parity.

Implementation references for the pinned-directory mechanism are cap-std v4.0.3
[open_dir](https://github.com/bytecodealliance/cap-std/blob/v4.0.3/cap-primitives/src/fs/open_dir.rs)
and Windows
[dir_options](https://github.com/bytecodealliance/cap-std/blob/v4.0.3/cap-primitives/src/windows/fs/dir_utils.rs).
These references explain mechanism selection, not native-platform test results.
