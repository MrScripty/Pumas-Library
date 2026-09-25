# Execution Ledger: Cross-Platform Torch Runtime Management

## 2026-09-24 — Support contract and design

- Rechecked branch `work/torch-version-management` at `9fb367b2`; preserved the
  pre-existing untracked `docs/breif/future.md`.
- Read the latest runtime inventory and existing release-management plan before
  targeted source inspection. Current implementation and accepted install/
  lifecycle evidence are Linux x86_64 only; installed CPython 3.10–3.13 is the
  existing managed interpreter contract.
- Confirmed current desktop release targets from
  `scripts/release/artifact-plan.json`: Linux x86_64, Windows x86_64 MSVC, and
  macOS arm64. No Windows ARM or Intel Mac artifact is shipped.
- Read the Coding Standards MCP policies for core, architecture,
  cross-platform support, Rust cross-platform behavior, security filesystem
  containment, contracts, dependencies, planning, and platform verification.
  Standards snapshot: `snapshot:v1:305ccfba-eed5-46b7-bfa2-02868fe804b3`
  (schema v5). Applied native-target evidence, exact artifact identity, native
  path APIs, lifecycle ownership, typed failures, and no guessed compatibility
  defaults.
- Official PyTorch indexes show Torch 2.14.0 CPU wheels for Windows x64 under
  `+cpu`, while macOS arm64 uses the plain distribution version and
  `macosx_14_0_arm64`; the mac wheel tags themselves declare the OS floor for
  that release. The CUDA 13.2 index contains Windows x64 wheels for CPython
  3.10–3.15. No Windows CUDA runtime recommendation is accepted until a
  Windows driver rule is proven.
- The Astra read-only planning review recommended the three shipped target
  triples, native-only interpreter architecture, macOS MPS as a runtime
  capability, exact distribution-version retention, and an owned cross-platform
  process lifecycle. See [target research](reports/target-wheel-research.md).
- The Astra high plan review and independent Sol xhigh plan review passed after
  narrowing the UI preset contract, requiring Windows Job admission before
  child code can run, and specifying observable quarantine/retry ownership for
  locked files.
- Root began M1 after writing the support contract.

## 2026-09-24 — M1 implementation and repair

- Implemented target-aware official wheel filtering for Linux x86_64, Windows
  x64, and macOS arm64, including native CPython discovery, host wheel tags,
  distribution-version/build identity, Windows CUDA as an explicit choice, and
  macOS MPS as a post-install runtime capability.
- Added manager-owned bundled-preset/default-adapter capabilities and updated
  desktop projection to consume them. The v2.9.1 Linux preset is not offered on
  Windows/macOS, and those targets default to core Torch (`none`) rather than
  claiming an image adapter.
- Added native path handling and managed child custody. Windows starts suspended,
  assigns the child to its retained kill-on-close Job before resume, and parks a
  failed-resume child in registered custody. Unix process-group ownership
  remains generation-scoped. Legacy detached Torch launch/stop remains rejected
  on Windows/macOS.
- Made installer/resolver child cleanup per-operation during normal work and
  global only at shutdown. Interrupted publication records durable native
  directory identity so cleanup can retry a partially removed owned directory
  without deleting an unrelated replacement.
- Added Windows/Linux/macOS Torch QA jobs. Cross-target test compilation is an
  early check only; the matrix does not substitute for native end-to-end
  acceptance.
- The Astra high final architecture review reported no findings. The Sol xhigh
  independent review reported no remaining high/medium lifecycle or security
  findings after the target-specific test fixes below. It confirmed that native
  install, sidecar, RPC, and packaged desktop evidence remains unverified.
- The independent review found three target-test assumptions, all repaired:
  an assertion relying on the host being Linux is now explicitly target-patched;
  a Linux PCI sysfs fixture is Linux-gated; and Windows symlink permission error
  1314 no longer prevents unrelated runtime hash assertions.
- Verification on this Linux x86_64 host:
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-app-manager --offline`
    passed (164 tests).
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library --offline`
    passed (library, integration, and doc tests; 6 ignored).
  - `python3 -m unittest discover -s torch-server/tests` passed (89 tests).
  - Ruff check/format, Cargo fmt, `git diff --check`, and the release-gating
    Node test passed.
  - Windows GNU app-manager test-target compilation and Windows GNU
    `pumas-library --no-default-features --tests` checking passed. These are
    cross-target compile checks; Windows tests were not executed. No macOS
    target compiler or runner was available.
- M1 implementation is complete in the candidate branch. M2 acceptance remains
  pending: the native QA matrix has not run, and exact CPU-wheel install/identity,
  RPC health/generation-owned stop, and packaged desktop smoke have not been
  exercised on Windows x64 or macOS arm64.

## 2026-09-25 — Managed CPython and install-preview repair

- Implemented the authorized Pumas-managed CPython provider for Linux x86_64,
  Windows x86_64 MSVC, and macOS arm64. It bootstraps pinned uv without host
  Python, provisions a private stable CPython 3.10+ runtime, and tries the
  newest candidates first when an exact official Torch wheel and the complete
  selected dependency profile are available. It has no user Python selector or
  upper minor allowlist. Clean-host download, license, and native-platform
  acceptance remain pending.
- Traced the desktop error `Unknown API method: get_torch_release_options` to
  Electron's method allowlist, which rejects the request before Rust dispatch.
  Commit `6a726eac` registers the method and adds the payload regression test.
  The toolbar-linked package still needs a published update; this work creates
  a local candidate only.
- Core Torch (`none`) is now the default on all targets. On Linux, FLUX.2 and
  the qualified v2.9.1 bundled recipe require explicit user selection. The
  preview explains that Pumas provisions its own Python runtime.
- Linux verification: app-manager tests (178), app-manager Clippy, Electron
  tests (12 files), Torch preview tests (14), frontend typecheck/lint, resolver
  tests (98), Ruff, Rust formatting, and `git diff --check` pass. The exact
  v2.14.0 remote artifact scan/install could not run because artifact hosts are
  unreachable in this session; Windows/macOS native acceptance remains open.
- Built local `v0.7.0` release artifacts:
  `electron/release/Pumas.Library-0.7.0.AppImage` and
  `electron/release/pumas-library-electron_0.7.0_amd64.deb`. The release builder
  completed without publishing. The artifact checker verified both; the
  headless release smoke and package smoke each reported RPC health passed for
  the standalone backend, AppImage, and deb. Inspection confirmed the packaged
  Electron bundle contains `get_torch_release_options` and the managed-Python
  UI copy.
- Next: use the candidate on a host with PyTorch/uv artifact access to run exact
  v2.14.0 preview/install and sidecar lifecycle acceptance. Complete clean-host
  provider/download/license and native Windows/macOS package acceptance before
  claiming those platforms are supported.
