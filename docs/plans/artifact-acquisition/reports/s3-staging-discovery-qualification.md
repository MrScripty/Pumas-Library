# S3 staging discovery qualification

## Source and checked boundaries

The non-force push of `fix/import-discovery-s3-version` was verified by fetching
the remote branch: tip `a606c80b0be41580c7af0583703d9f9af5f2cb44`, parent
`cab3891232942d9e13d8cb8b74184f13cd2aa747`, over merged PR39
`05717338c2aea737483fb4b28c3ed4053a65de96`.

Those fixes passed 32 reconciliation tests, 40 S3 acquisition tests, 11 S3 reader
tests, strict enabled-S3 all-target core Clippy, formatting and diff checks.
The discovery regressions failed before staging-event exclusion. The distinct
VersionId regression failed with `Manifest(ConflictingSourceEvidence)` before
version-qualified manifest keys. Same-key/same-version digest and size conflicts
remain refused; consistent evidence may serve two distinct logical paths.

The `5fb3f1ea` successor changed only tests and qualification documentation. Its
production source retained the two fixes above. The added test is
`manifest::staging_acceptance::large_s3_bundle_with_live_watcher_publishes_and_cold_recovers`;
its source blob is `65c0f54ada03ac23d18145c661db78aa1e3af02e`.

## Payload-name review correction

The original watcher filter also hid accepted final payload basenames such as
`llm/family/model/.tmp_import_weights.gguf`. Staging exclusion now applies to
directory positions and ancestors, plus existing nested directories, rather than
every prefixed leaf. Deleted stage descendants and root-level staging-directory
events remain excluded. A missing prefixed leaf below the model root is
conservatively visible because the path-only callback cannot prove it was a
directory. Import admission and version-qualified S3 manifest keys are unchanged.

New regressions cover raw Modify/Remove event forwarding and mixed staging and
published-payload batches retaining the model's dirty mark. These Rust regressions
require exact-head hosted qualification; the historical results below do not
claim to execute this correction.

## Hosted publication/discovery race correction

Exact-head hosted execution at `63123fd8f9f866064a8315096ab3fb1fc81e8f0f`
reported 39 passing and two failing S3 acquisition cases in
[the headless integration job](https://github.com/MrScripty/Pumas-Library/actions/runs/37326608913/job/111819229166).
The live-watcher case reached owner shutdown with a background reconciliation
failure: discovery attempted to reclassify an unacknowledged copied publication.
Source inspection also found discovery could rewrite its Pending index snapshot
before the producer's conditional Ready commit. A two-second notification
suppression does not provide publication ownership.

Unacknowledged copied-publication observations now acquire the existing native
root grant, reread their evidence, and retain exclusion through conditional
index projection. Busy observations defer through the existing reconciliation
path. Terminal unavailable publications retain diagnostic projection, but are
not reclassified. Legacy and acknowledged Ready projections keep their existing
paths. No new publication owner or recovery API is added.

The second failure, `missing_index_publication_identity_cannot_settle_changed_output`,
reported only `RecvError`: its fixture awaited a dropped callback before checking
the acquisition/import result. The fixture now reports that underlying result
first; this log cannot establish that it shares the first failure's cause.
Deterministic regressions cover live Pending/Ready-metadata observation, deferred
owner shutdown, cold admission exclusion, and preserved terminal diagnostics.
These changes require fresh exact-head hosted Rust qualification; the historical
passing runs below do not qualify this correction.

## Normal workflow evidence

On Linux x86_64 with pinned Rust 1.92.0, the real anonymous loopback S3 reader
resolves two exact versions, verifies one GGUF and a JSON auxiliary just over
3 MiB, and carries a valid import-spec callback just over 3 MiB (below the
existing 4 MiB admission bound). Pumas' normal metadata notifier and watcher
remain intact. A separate read-only OS watcher must observe the nested config
file event under the actual temporary-import directory; absence of that event
fails the test rather than being reported as coverage.

The importer must publish successfully. The public `get_model` operation must
return the published ID, with one discovered model directory and exact auxiliary
bytes. Consumer use must be Adopted. Consumer and API shutdown must succeed.
After closing the first owner, a new owner proves the complete output through
the existing bundle reconciliation operation. Publication receipt bytes,
auxiliary bytes and the adopted acquisition record stay unchanged. The source
observes exactly four requests across transfer and cold proof, proving recovery
does not replay source access. Acquisition/import, staging-event observation,
public `get_model`, and both API close calls have explicit 30-second deadlines.
Cold reconciliation and consumer shutdown are not individually timeout-wrapped;
the enclosing test command supplies the overall timeout.

The new test passed alone and in the focused aggregate. Final results: 41 S3
acquisition tests, 11 reader tests and five HTTP acquisition tests passed, with
zero failed or ignored tests. Strict core Clippy with warnings denied passed for
all targets with S3/test-support enabled. Formatting and diff checks passed.

From the repository root, after activating the retained Rust toolchain:

```bash
export PUMAS_REGISTRY_DB_PATH=/workspace/.pumas-tools/s3-test-registry.db
export CARGO_PROFILE_TEST_DEBUG=0
timeout 1200 cargo test --locked --manifest-path rust/Cargo.toml \
  -p pumas-library --no-default-features --features s3,test-support \
  --test s3_acquisition --test s3_reader --test artifact_acquisition -j 4
CARGO_PROFILE_DEV_DEBUG=0 timeout 1200 cargo clippy --locked \
  --manifest-path rust/Cargo.toml -p pumas-library --no-default-features \
  --features s3,test-support --all-targets -j 4 -- -D warnings
./scripts/rust/check.sh fmt
git diff --check
```

Registry state is disposable local test configuration, not a credential.
Logs are retained in `/workspace/.pumas-tools/s3-acceptance-suite.log` and
`/workspace/.pumas-tools/s3-acceptance-clippy.log`.

## Failed experiment and limits

The first fixture replaced the public metadata notifier with a blocking staging
barrier. That replacement also removed the builder's normal watcher suppression;
the run failed with a post-publication optimistic index conflict. Its source and
log remain in `/workspace/.pumas-tools/s3-staging-notifier-experiment.rs` and
`/workspace/.pumas-tools/s3-staging-notifier-experiment.log`. It is not passing
acceptance evidence. The final fixture leaves the normal notifier intact and
observes the real workflow instead. Custom-notifier composition remains
unqualified. The separately committed reconciliation tests own deterministic
stage exclusion and visibility of final publication events.

This adds local AC13/AC14 supporting evidence only. It does not establish measured
resource ceilings, every interruption boundary, hard kill/power loss, sharded
weights, live AWS/non-AWS/MinIO, authenticated or refreshing credentials,
source-facing desktop configuration, hosted review/CI, or native/GPU inference.
No credentials, permissions, network policy, speed, credits or privacy settings
were changed. AQ-S3 and the full acceptance criteria remain not ready.
