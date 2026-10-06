# Final selected-projection comparison bound repair — 2026-10-06

Independent review of frozen `31b5c12536b07a69e7c691e90e4219490f3f3e36` found that the final comparison of
projection.json, roots.in and constraints.in used the global unbounded
`validate_torch_provenance` reader. Earlier selection-local bounded reads did not
protect this final post-checker call. Refusal occurred after allocating/reading
the replacement, and the async timeout could not cancel that blocking read.
This report corrects the historical whole-selection read-bound claim for these
three files. Review otherwise found no false-success semantic issue.

The narrow successor preserves owned-path and exact-content checks and delegates
only the selection-local final comparison to existing `selection_bytes` at each
retained input's expected length, after confirming that length is within admitted
MAX_EVIDENCE (2 MiB). The existing reader rejects non-regular/linked/oversized files
before opening data; racing growth is limited to the expected length plus one
bounded detection byte. Shorter and same-size changed content also refuse. No
assumption that timeout cancels blocking work is used to establish this bound.
The global function/interfaces and every other owner/consumer remain unchanged.

## Exact source

- Existing branch: `feat/torch-offline-selected-3f573449`.
- Frozen base `31b5c12536b07a69e7c691e90e4219490f3f3e36`, tree
  `f28400db93069442502b747107e02b9b8c27aabf`.
- Tested source `b724d5056fd898625ed9d1a51d82425563901aa6`, tree `fffe76996b6d62f470ab5b9cdb1b23e699938325`.
- Source commit changes only the local Rust module (14 added lines/one changed
  call), its fixture controls and pre-edit write-set admission. Final evidence
  successor changes documentation/evidence only. No history rewrite or main merge.
- Main observed/preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.
- [Source and evidence hashes](torch-selected-projection-bound-2026-10-06/provenance.json).
  [Frozen proof](torch-selected-projection-bound-2026-10-06/frozen-proof.json)
  records unchanged production module bytes before reproduction, the regression
  patch hash, and nine retained global/owner/helper/native/S3/manifest file hashes.

## Actual reproduction and correction

The saved [regression patch](torch-selected-projection-bound-2026-10-06/regression.patch)
adds two actual shared tests, each covering all three retained inputs. A genuine
public solver and independent checker first finish successfully. Only afterward
the fixture grows the chosen file to a 32 MiB sparse regular file or atomically
replaces it with same-length different bytes. The private result refuses; the
legitimate catalog snapshot receipt stays Adopted, its granted five original
acquired wheels remain held, no installation metadata appears, and cleanup drains.
The same-size replacement verifies exact-content validation rather than merely
checking length. Untouched valid selection passes the aggregate control.

On unchanged frozen production bytes both refusal tests pass semantically; that
alone cannot test the claimed resource bound. Actual kernel `strace -f -yy` of
read/open/close demonstrates the defect separately: **each grown retained file
returns 33,554,432 bytes in a single read** before refusal. This directly measures
the unbounded call instead of inferring safety from a Refused result or timeout.
The repaired growth control observes no oversized read. All-process totals for
matching fixture paths (including the small reads before mutation) are:

| Input | Frozen largest read | Repaired largest read | Repaired total bytes |
|---|---:|---:|---:|
| projection.json | 33,554,432 | 3,911 | 11,733 |
| roots.in | 33,554,432 | 18 | 54 |
| constraints.in | 33,554,432 | 1 | 3 |

[Kernel-read summaries](torch-selected-projection-bound-2026-10-06/repaired-reads.json),
their frozen companion, focused unmodified trace lines, analyzer and independent
`verify_read_bounds.py` volume assertions are retained adjacent to this report.
Complete traces remain locally at
`/tmp/pumas-q2-selected-projection-{frozen,repaired}.trace`; their hashes are
recorded. Only matching fixture input reads are copied into repository evidence,
keeping unrelated execution trace outside the review patch.

## Qualification

**60 Rust tests** (21 catalog/selection plus 39 existing adjacent/handoff),
**110 Python tests**, **33 pre-build feature graphs**, offline/locked strict
Clippy, scoped Rustfmt/Ruff and whitespace checks pass. No configured-tool
control is skipped. Six new growth/replacement subcases all follow genuine
checker completion and refuse with retained grant/receipt. Prior direct-root,
extras/closure, target/interpreter, resolver-error, namespace/input mutation,
abandonment and shared consumer controls remain passing.

```text
python scripts/release/check-dependency-features.py --s3
# Reproduction compile: exact frozen module with only the saved fixture patch.
cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support selected_checker_cannot_hide_retained_projection_ --no-run
PUMAS_QUALIFIED_UV=<existing qualified tool> strace -f -yy -e trace=openat,read,close -o <trace> <exact built test binary> selected_checker_cannot_hide_retained_projection_ --ignored --test-threads=1 --nocapture
# Repaired read-volume trace runs the growth control; aggregate covers replacement too.
cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support version_manager::installer::torch::torch_ -- --include-ignored --test-threads=1 --nocapture
cargo clippy --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support --tests -- -D warnings
```

Cargo uses the existing admission flock, jobs 1/debug 0/incremental 0 and offline
locked cache. Reproduction/final builds, raw trace-control logs, aggregate logs,
source push and proof checks are saved in the adjacent evidence directory. Python
reruns the same six affected suites from prior qualification with the already
provisioned qualified uv binary. No new download, credential, provider, network or
host configuration change occurred.

No public API/schema, global provenance reader, dependency/tooling, target or
source/version/fallback policy change. No automatic/preview activation, real
provider/platform/installed acceptance or AQ/P1 advancement. Native/S3/manifest
write sets remain frozen and both Library transfers stay paused. Existing native
glibc 2.41/tool-projection, provider/volume, tool distribution and consumer gates
remain unchanged. Next existing-plan slice stays separately admitted selected-
packet local-consumer composition; parent owns further review/CI/PRs/merges.
