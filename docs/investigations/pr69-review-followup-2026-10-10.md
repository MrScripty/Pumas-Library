# PR #69 review follow-up

Scope: repair the two reviewed regressions, investigate audio qualification, and
selectively transfer/amend the reconciliation plan. Reconciliation implementation,
live-library repair, model inference and a new Debian release are outside this
follow-up. The plan remains Planned; its ten acceptance claims remain pending.

## Source and selective integration

GitHub main/base was `ad31e391dcd94c204b3fcd748cc98d27b3281e65`; PR #69's
reviewed head was `1a78ea8ecbe3ddf589d84db6783577eaf4efb79d`. Both remote refs
were verified before work. Local review branch: `fix/main-test-validation`.

| Source branch / commit | Accepted replacement | Integration / verification |
| --- | --- | --- |
| `work/acquisition-q1-http`, investigation `1568e90c` | `a36b91e8236a65b05bc4f6e3e2fc9fe7e79d511b` | Selective document reconstruction; investigation matches the source blob exactly. |
| `work/acquisition-q1-http`, plan `0b97dcb2` | `a36b91e8236a65b05bc4f6e3e2fc9fe7e79d511b` | Only the eight planning documents transferred, then amended for this review. Links, all ten pending claims and the exact staged scope checked; active hooks passed. |

The old branch was not merged. Its unrelated acquisition edits and untracked
document remain user-owned, with the tracked binary diff verified byte-identical
to the preserved pre-task baseline. The original branch is retained for that
work, not retired by document transfer. The locked PR worktree is retained for
the maintainer's push/review under the [ledger's resource contract](../plans/download-location-reconciliation/execution-ledger.md).
No published history was rewritten and no push was performed.

## Regression repairs and observed evidence

The missing constant was reproduced with Windows GNU cross-target compilation:
`cargo check --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --tests
--target x86_64-pc-windows-gnu`. Before the fix it failed with E0425 at the
unconditional cancellation test's timeout reference. The same check passed after
`f6fb33fc` removed the Linux-only constant gate. The actual Linux cancellation
test passed, one test selected. The existing Windows/macOS CI jobs show the same
E0425 at the reviewed head; native Windows MSVC/macOS jobs still require a rerun.
GNU cross-compilation is not native-platform execution evidence.

The headless startup regression was reproduced by running the exact core test
`tests::acquisition_integration::acquisition_integration_startup_refuses_legacy_store_without_rewriting_it`
with `--no-default-features --lib`. Before the fix, registry absence failed after
legacy refusal. After `58fa0382` removed the feature gate, the same test passed.
Runtime HF enablement now controls preflight, matching actual initialization.
Catalog-query and HF-disabled behavior retain their existing conditions.

Focused HF-disabled compatibility also passed: one test selected,
`ordinary_builder_leaves_legacy_state_read_only_until_explicit_offline_migration`
in `--no-default-features --test artifact_acquisition`. It verifies supported
legacy reads remain unchanged until explicit migration.

The full core library suite with `--no-default-features --features hf-client
--lib` passed: **2,070 passed, zero failed, 13 existing ignored**, with no filter.
This includes the startup regression. The full `--no-default-features --lib`
suite also passed: **2,070 passed, zero failed, 13 existing ignored**, no filter.
Commands used the locked dependency graph,
repository toolchain and native subprocess/loopback permissions, with
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_BUILD_JOBS=4`.
Formatting and diff checks passed. Other workspace/frontend/Electron/release
suites were not rerun for these two Rust lines; this is not a full candidate QA
or release claim.

## Audio qualification: evidence and remaining blocker

The [failed CI job](https://github.com/MrScripty/Pumas-Library/actions/runs/38090970191/job/114327241132)
at `1a78ea8e` installs successfully, then refuses during source-pinned recipe
validation, before dependency confinement qualification. Its retained artifact
contains only source identity and the probe log; installed `runtime.json` is
absent. The relevant installer, recipe and workflow files were unchanged by the
reviewed PR range. Exact attribution to a particular selected field in that run
cannot be recovered from its generic error alone.

Three hypotheses were investigated in order:

1. Interpreter selection drift. The installed pinned uv `0.12.18` catalog selects
   CPython `3.12.14` for Linux x86_64 GNU, matching the recipe's distribution and
   source URL. Provider version/archive pin also match source. This observation
   does not attest the missing CI executable bytes.
2. Dependency resolution drift. Qualification requests Python minor `3.12` and
   uses the general installer; `resolve_runtime.py` leaves core/cohere-asr
   dependencies partly unpinned. A resolution-only probe with the existing pinned
   uv, the recipe's exact official CPU Torch wheel URL, and the current source
   CORE/cohere-asr constraints resolved the same 68 package names, but differed:

   | Dependency | Recipe pin | Current resolved version |
   | --- | --- | --- |
   | NumPy | `2.5.3` | `2.5.4` |
   | narwhals | `2.26.0` | `2.27.1` |

   This is a concrete counterexample to reproducible recipe selection and
   supports dependency drift as the likely cause. The probe uses uv resolution,
   whereas the actual installer uses pip; it is not the missing original CI
   selection or installed-byte/confinement evidence. No packages or models were
   installed by the resolution-only probe.
3. Installed member/metadata drift. Exact retained interpreter/dependency bytes
   cannot be checked without a successfully installed matching candidate.

A replay of the unmodified `qualify_audio_dependencies` example used a fresh
ignored fixture under the original repository's
`launcher-data/cache/torch-qualification/`, never the live library. It failed
earlier with `Pinned uv archive download was interrupted`; it did not reach the
recipe comparison. No model weights were acquired. An initial resolution probe
also exhausted local disk; its retry after build-cache cleanup succeeded.
Cache cleanup interrupted one feature-check compile; that invalid run was
replaced by the successful complete HF-enabled core suite above.

The workflow now preserves the installed runtime selection with `if: always()`
alongside the existing source/probe artifact, even when recipe validation fails.
It captures only fixture runtime metadata, not environment variables, live
library data or models. The bounded existing artifact retention remains seven
days. The recipe, acquisition checks and qualification gates are unchanged.
Workflow structure passed CI-pinned actionlint `1.7.8`; local shellcheck/pyflakes
were unavailable, so those subchecks remain native CI obligations. The new Bash
step passed syntax checking and actual absent/present fixture replay, including
byte-exact copying and upload-path selection.

Audio candidate qualification remains **unsatisfied**. Next: rerun CI after the
maintainer pushes, compare its captured installed selection with the source
recipe, and make qualification acquisition reproducibly select the reviewed
recipe through the existing owned installer. Do not merely update pins to the
latest resolver output or bypass byte validation. A passing dependency recipe
alone still does not prove confinement, controlled-session lifecycle or model
inference; run the actual required qualification chain before acceptance.

## Plan amendments

The [amended plan](../plans/download-location-reconciliation/plan.md) now requires
an explicit warning before the first HF storage upgrade, cancellation without
upgrade, and separate direct-library versus upgraded-RPC-server tests using the
actual Lanternwake/Eidetic pinned dependency revisions. Those sources/pins are
not available in the adjacent checkouts; obtaining and testing them is a blocking
C5 input before recovery UI expansion, not assumed compatibility.

It also distinguishes durable queue-admission release from live destination
ownership. The existing `DestinationExecutionOwner` restores dormant claims,
while generation-scoped release alone cannot settle them. M1 must settle the
exact restored claim after durable publication and before success, fence stale
inventory/generations, allow a new authorized same-destination transfer immediately
without restart, and preserve both unrelated paused downloads and their claims.
Interruption between publication and live settlement is explicitly tested.

Coding standards MCP was rerouted at
`snapshot:v1:f7ec5a4e-affb-4206-91ad-a4b1f74260fe`, schema 5; both pages read,
42 policies selected and zero unresolved fact categories. Source repairs preserve
the existing owners; amendments add independent consumer/effect observations
instead of accepting projection agreement. No reconciliation code-compliance or
release acceptance is claimed before its implementation and required evidence.
