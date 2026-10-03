# Copied-import review corrections

PR #28's first complete external review covered head
`48d86d91f3da94a2d0703ff936e8c18d98448655`. Its ordinary Build gates passed,
but the review identified behavioral corrections. That evidence does not qualify
this follow-up tree; native execution and independent review remain required.

## Copy and terminal observations

- Copy-time hashing and the final held destination verification retry only
  `io::ErrorKind::Interrupted`. The regression interrupts before data, between
  short reads, and before EOF; bytes, length, SHA256 and BLAKE3 must remain equal.
  A separate InvalidData regression preserves the actual read error and partial
  destination bytes rather than retrying corruption.
- Batch terminal `Complete` awaits channel capacity after every per-item owner
  has settled. Per-item/staging progress remains best effort. A full-channel
  regression drains `RuntimeTasks` before receiving the terminal batch receipt;
  channel backpressure therefore cannot retain payload custody. Closing the
  observer still returns the accumulated results. Dropping the batch caller is
  observer loss, not a promise that an absent observer receives a terminal event.
- B1 merge publication checks, expected-row observation, metadata normalization,
  conditional remap and unstarted-claim release run in observed blocking tasks.
  Both mutation guards remain retained until successful post-move settlement;
  filesystem/database failures are not converted into successful cleanup.

## Ordinary Unix directory mode compatibility

The previous importer created `.tmp_import_<uuid>` directly under the library
root with normal directory-creation permissions, then renamed that same object
under its canonical model parent (`main b625862b`, importer.rs's
`create_temp_import_dir`). It did not inherit a new group or ACL from the final
parent on rename.

The repair keeps the payload stage atomically private (0700 subject to the
process umask). Before copying payload, it creates a separate **empty** template
with normal directory-creation permissions under the same held library root.
No payload, metadata, receipt or credential is ever placed in that template.
The template's observed mode is bound to the actual stage and root identities;
its UID/GID must match the stage. Cleanup validates the held template's identity
and removes only an empty directory. Replacement or unexpected contents are
retained with an explicit error, never recursively removed or retried by name.

Final ordering is: no-replace payload publication -> exact held payload
verification -> held-directory mode restoration and synchronization/readback ->
durable Confirmed receipt -> Ready metadata -> conditional Ready index.
An inability to settle final permissions leaves a retained Pending publication
and a real import/shutdown failure. No rollback or automatic promotion is added.

Native Unix subprocess fixtures use umasks 000, 002, 022 and 077 without changing
the multi-threaded parent process's umask. They assert private staging and the
expected final mode (0777, 0775, 0755 and 0700 respectively). Separate regressions
cover template replacement, unexpected template contents and injected final-mode
failure before confirmation. Existing payload-file permission preservation stays
unchanged.

This qualification is limited to ordinary Unix mode/umask behavior. It does not
claim preservation of custom ACL policies, alternate-parent group inheritance,
Windows DACL behavior or access for arbitrary different-UID deployments. The
repair neither copies ACLs nor changes UID/GID. Inherited policy remains that of
the library root, as before; differing observed ownership is an error. Metadata
mode bits alone are not proof of equivalent ACL access. [Linux ACL semantics](https://man7.org/linux/man-pages/man5/acl.5.html)
and [Apple mkdir semantics](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/mkdir.2.html)
describe the distinct mode/inheritance boundaries. Operators requiring custom
ACL/group guarantees need representative qualification before relying on them.

The cooperating-writer/trusted-filesystem limit remains: these identity checks do
not eliminate arbitrary concurrent hostile rebinding or in-place mutation by an
actor with equal filesystem authority.

## Verification status

Local: changed Rust parsed/formatted by the pinned toolchain, whitespace and
release attribution checks passed. No local monolithic build or native behavior
claim. New regressions use the existing `copied_import` native test filter.
Independent source review and fresh exact-head Linux/macOS/Windows Build results
will be recorded separately before acceptance.

## Combined migration report consumer correction

The integration adds the minimal adjacent consumers required by the new
`blocked_import_publication` result. API recounting preserves its skipped and
error counts even when partial-download report wording is rewritten. It never
changes the incomplete checkpoint timestamp or erases the per-model diagnostic.
The panel reports success only for an acknowledged completion timestamp, no
errors/skips, all planned moves completed, and consistent references. A consistent
reference graph alone cannot make retained work complete. This changes feedback,
not migration execution or recovery authority.

Local frontend qualification: all seven MigrationReportsPanel tests passed,
including blocked publication, absent completion timestamp, skipped work and
execution-error cases. Scoped ESLint and the full frontend TypeScript check
passed using existing workspace dependencies. The first TypeScript attempt lacked
the existing Electron dependency link; restoring that local test-environment link
resolved it. No dependency or lockfile was changed or installed. Rust recounting
has a `copied_import` regression for fresh native hosted execution.
