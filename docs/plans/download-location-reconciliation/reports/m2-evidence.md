# M2: custody and failure evidence

The existing LibraryMutationAuthority remains the shared mutation owner. Resolution
adds only unconfirmed old/candidate targets to its existing custody inventory.
Reclassification/relocation acquires both targets; migration, merge, import,
delete and metadata publication continue using their existing exact capabilities.
No sibling gains a skip-custody rule, path-alias authority or queue-wide reset.

The focused release test proves a real authorized same-destination transfer in
one process, stale claim/generation fencing and two unrelated paused admissions.
Three private publisher fault modes keep original/released history coherent,
preserve custody until a real publication barrier, refuse old-attempt resume and
recover through same-operation retry and clean client restoration. Caller effects
use the existing owned invocation and blocking-task drain; no dropped receiver
can cancel a publication already owned by that runtime.

Actual desktop testing discovered five ordinary observation disconnects recorded
as unfinished HTTP effects. The small regression failed with a shutdown-grace
receipt before the fix and passed afterward. All 18 HTTP custody tests passed,
including unmarked blocked writers, pending handlers, malformed bodies and panic
receipts. Only an actual producer-marked, sole completed observation response is
exempt from delivery failure; inference/retention streams remain unmarked. Electron
uses separate nonpooled connections for the five update streams. Real desktop
close/reopen now exits cleanly. This is committed separately as `0175852b`.

## Remaining objective evidence

The complete controlled multi-file transfer plus native watcher classification
flow required by C1 is not established by the focused guards or the same-destination
transfer. C6's controlled process termination at publication/live-settlement
boundaries is also not replaced by fault injection and normal reopen. These exact
composed probes remain pending; no assertion or accepted claim hides that gap.
The existing native CI matrix now runs retained-location tests with default and
no-default features on Linux/macOS/Windows. New non-Linux results are unavailable
until this source is pushed and those jobs run. The plan remains Verifying.
