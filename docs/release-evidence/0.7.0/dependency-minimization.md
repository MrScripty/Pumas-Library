# Dependency minimization and macOS follow-up

This follow-up supersedes the dependency graph and Linux artifact inputs in
[the earlier installer report](follow-up.md). Earlier hashes remain historical
receipts; those installers do not include these dependency reductions.

## Resolved findings

- Removed unused `lnk` and its parser/macro dependencies on all targets.
- Removed the unused core `zip` declaration; app-manager owns ZIP extraction.
- Disabled ZIP defaults and selected existing decoders explicitly. Removed AES
  encryption, ZIP time conversions and the Zopfli compression encoder. Stored
  and DEFLATE runtime extraction have a payload round-trip regression test.
  Other existing decoders remain intentional compatibility support for upstream
  archives; this release does not narrow accepted runtime download formats.
- Limited `sysinfo` to system/process, disk and multithreaded collection. Removed
  unused component, network and user enumeration features.
- Removed JSON formatting from `tracing-subscriber` and unused form extraction
  and tower logging from Axum. Kept the actual JSON/query/original-URI routing,
  tracing and HTTP server features.
- Removed redundant `@types/loglevel` and unused direct frontend `glob` and
  `brace-expansion`. Existing transitive security overrides remain in place.
- Made ONNX execution, `ort`, tokenizers and `half` optional through the core
  `onnx-runtime` feature. Default core and RPC inference builds retain execution;
  no-default core/RPC builds omit it. Model metadata remains available.

Reqwest retains TLS, system proxy discovery, HTTP/2, text decoding, JSON and
streaming for existing upstream/runtime clients. Those are compatibility and
network behavior, not unused extras. Tokenizer `onig` and the selected ONNX
API/native build features remain for the verified Nomic inference path.

The lockfile removes 29 Rust packages without adding or upgrading any. Cargo
feature contracts are checked for default RPC and separate no-default core/RPC
closures across Linux x64, macOS ARM64 and Windows x64; see
[the feature checks](dependency-features-minimized.txt). Those graph checks prove
resolution, not native compilation or execution on the other platforms.

The regenerated attribution has 367 entries (previously 401). Freshness checks
now include Cargo manifests, so feature-only changes invalidate attribution.
The [audit](cargo-audit-minimized.json) against the advisory database refreshed during this release review
reports zero vulnerabilities/yanks and three unmaintained notices (`instant`,
`paste`, `bincode`); removing `lnk` also removed `proc-macro-error2`.

## macOS acceptance

CI now installs the pinned Python and runs `smoke-macos-package.py` on the DMG.
It mounts read-only, copies the app, unmounts, verifies version/resource hashes
and ARM64 executables, and starts packaged RPC and Electron from the copied app.
Smoke processes use a temporary registry and library. A failed desktop startup
has process-group cleanup. The claim is package loading/startup, not interactive
UI, child-shutdown correctness, signing/notarization or Gatekeeper acceptance.

The helper and workflow receive local static checks. Native macOS execution,
interactive recovery and the Apple SDK attribution caveat remain explicit
platform acceptance requirements. macOS remains required; Windows remains best
effort. No remote workflow is dispatched or artifact published by this work.

## Verification

Both no-default suites, all-feature workspace Clippy and the default workspace
suite passed. The optimized RPC build/tests and standalone opt-in ONNX check
also passed; [Rust QA summary](qa-minimized.txt) records the commands and results.
Existing ignored live-network/host fixtures remain unchanged. Frontend types/lint/build and 678 tests, Electron lint and 11 tests,
launcher process tests, and 22 Torch source fixtures also passed.

The full core suite exposed a five-second reconciliation fixture timeout under
parallel load. Its isolated run passed. The fixture now sleeps between polls
and uses a bounded 30-second budget for real filesystem/database work; it does
not assert a latency SLA. The full no-default and default suites passed after
that fixture change. Production reconciliation code is unchanged.

The rebuilt Linux candidates are in `/tmp/pumas-070-minimized-linux/`; exact
hashes and limitations are in [the candidate receipt](minimized-linux-candidate.json).
The earlier Linux artifacts must not be published as evidence for this graph.

| Rebuilt artifact claim | Evidence |
| --- | --- |
| Exact resources, embedded attribution and RPC health in both formats | [Resource checks](minimized-installer-resources.txt) |
| Real ONNX import/load/finite 256-dimensional embeddings/unload | [AppImage](minimized-appimage-onnx.txt), [Debian](minimized-debian-onnx.txt) |
| Desktop readiness, category menu, close and backend termination with the sandbox enabled | [AppImage](minimized-appimage-desktop.txt), [Debian](minimized-debian-desktop.txt) |
| Actual isolated Debian installation, maintainer scripts, alternatives, Electron identity and removal | [Install/remove](minimized-debian-install.txt) |

Both SPDX inventories passed structural schema validation (format annotations
were disabled), and checksums cover both installers, both inventories, notices
and unsigned local provenance. Native component-level SBOM limits remain as
recorded in the attribution report. Desktop checks disabled GPU rendering;
chooser recovery evidence remains from the earlier candidate, with no chooser
code changed here. Live host AppArmor policy loading is outside the isolated
Debian installation claim. Electron-builder retains its existing desktop-name
association warning; this is not a dependency or installer-startup failure.
