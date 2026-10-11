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

Direct Rust consumers must recompile and handle the optional checksum returned by
`RuntimeReadFile::sha256`; `RuntimeReadFile::observed` represents absent historical
evidence. The removed public fixed installed-byte validation helper has no
automatic replacement. Existing acquisition receipts and RPC shapes are unchanged.

The experimental owned Cohere session uses a coordinated private v2 layout
correlation identifier. An older installed v1 worker needs a fresh runtime
installation before using that path; incompatible workers are refused and drained.
The shipping audio admission catalog remains closed. These changes do not implement
the separate download-location reconciliation UI or rewrite model files.
