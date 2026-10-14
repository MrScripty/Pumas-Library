# v0.8.0 source checkpoint

Checkpoint owner: parent cross-repository integrator. Preparation date:
2026-10-14. Runtime source baseline:
`ef9eead9fe6595f5b3152ef8df19c5dace30190b` (merged PR #69).
The immutable qualified code revision is
`5da74a2c9bd1fe3c0b1fa0184eef3667f31eed88`; its subsequent documentation commit
adds this handoff without production changes. [Draft PR #70](https://github.com/MrScripty/Pumas-Library/pull/70)
owns review of both. The [machine-readable receipt](plans/v080-readiness/reports/candidate.json)
binds source/tree, lockfiles, input hashes, local artifact bytes and each gate.
This slice changes release information and launcher
fixtures, not production APIs, runtime code, dependency resolution or manifests.
Version remains `0.8.0-rc.1`. No tag, final release or accepted archive is created.

## Current consumer inventory

These are read-only observations of consumer `main` on 2026-10-14, not successful
downstream builds. Each consumer owner qualifies the actual dependency edge used
by its demo; unrelated repositories do not need to release together. A Pumas-only
test cannot accept the external application.

| Consumer / inspected revision | Actual Pumas boundary | Required next acceptance |
| --- | --- | --- |
| Pantograph `2253cbe6fe8f1969b347c61fccba31859b13a4ba` | Cargo.toml, Cargo.lock and `.github/workflows/quality-gates.yml` all pin `aa1d79ae2d53f0acb088bc85223ecc2ff46af12b`. Native Rust library consumer. | Coordinate all three pins to one reviewed Pumas SHA, retain lockfile resolution, then run actual locked Rust/library/IPC and lifecycle tests. This inventory does not establish source compatibility across that revision gap. |
| Chrema `508c8b8cb724b49c655dab8bc4c8336190e516c7` | Current main contains reusable workflow/content-ledger packages and host-supplied provider adapters; no Pumas integration is present. | The Chrema/demo owner selects and publishes the actual adapter cohort. Historical Pumas `qualify_chrema_native.py` requires Chrema `d9f660beb8273aac16e62fae3902b9f5e4d1559b` and Pumas `fe3c9f04c243f436a393ae47e16f616244a85cee`; it cannot be relabeled current-main acceptance. |
| Tuldok `d856a984535a430909761ed13ce42eadd55b0710` | `docs/contracts/pumas-image-to-text-consumer.md` and fixture `source.json` pin merged Pumas `36c2aab2c885b8416fd4e39cf8c37c510596d6e4`. Opt-in typed HTTP caption consumer. | Retain its seven-variant parser and six actual DTO-schema pins, then qualify final source/schema advertisements, finite real model/projector captioning, cancellation and borrowed-owner survival. The inspected operation DTO definitions are unchanged from that merge; this is source comparison, not a new schema-export or model receipt. |

Protocol/schema advertisement, exact selected owner/root/generation, artifact
digest, model/profile and runtime identity remain separate compatibility facts.
Do not substitute a product-version match or a successful health check for them.
The [release notes](release-notes-0.8.0.md) link the supported migration path.

## Evidence from this preparation

Environment: this Cloud Linux x86_64 container, Rust/Cargo 1.92.0 from the repository
pin, Node 24.19.0 (repository pin 24.15.0), Python 3.12.14 (repository pin 3.12.3).
Node/Python results are supporting compatibility observations on these patch
versions, not a claim that the exact pinned environment was run here. No models,
native inference runtime or external S3 account were provisioned by this task.

| Command | Observed result / proof boundary |
| --- | --- |
| `node --test scripts/release/*.test.mjs` | 33 passed, zero failed/skipped; existing release/packaging fixture contracts, not actual release archives |
| `npm run check:release-versions -- v0.8.0-rc.1` | Passed; aligned workspace source manifests |
| `npm run check:dependency-ownership` | Passed; workspace ownership declarations |
| `node scripts/release/check-attribution.cjs` and `--features s3` | Both passed; current inputs/pinned notice text freshness, not final SBOM/security approval |
| `npm run test:launcher` | Initial 47 passed / 2 failed. After fixture correction, 49 passed / zero failed/skipped. See diagnosis below. |
| `cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library --no-default-features` | Passed: 2,070 unit tests, 218 integration tests and 6 doctests; 29 ignored entries including child helpers/doctest examples. Catalogue recovery: 7 passed, 1 child helper ignored at top level. Feature-gated S3 targets contain zero tests in this configuration. No pretrained inference is exercised. |
| `cargo test --locked --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features` | Passed with task-local CMake and XDG configuration: 233 unit and 24 integration tests; 13 ignored entries. S3 feature-gated process targets contain zero tests. |
| `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | Passed |
| `python3 scripts/release/smoke-rpc.py rust/target/debug/pumas-rpc --inference-disabled` | Passed HTTP health, inference-route absence and graceful stop against the rebuilt development test binary |
| Two selected `NativeConsumerTests` in `scripts/consumers/test_local_http_session.py` | Passed actual owned start/borrowed retention/cleanup and borrowed cancellation preserving the external owner. This is Pumas's source reference, not acceptance of an external application. Exact command is in the receipt. |

Raw preparation logs are task-local `/tmp/pumas-v080-*.log`;
they are not permanent downloadable release assets. The execution ledger and PR
retain reviewed result summaries.

The initial launcher failures had distinct observations: an immediate Node child
exited after 146 ms, exceeding the negative fixture's 100 ms threshold; a killed
descendant remained as Linux state `Z` under container PID 1. The fixture now
allows one second for the early-exit discriminator and checks Linux execution
state rather than interpreting every retained PID as live work. Actual process
group termination and force/close assertions remain exercised. Production
launcher code is unchanged. Native Windows/macOS launcher evidence is not run here.

RPC's first attempt stopped before tests because CMake was missing. The second
link attempt terminated with SIGBUS while the container filesystem was full;
sandbox startup independently reported ENOSPC. After task-local CMake 3.31.6 and
removal of completed task-built test executables, the unchanged RPC invocation
passed. These attempts and log digests remain in the receipt.

The local test binary is 752,728,784 bytes, SHA-256
`da0bb23c6a4889baca2bc8500a07d07a38457cb43c2999d324d6d8a5419b04ed`.
Its actual advertisement lists `pumas-library/hf-client` and
`pumas-library/test-support`, with no inference feature. Source/target provenance
fields are absent; checkout equivalence is recorded independently. This debug
test executable is not an optimized, self-authenticating or shipped artifact.
No native inference runtime was loaded. The original saved executable is retained
separately; its path and hash are in the execution ledger.

Current local execution does not include all-feature workspace/Clippy,
enabled-S3 source cohorts, frontend/Electron/Torch suites, the explicitly ignored
kernel-confinement gate, external consumer applications, native Windows/macOS,
optimized packaging, pretrained inference or final security/SBOM/signing review.
The receipt lists each as not-run; historical CI never silently closes them.

Historical PR #69 head `036bed9cb60a91a440034b7324fd6f611ea58c2a`
records successful [Build 38247599905](https://github.com/MrScripty/Pumas-Library/actions/runs/38247599905)
and [CodeQL 38247594558](https://github.com/MrScripty/Pumas-Library/actions/runs/38247594558).
Its Linux extracted inference archive exercised finite 256-dimensional embeddings
through real ORT with an untrained graph; its audio job acquired the frozen cohort,
imported dependencies and performed a CPU operation with `production_available=false`.
These are historical recorded CI results for that head, not this executor's
inference tests, pretrained quality acceptance or final candidate artifacts.

## Release blockers and later scope

| Item | Disposition / owner / required evidence |
| --- | --- |
| Earlier headless catalogue ownership timing failure | Did not reproduce in the complete current core headless cohort, including actual catalogue cold-recovery tests. Do not carry it forward as a current reproduced blocker. |
| Consumer dependency edges | Parent selects only the edges used by each demonstration. Pantograph source repinning requires its locked consumer checks; Tuldok captioning requires its advertised schema and model path; Chrema requires an actual published adapter. Independent release schedules remain independent. |
| Final version and notices | Parent decides promotion from rc.1 to 0.8.0. Change the four manifest sources and matching workspace lock entries without dependency updates, regenerate/review both versioned attribution sets, run version and release tests |
| Sixteen exact candidate artifacts | Blocks release publication: all native targets and package/install/start/recovery/shutdown flows required by RELEASING.md, plus exact checksums/provenance/SBOM and dependency/security/license review. GitHub's push notice reported 52 default-branch dependency alerts (27 high, 19 moderate, 6 low); these were not independently triaged against the shipped closure here. |
| Pretrained ONNX and vision/runtime evidence | Blocks claims of those production/demo capabilities. Use explicitly provided model/tokenizer/projector fixtures and the extracted pinned runtime; synthetic graphs and provider responses do not satisfy this |
| Production installed speech | Later capability scope while admission stays unavailable. Its owner must satisfy actual model/runtime/read containment/disposal gates before advertising support; do not expand this checkpoint to enable it |
| Interactive download-location reconciliation, custom Torch assembly, new SDKs/network discovery | Later scope under their existing owners; do not treat plans as delivered features |

## Exact parent release action

First review/merge this draft and record the accepted full candidate SHA. Source
demonstrations may pin that immutable revision within their explicitly qualified
capabilities; accept each actual consumer edge separately. They need no final tag.

If the parent selects the unchanged prerelease version after required pre-tag
qualification, its candidate action is `git tag -a v0.8.0-rc.1
<accepted-candidate-commit> -m 'Pumas Library v0.8.0-rc.1'` followed by
`git push origin refs/tags/v0.8.0-rc.1`. It must first verify that this tag does
not exist. This task does not execute either command.

After the final-version commit and all **pre-tag** QA in RELEASING.md are accepted,
the parent/maintainer may run the following with the actual reviewed full SHA:

```bash
npm run check:release-versions -- v0.8.0
git tag -a v0.8.0 <accepted-final-version-commit> -m 'Pumas Library v0.8.0'
git push origin refs/tags/v0.8.0
```

This is a proposed future action, not an executed command. Current rc.1 manifests
must not be tagged v0.8.0. The tag triggers candidate assembly, which must pass on
that exact SHA for all sixteen planned files. Inspect native results and final
metadata, stage a draft release with the reviewed notes, then let the maintainer
authorize publication. Never move a published tag or substitute historical CI
artifacts. If pre-tag qualification remains incomplete, keep the source checkpoint
pinned and the release blocked.
