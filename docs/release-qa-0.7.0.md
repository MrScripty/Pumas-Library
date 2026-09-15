# 0.7.0 local release preparation

Current dependency-minimization results and macOS follow-up are tracked in
[the minimization report](release-evidence/0.7.0/dependency-minimization.md).

See [the dependency and installer follow-up](release-evidence/0.7.0/follow-up.md)
for the earlier der 0.8.2, attribution, metadata and installer evidence.
The original preparation record below is retained as the earlier baseline.

Prepared on 2026-09-14 against `af45f3e0` plus the local release changes.
Preparation and verification were local. No push, tag, remote workflow dispatch, or publication was performed.
This is a local candidate, not approval to publish all advertised platforms.

## Changes and historical findings

The release histories through v0.6 repeatedly repaired binding generation,
pnpm/native dependencies, action pins, platform paths, symlinks, fixture isolation,
lint, and RAM-disk setup after attempts to release. The most recent remote main
run (`34836933217`) also failed formatting, Windows compilation, and Linux/macOS
Rust tests.

The replacement workflow rehearses the same candidate pipeline on PRs, main,
manual dispatch, and tags. It uses repository toolchain pins, locked dependency
resolution, independent headless checks, native release tests, desktop contracts,
Python QA, exact installer inventory, and checksums. Linux installers are extracted
and their backend/renderer/license/native resources checked against build inputs;
the extracted RPC servers must start successfully. Missing artifacts fail the job.
Publication is separate and repository permissions are read-only.

Versions are aligned at 0.7.0. Electron, packaging, frontend tooling and vulnerable
Rust dependencies were updated. Staging excludes stale Rustler/UniFFI libraries.
Test fixtures now supply real mutation authority, use canonical paths, exercise
current conversion setup probes, start an owned runtime for shutdown verification,
and respect Torch's prohibition on in-place dependency repair. The initial
preparation removed nightly-only Windows metadata calls. The subsequent Windows
implementation adds native download authority and durable JSON publication.

The [release notes](release-notes-0.7.0.md) remove repeated feature descriptions,
correct the ownership transition's chronology (already present in v0.6), and avoid
claiming unsupported binding distributions or universally qualified GPU runtimes.

## Local evidence

Toolchain: Linux x86_64, Rust 1.92.0, Node 24.15.0, pnpm 10.33.0.
Commands and acceptance requirements are in [RELEASING.md](../RELEASING.md).

| Check | Result |
| --- | --- |
| Workflow syntax, dependency ownership, version/tag alignment | Passed; mismatched tag rejected |
| Installer inventory regression checks | Passed |
| Frontend lint, types, tests, default/library-only builds | Passed; 117 files, 678 tests |
| Electron lint, types, tests | Passed; 172 tests |
| Desktop contract generation/conformance | Passed, including 48 frontend contract tests |
| Launcher tests | Passed; 49 tests |
| Torch Python lint/format and unit tests | Passed; 22 tests |
| Release Python scripts lint/format | Passed |
| Rust formatting, all-feature check/Clippy, default workspace tests/docs | Passed; core 1,408 tests, API 36, RPC 227; other integration/workspace suites also passed |
| Rust no-default workspace check and independent headless suites | Passed |
| Optimized workspace tests | Passed, including RPC and doctests |
| Inference-disabled optimized RPC build and route smoke | Passed; health 200, inference routes 404 |
| Canonical-path recovery regression through a symlinked temp root | Passed |
| Default optimized backend and launcher build | Passed |
| Real ONNX load/embed/unload and gateway embedding fixture | Passed; source integration tests using the local Nomic fixture |
| Linux AppImage and Debian package build/extraction | Passed |
| Extracted installer backend health and resource hashes | Passed for both formats |
| Extracted AppImage and Debian desktop startup/shutdown | Passed with an isolated library and CI-style sandbox flags |

Existing ignored tests were not broadly enabled, including live Hugging Face/CDN
acquisition tests and environment-dependent examples. The explicitly supplied real
ONNX fixture is additional evidence; standard suite success does not qualify all
external services or devices.

The desktop smokes log an in-flight RPC socket closure during shutdown, then
completes cleanup and exits successfully. This bounded smoke does not establish
all desktop lifecycle/user flows. Real ONNX fixture tests are distinct from the
packaged backend health checks; packaged model inference remains an acceptance
obligation.

Local installers and SHA-256 checksums are in `/tmp/pumas-070-candidate-linux-electron43/`.
The inference-disabled diagnostic binary is in `/tmp/pumas-070-headless/`; the
normal inference-enabled launcher backend was restored from verified staging.
Detailed transient logs are `/tmp/pumas-070-*.log`; they are not committed evidence
or durable release assets. The installed system was not changed to test the Debian
package: extraction and execution of its bundled backend were verified.

## Remaining release and platform blockers

- **Native candidate validation:** Windows now implements held filesystem identity,
  no-follow directory traversal, execution locking, native rename, and directory
  synchronization. Windows is a required candidate gate alongside Linux and macOS.
  The [native Windows verification](https://github.com/MrScripty/Pumas-Library/actions/runs/35028010502)
  passed filesystem/recovery checks, API/RPC integration tests, and backend startup;
  its three remaining failures were Unix directory opens in test fixtures, now
  switched to the native helper. The final candidate must also pass release tests
  and installed/portable application startup checks. Earlier
  [Linux/macOS CI](https://github.com/MrScripty/Pumas-Library/actions/runs/35011477450)
  passed their release and installer checks; those gates remain required after
  the Windows changes.
- **Dependency review:** the [review](dependency-review-0.7.0.md) found the
  unsupported Electron 39 line and reproduced its legacy extractor's overwrite
  flaw. The follow-up upgrade to Electron 43.7.0 removes that dependency; fresh
  pnpm audit has no actual installed-version advisories (workspace-importer false
  positives remain). All 172 Electron tests, including the native preload oracle,
  plus launcher and both extracted Linux package checks pass. Fresh Cargo audit
  reports zero vulnerabilities; the `der` update and maintenance follow-up remain.
- **Publication acceptance:** complete per-installer third-party notices, SPDX
  SBOMs, provenance, target-native installation/lifecycle flows, and packaged
  inference evidence remain required by the artifact plan. The project license is
  now bundled, but it is not a substitute for complete third-party notices.
- **Excluded surfaces:** no host-binding tuple is accepted by the binding support
  matrix; Rustler requires an Erlang host and is excluded from ordinary workspace
  QA. No Torch runtime distribution or real GPU/device tuple was qualified here.

Do not tag or publish until these obligations are satisfied or the release's
claimed target/artifact scope is explicitly revised and reverified.

The best-effort Windows workflow change and required-only installer assembly were
validated with Actionlint and inventory regression tests locally. No Windows
runner was executed. Dependency triage is recorded in
[dependency-review-0.7.0.md](dependency-review-0.7.0.md).

## Electron 43.7.0 follow-up

The current desktop candidates use Electron 43.7.0. CI explicitly installs its
lazily downloaded runtime before native tests/smoke; test discovery now resolves
the executable only when the optional native preload oracle actually runs. An
initial cold downloader stalled before that separation and was interrupted; the
subsequent explicit install, all 172 tests (zero skipped with the native oracle
enabled), 41 contract-conformance tests, 49 launcher tests, and desktop startup/
shutdown passed. Both extracted installer executables report Electron 43.7.0.
See [upgrade evidence](release-evidence/0.7.0/electron-upgrade.json) for current
installer and lockfile hashes. Earlier inventory/installer evidence is retained
as the Electron 39 baseline. Rust sources and the staged RPC binary are unchanged
by this Electron upgrade; their previous verification remains applicable.
