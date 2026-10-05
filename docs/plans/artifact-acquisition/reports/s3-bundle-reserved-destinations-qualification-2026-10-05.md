# Reserved acquired-bundle destination correction — 2026-10-05

Tested implementation milestone: `2defee5c1d85b69f0e2b73e51d9c1980616a1d1d`; tree `89e7525a57143f1bb5b39bc0e061e8532db389bc`.
The subsequent documentation-only commit and final remote state are recorded in
`/workspace/scratch/s3-bundle-reserved/final-state.json`.

## Scope and lineage

Separate branch `fix/s3-bundle-reserved-e8082649` starts at frozen
`e8082649a6d4aaebf996c30870d996d43bbee1d3`, tree
`050733c55a3011f23c8832e210d52d0d4d60d695`. The explicit accepted PR40
merge `d9d907cfe0e57b1e2bc7e296eff1327de835bc5e` and its ordered parents
339032ff / 838eb299 are preserved. Parent owns PRs/review/integration.
No zero-length implementation is included: acquisition reader and manifest,
frontend/Electron/generated wire DTOs, dependencies, watcher and existing
publication/receipt/recovery machinery remain byte-identical to e808.

The reviewer identified a P2 preflight gap: `metadata.json` could pass desktop
bundle validation, transfer and enter durable Using with a consumer receipt,
then fail the importer's reserved filename check. Retained work prevented
ordinary subsequent admission. That frozen candidate remains changes-requested
until this correction is reviewed/integrated; this report does not claim review
acceptance or alter its frozen ref.

`ModelImporter::validate_acquired_payload_paths(&[&str]) -> Result<()>` is an
additive pure public preflight. The acquired copy plan's existing normalized
first-component rule is extracted once and reused by that method and final
publication. The same existing `IMPORT_MUTABLE_DOCUMENTS` table owns reserved
metadata, backup, overrides and publication receipt roots, aliases and
descendants. No second blacklist or normalization policy is introduced.

RPC bundle validation invokes the importer rule after shared portable-path /
manifest checks and before job admission; anonymous and authenticated paths
share it. The public native S3 workflow invokes it before creating its consumer
or resolving source objects. Full path, immutable identity, byte verification,
model format and final publication authorities remain required. Existing wire
shapes and generated schemas are unchanged; the Node structural decoder does
not grant importer destination authority. No new RPC/UI policy copy is needed.

## Regression evidence

The pure regression enumerates all importer-owned reserved roots, case aliases
and descendants, plus `_metadata_.json`; valid `config/metadata.json` and
single GGUF paths remain allowed. The native test checks rejected paths before
HEAD, any acquisition/store mutation or staged file creation, then publishes a
corrected request on the same owner. The production HTTPS/RPC fixture checks
`metadata.json`, `overrides.json`, both reserved-root descendants, case and
normalization aliases, metadata backup and publication-receipt descendants:
standard invalid params, idle state, no reserved workspace, no acquisition or
model, followed by successful corrected admission using the same operation ID.
The captured source sequence consists only of the corrected operation's pins.
Existing anonymous/authenticated signing, cancellation, receipts, retained
failures and credential redaction continue passing in that fixture.

Logs and hashes: `/workspace/scratch/s3-bundle-reserved/verification.json`.
Commands use Rust 1.92.0 / four jobs / no debug or incremental data / isolated
XDG config, the already inspected standards snapshot 188beda1 and existing
library/Rust/security/async/contracts/verification/commit routing. No AGENTS or
repository `.agents/skills` were present. Checks selected for this correction:
S3 native workflow (7 pass plus isolated child), importer units (122 pass,
1 ignored plus owned child), source-bundle RPC (2 pass and eight HTTPS scenarios),
no-S3 source regressions, strict all-target enabled-S3 core Clippy, and RPC
Clippy with the inherited explicit `-A dead_code` allowance. Exact commands,
exit statuses and counts are recorded in verification JSON. Formatting,
diff checks and unchanged/frozen-path/ref checks are retained there.

The initial compile omitted the `ModelImporter` import in the native module;
it was corrected before passing qualification. No dependency or security
workaround was required. No real credentials, live-provider/account mutation,
paid service, external reviewer contact or sandbox bypass occurred.

## Separate successor and pending acceptance

The separately delegated zero-length capability remains on
`feat/s3-empty-members-e8082649`; these are sibling successors and are not
silently combined. Parent owns their review and composition. A corrected
native request is proven operational here; existing retained Using work from
older binaries still requires its existing reconciliation path and is not
cleared or replayed by preflight. Packaged/browser/native-platform and
real-provider qualification remain outside these fixture claims; AQ-S3 and Q4
are not advanced. The next implementation slice is the separately delegated
zero-length member capability, followed by authorized Q3 provider acceptance.
