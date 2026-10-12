# 0.8.0-rc.1 local candidate changes

This unpublished Linux desktop candidate continues the existing 0.8.0-rc.1
version. It is built for local review; Windows/macOS execution and model
inference are not qualified by its Linux packaging checks.

- Downloaded files are checked against supplied SHA values during acquisition.
  Subsequent startup, model reads, reconciliation and runtime preparation no
  longer compare file contents with historical SHA values. Original checksums
  remain stored for explicit integrity verification. Required files, lengths,
  ownership and valid acquisition receipts are still checked.
- The normal staged audio installer derives all 68 version constraints from the
  pinned recipe and verifies package name, version, URL and acquisition hash
  before publishing the runtime. Cold and warm installs match the same recipe.
  Installed-tree byte fingerprints no longer control runtime admission.
- Startup recovery refuses unsupported legacy library schemas before registering
  a claim, including headless builds. Cross-platform cancellation fixtures share
  their readiness timeout. GGUF corruption tests isolate filesystem reconciliation
  and separately exercise live reindexing.
- A partial catalog model with a managed download no longer shows the inactive
  recovery warning while downloading, queued, pausing, paused, cancelling or
  awaiting retry. Live pause/resume controls continue to use the exact download
  ID. Inactive partial models still require current recovery information.
- Failed model deletion now shows an accessible error on the exact selected
  model row, including partial entries. Delete controls stay disabled while the
  request is pending and allow retry after refusal. Busy-library RPC errors
  explain that the owning download or operation must finish or be cancelled;
  the existing whole-library exclusion remains in effect.
- Primary API and local IPC deletion can remove one model while an unrelated HF
  download runs. Exact destination custody, admission and intent protections remain
  enforced; the sibling download's partial data and receipts are preserved.
- HTTP resume retains partial downloads across pause, failure and process restart.
  When the selected file has a supplied SHA, it continues from the actual saved
  offset and checks the assembled download before publication. Unsupported or
  invalid continuation leaves the partial intact; a server ignoring the range
  request cannot silently restart the download.
- A failed download now shows an accessible error beside its exact library
  weight or unassociated activity row. Its retry control retains the download
  identity. An enabled play action still requires a complete catalog entry;
  transfer progress alone does not mark a model ready.

Direct Rust consumers must recompile and handle the optional checksum returned by
`RuntimeReadFile::sha256`; `RuntimeReadFile::observed` represents absent historical
evidence. The removed public fixed installed-byte validation helper has no
automatic replacement. Existing acquisition receipts and RPC shapes are unchanged.

The experimental owned Cohere session uses a coordinated private v2 layout
correlation identifier. An older installed v1 worker needs a fresh runtime
installation before using that path; incompatible workers are refused and drained.
The shipping audio admission catalog remains closed. The [retained-download recovery](runbooks/download-location-reconciliation.md)
action now inspects moved legacy files and explicitly settles the old failure
in the running desktop. It preserves model files and original Error history.
The first confirmation upgrades only the HF partition to format 6; older direct
library readers must update their Pumas dependency. Local acceptance creates no
verified HF receipt and makes no inference claim. Read-only update streams now
close cleanly without marking the instance shutdown as failed.
