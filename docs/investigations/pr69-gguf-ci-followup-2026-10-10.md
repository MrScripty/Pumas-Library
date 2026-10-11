# PR #69: GGUF CI failure

## Observed run

[CI run 38095339520](https://github.com/MrScripty/Pumas-Library/actions/runs/38095339520)
ran head `98eb3baf0ccae6e455bf6d0a7017626cae62bf1c`. Native Windows,
macOS and Linux Torch QA passed. The headless core unit suite, including the
startup preflight regression, passed. Its acquired-GGUF integration test failed
at the awaited descriptor-refusal assertion. Nine other tests in that binary
passed. Downstream release jobs were skipped, not accepted.

The original failure did not identify which of twelve corruption cases ran.
Ten local runs of the original cohort with diagnostic assertions all passed;
that does not establish the original case or rule out an interleaving.

## Controlled reproduction and repair

The fixture disables HF but starts the filesystem watcher and startup work.
Both primary-selection paths use the same acquired-publication validator.
Descriptor refresh observes retained publication gates; it does not recreate a
missing index row. Filesystem reconciliation can recreate that projection from
intact canonical metadata, confirmed receipt and verified physical payload.

A temporary controlled probe used the actual `index_model_dir` operation between
synchronous refusal and awaited descriptor resolution in `missing_index`. It
reproduced the failed assertion: index before was absent; index after contained
the current Ready publication; resolution succeeded with the acknowledged primary.
This proves that legitimate repair can invalidate the old test's assumption.
It does not prove which unlabelled case raced in the original hosted run.

The negative test now drains the public instance shutdown receipt before fault
injection. A successful primary/descriptor resolution after drainage is the
positive control; shutdown cannot itself satisfy the negative assertions. All
twelve cases retain both refusal assertions and identify the corruption, result,
and before/after index evidence on unexpected success. No sleeps, assertions
weakened, receipt changes, or production-validator bypasses were introduced.

A separate live test calls `rebuild_model_index` on a running API, without a
restart. Missing rows and stale index paths/entry paths recover from a valid
publication. Corrupted primary/projector payloads and a pending receipt remain
refused after reconciliation; the invalid bytes/receipt remain unchanged.

## Verification boundary

Commands used the locked graph and native Linux test permissions, with
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, and
`CARGO_BUILD_JOBS=4`:

- `cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library
  --no-default-features --test acquired_gguf_vision -- --nocapture`: 11 passed.
- Same command with default features: 11 passed.

- Full `cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library
  --no-default-features`: 2,070 unit tests passed (13 existing ignored),
  219 integration tests passed, six doc tests passed (eight existing ignored).
  This is the complete failing headless CI command, without a test filter.

The controlled reproduction was removed. No `[DEBUG-ci-gguf]` instrumentation
remains. This repair is separate from download-location reconciliation, whose
plan and pending acceptance claims are unchanged. A hosted rerun still must
validate the revised source; these results are local Linux evidence.
