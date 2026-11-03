# Retained download location reconciliation

The HF lifecycle and its model partition own resolution of a legacy Error whose
original destination is absent. The catalog owns the current local artifact;
local acceptance does not turn the historical attempt into Completed.

`inspect_retained_download(download_id)` observes bounded canonical metadata and
selected-file stats through held capabilities. It returns typed eligibility,
exact candidates and a metadata/stat equality observation. It reads no payload,
performs no integrity hash, repair, download or model/index write. Candidate
repository and explicit selected-file evidence must agree; every selected file
must be regular, without symlink traversal or a retained selected `.part`.

`resolve_retained_download` requires the exact failed admission, selected catalog
ID, unchanged observation, canonical operation UUID and `keep_existing_local_files`.
The first confirmation must explicitly accept the older-direct-reader format
incompatibility. An inspection is never a write capability. The core reacquires
root and exact destination exclusion, checks live work/intent/custody, and
reobserves files under the canonical transaction before publishing.

One durable JSON publication preserves the original failed snapshot and admission
in immutable resolution history, transfers only that admission to released
history and removes the obsolete active snapshot. It preserves acquisitions,
receipts, quarantines and unrelated paused downloads. It makes no SQLite or model
file write, no folder recreation, and no successful HF receipt. The same live
owner settles its exact dormant destination claim and fences stale reconstruction
before returning Resolved and publishing a fresh download snapshot.

The envelope remains version 7. HF format 5 has no discriminator/history; the
first resolution selects `hf_model_partition_version: 6`. New readers accept
both formats; older strict HF readers must refuse format 6 without writing it.
Generic acquisition writers preserve the opaque partition. Future/contradictory
formats are invalid. Older RPC consumers have a separate wire contract.

Repeated identical operations establish durability and settle the same live
claim. Conflicting reuse is refused. Stale observations, ambiguity, busy custody,
invalid metadata and uncertain publication remain distinct outcomes. Uncertain
publication cannot authorize old-attempt resume or claim resurrection. Reopen
reestablishes the existing durability barrier before reconstructing active state.

The desktop exposes failed-attempt inspection independently of model filters and
catalog joins. Before local acceptance it explains unverified historical source
and file integrity, and warns of older direct-library incompatibility. Cancellation
is read-only. Confirmed resolution refreshes download and model projections in
the same session; it does not promise inference readiness.

Offline conversion of older envelopes still produces supported HF format 5.
Only an explicit local-acceptance transaction opts that library into format 6.
The separate observation-response marker is a transport custody classification,
not download authority: exactly one completed read-only update response may end
without a delivery receipt. Pending/multiple requests, finite responses and
inference/retention streams keep their normal shutdown obligations.
