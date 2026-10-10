# S3 preflight expectation repair — 2026-10-09

This follow-up starts from frozen authored-bundle RPC commit
`c6069ce9104c92cc4c31bf477256b69b67b2e302`, whose exact native parent is
`766f4784b97cde38dac897985a41c21eb4b1a5b9`. It changes tests and this report only.
Production transport, shared package qualification, publication, custody and
custom-code policy are unchanged. Main and version files are unchanged.

## Demonstrated cause

The older `source_bundle_structural_preflight_refuses_entire_set_before_admission`
asserted that selecting `config/run.py` must be rejected at transport admission.
Fresh exact-parent compilation with diagnostic-only test instrumentation, and
current candidate instrumentation, both identify that exact stale table row.
Removing that row alone makes the exact-parent test pass, including all remaining
structural refusals. The original failed logs are preserved verbatim with hashes.

Commit `bb8a8956895f4ccf9d04a615f3482bdf8b17c6d5` intentionally removed the
auxiliary extension whitelist from `S3BundleImportParams::validate` as part of the
format-independent public bridge. Byte selection must not assign model semantics.
This is an obsolete test expectation, not a fixture configuration issue or a
production transport regression.

The full current S3/RPC run found a second copy of the same obsolete expectation:
`source_bundle_rpc_https_complete_pins_totals_cancellation_and_redaction` expected
`run.py` to return InvalidParams before transfer. Running its owned TLS child
directly exposed the exact assertion (null error code versus `-32602`). Both
obsolete Python-path rejection rows are now removed. All other namespace,
reserved-destination, pin and hash rejection cases remain, with better diagnostics.
A focused admission test checks that root/nested Python, a safetensors shard and
unknown bytes preserve their selected paths without granting model qualification.

The shared importer still refuses Python package members (`AUXILIARIES` excludes
`py`) and custom-code `auto_map` documents. The production authored-bundle suite
checks verified custom-code bytes retain a consumer receipt but produce no model.
Downloading or admitting selected bytes does not authorize their execution.

## Qualification and ownership

Detailed commands, counts and failures are in the accompanying local/Library
source-and-evidence packet. Cargo runs use official cached dependencies,
`--offline --locked`, `ORT_SKIP_DOWNLOAD=1`, isolated owned XDG configuration/cache,
a rejecting proxy for non-loopback traffic and serial test execution. No provider
campaign, inference, release, version change or main modification is performed.
The exact-parent diagnostic worktree is restored clean after qualification.

The full S3-enabled RPC run completes with 247 passed, three failed and thirteen
ignored tests (including 226 passed/three failed/one ignored unit tests). All five
integration targets pass, including the actual 32 authored-bundle and 18
single-object production-process cases. Native S3 integration has 82 passed and
one ignored test; the ignored sharded cohort requires its separate HTTPS
controller. The native S3 unit set (`--lib s3`) has 46 passed and two ignored
child fixtures that their parent tests execute separately. The full no-S3 RPC
profile has 233 passed and twelve ignored tests. All 20 desktop S3
contract/transport checks pass.

The three RPC failures reach the physical-store ownership gate during cold
reopen. The single import and prefix-discovery tests visibly retain old library
and acquisition clones. The persisted-inspection test reaches the same gate
without those explicit clones before its first reopen; its exact retained owner
has not been localized here. Do not classify that third failure as a proven
fixture-only issue or claim a production ownership fix. Main owns the pending
runtime/cold-reopen work, and those test sections remain untouched. Their exact
errors are preserved. This follow-up does not claim the entire S3/RPC suite is
green or relax the physical lifetime lock.

## Integration

Apply this narrow follow-up after the frozen `c6069ce9` authored-bundle source.
When composing with main's pending combined integration, preserve main's
cold-reopen fixture edits and apply only the preflight table correction, positive
admission test and diagnostic hunks in `s3_imports/tests.rs`. Do not replace that
whole file. This follow-up changes no schema or generated client. Retain the
previous requirement to regenerate both combined clients once after composing
the main and authored-bundle schemas, then rerun the full S3/RPC suite.
