# Execution Ledger: Cross-Platform Torch Runtime Management

## 2026-09-26 — Packaged Linux AppImage progress UX acceptance

- Rebuilt the local Linux v0.7.0 AppImage and deb after fixing the Torch
  install-progress presentation. The artifact/resource checks passed, and both
  extracted backend packages passed `/health`.
- Through the rebuilt AppImage at 800×1000, removed the previous test install,
  reviewed the v2.14.0 cu132/Core preset with automatic Python selection, and
  installed 44 exact hashed artifacts. At the first sampled install state
  (9 seconds elapsed), the header named the Torch install and current phase; the
  dialog showed `Working…`, indeterminate setup bars, and accessible Cancel.
  Neither bar exposed `aria-valuenow`, and the old false 95% was absent. The row
  returned to Ready after completion. Focused source tests verify the pending
  state before the first progress poll.
- Managed CPython 3.14.7 reported Torch `2.14.0+cu132`; a CPU tensor sum
  returned 5. The host pip cache matched its pre-test snapshot and resolver
  scratch was cleaned. The package cache was warm, so this run does not claim
  cold-cache transfer progress. CUDA/device and Tuldok generation remain
  untested.
- See
  `reports/v2.14.0-linux-appimage-cu132-ui-acceptance/README.md`. AppImage
  SHA-256 is
  `a28f302822ce99a9d687797606574c93a5afb9c184f293526b16b972294a670b`; deb
  SHA-256 is
  `d8effc33ba37466171c5fae0178e2555064850e40544c93249966fedbf4bce88`.
  These are local candidates and do not update the public toolbar-linked
  package; packaged Windows/macOS acceptance remains open.

## 2026-09-26 — Current-source native Torch acceptance

- Manual workflow
  [36229508586](https://github.com/MrScripty/Pumas-Library/actions/runs/36229508586)
  passed on runtime commit `21041697`: workflow/release contracts,
  frontend/desktop contracts, headless checks, Rust quality, and all three
  native Torch QA and RPC E2E jobs.
- Linux x86_64, Windows x86_64 MSVC, and macOS arm64 each provisioned
  Pumas-managed CPython 3.14.7 with pinned uv 0.12.18; discovered and installed
  v2.14.0 CPU/Core using 25 hashed official artifacts; persisted the interpreter
  across a second backend; returned 14 from a fresh CPU tensor operation; passed
  the resolver probe and protocol 3 sidecar trial/stop; shut down gracefully;
  and reported safe cleanup.
- Full native evidence is retained in the per-target
  `v2.14.0-*-current-source-cpu-rpc-restart-acceptance/` report directories,
  including acceptance, runtime, wheel/dependency resolution, probe, and backend
  session records. This verifies the current metadata-lock follow-up.
- PR head `57e5fb7f` adds the acceptance documentation and evidence after the
  runtime-source commit. Electron UI-driven install, packaged Windows/macOS
  install, provider cancellation/tamper/retry, CUDA/MPS, and v2.14.0 Tuldok
  generation remain unverified.

## 2026-09-26 — Torch install through the packaged Linux backend

- Built the current branch's Linux AppImage/deb with the latest generated
  CPython notice inventory and updated Electron RPC bridge. The exact resources
  were checked in both installers, and each bundled backend passed `/health`.
- Ran `torch-managed-python-acceptance.py` against the packaged RPC backend
  with its inherited `PATH` cleared. The acceptance fetched the upstream
  `v2.14.0` release, selected the CPU profile, provisioned managed CPython
  3.14.7 through uv 0.12.18, and installed 25 SHA-256-verified wheel/dependency
  artifacts. A CPU tensor operation returned 14 after a fresh backend restart;
  the retained resolver probe, protocol 3 trial/stop, graceful shutdown, and
  disposable-root cleanup all passed.
- The exact result is retained in
  `reports/v2.14.0-linux-packaged-cpu-acceptance/acceptance.json`. The backend
  SHA-256 is
  `1b42b6cbedc0bf842990a895cf759537f60ba1564e054a8c8b5f7f022e4a2114`.
  AppImage SHA-256 is
  `1398ef0a9da1c0aab90681d3c91674ef88c6229a84984047938e7bd6eb350acd`; deb
  SHA-256 is
  `468b6f7c2af00ff8785f80e5486cd5133979e875dd351b4b9f5284ddfd195043`.
- This verifies the packaged Linux RPC backend, not Electron UI-driven
  installation. Packaged Windows/macOS installation, CUDA/MPS, and v2.14.0
  Tuldok generation remain unverified; the local artifacts do not update the
  public toolbar-linked release.

## 2026-09-26 — Attribution repair and current Linux package candidate

- Generated CPython full-archive notice supersets for Linux x86_64 GNU,
  Windows x86_64 MSVC, and macOS arm64. The 0.7.0 inventory contains 371
  package entries, 78 hashed inputs, and all 57 captured CPython legal texts;
  Windows CRLF bytes are preserved.
- Generation and packaging checks now bind the Rust uv version and each
  `NativeTarget::pin()` enum arm to its target triple and SHA-256. They also
  require the reviewed PBS release, selected/full archive URL, filename and
  flavor to match `PYTHON.json`, and require each legal text exactly once.
  Missing or malformed full-archive digests fail closed. Tests cover a
  rehashed swapped uv pin and missing, uppercase, short, and non-hex archive
  hashes. The Electron Builder hook is also tested with its context argument.
- Attribution checker and all release-script tests pass; Ruff passes. The
  focused Electron packaging-hook and RPC allowlist tests pass.
- Built current-branch Linux v0.7.0 AppImage and deb locally with the updated
  notices and bridge. Artifact checks, extracted resource hashes, and packaged
  backend `/health` smoke pass. AppImage SHA-256 is
  `1398ef0a9da1c0aab90681d3c91674ef88c6229a84984047938e7bd6eb350acd`; deb
  SHA-256 is
  `468b6f7c2af00ff8785f80e5486cd5133979e875dd351b4b9f5284ddfd195043`.
- This does not verify packaged Torch installation or v2.14.0 Tuldok image
  generation. The public toolbar-linked v0.7.0 assets remain unchanged; these
  files are local candidates only.

## 2026-09-25 — Exact-wheel and native preflight follow-up

- At `032045ad`, the manual native RPC run
  [36222136437](https://github.com/MrScripty/Pumas-Library/actions/runs/36222136437)
  passed the Linux v2.14.0 CPU/Core install and second-session restart with
  managed CPython 3.14.7. The resolver used the discovered official Torch wheel
  identity and completed the CPU operation, retained probe, sidecar trial/stop,
  and graceful shutdown.
- Windows and macOS E2E stopped at version preflight before release discovery.
  Their backend logs show GitHub 403 rate limits with `Retry-After` values of
  1411 seconds (Windows) and 1448 seconds (macOS); the acceptance harness
  rejected both under its separate 900-second cap. These results do not test
  Windows/macOS wheel resolution or installation. This Linux pass supersedes
  the earlier incomplete Linux release-options scan.
- Removed the separate retry-delay cap. The harness accepts only exact
  nonnegative integer delays that fit the 1800-second total budget, keeps the
  three-attempt bound, and fails before sleeping if the delay would consume the
  budget. The 1411-second sleep/success case, malformed values, over-budget
  values, and final-attempt budget classification are covered.
- All 53 acceptance-script fixtures, Ruff lint/format, and `git diff --check`
  pass locally. Astra high reviewed the budget semantics; Sol xhigh independently
  reviewed retry edge cases and verified no sleep occurs for over-budget values.
- PR run
  [36222118583](https://github.com/MrScripty/Pumas-Library/actions/runs/36222118583)
  passed workflow/release, frontend/desktop, Linux/Windows/macOS native QA, Rust
  quality, and headless checks. Native RPC E2E is skipped on pull requests, so
  that run is not platform install evidence.
- Commit `07f7e9f6` was pushed to the existing PR branch. The updated PR run
  [36223095054](https://github.com/MrScripty/Pumas-Library/actions/runs/36223095054)
  passed all required checks. Manual native run
  [36223106097](https://github.com/MrScripty/Pumas-Library/actions/runs/36223106097)
  passed v2.14.0 CPU/Core RPC install and restart on Linux x86_64, Windows x64,
  and macOS arm64, with CPython 3.14.7 provisioned on each target. Each run
  retained 25 hashed profile artifacts, passed a fresh CPU tensor operation
  (`14`), revalidated probes, passed protocol 3 sidecar trial/stop, and shut
  down gracefully. Linux and Windows release preflight passed on the first
  attempt. macOS honored a 913-second `Retry-After`, then found the release on
  attempt two in 916.889 seconds total.
- The retained platform-specific acceptance results, runtime manifests,
  resolver reports, wheel/dependency hashes, and backend logs are linked from
  the plan's [Linux](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md),
  [Windows](reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md), and
  [macOS](reports/v2.14.0-macos-cpu-rpc-restart-acceptance/README.md) reports.
- This closes the current-revision CPU/Core RPC install/restart gate on the
  three shipped targets. Provider notice integration, packaged desktop install,
  broader cancellation/tamper cases, CUDA/MPS execution, and v2.14.0
  image/Tuldok generation remain open.

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
  tests (12 files), Torch preview tests (14), frontend typecheck/lint, full
  Python suite (98), Ruff, Rust formatting, and `git diff --check` pass. The exact
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

## 2026-09-25 — Live Torch 2.14 provider and index repair

- Corrected the uv pin to published `0.12.18` after confirming `0.12.19` was
  not an official release. The Linux archive matched its official SHA-256, and
  bootstrap/cache paths now include the exact provider version and hash so an
  old binary cannot silently satisfy a new pin.
- Corrected the provider source allowlist for uv's actual
  `releases.astral.sh/github/python-build-standalone/...` URLs and corrected
  exact-wheel validation to accept current official PyTorch direct-file URLs at
  `/whl/{channel}/{filename}` while retaining strict host/channel/tag/path
  validation. Added provider and official CPU/CUDA URL regression fixtures.
- Live Pumas RPC acceptance in an isolated Linux launcher root passed the
  Torch 2.14.0 release scan and full Core preview. Pinned uv provisioned
  CPython 3.14.7; the manager recommended `cu132`, and the retained resolver
  result contained 44 artifacts for `cu132` / `python3.14` / adapter `none`.
  No Torch wheels were installed; installed identity and sidecar lifecycle
  remain unverified for v2.14.0.
- Rebuilt the local AppImage and deb from commit `ce9170d4`. Both pass the
  artifact checker and bundled RPC health smoke; the standalone release startup
  smoke also passes. Inspection confirms the packed Electron bundle includes
  `get_torch_release_options` and the bundled backend includes uv `0.12.18`.
  These are local artifacts only; Windows/macOS native provider and runtime
  acceptance, provider license inventory, and toolbar-linked package
  publication remain outstanding.

## 2026-09-25 — Managed-Python Torch 2.14.0 install acceptance

- In a disposable Linux x86_64 launcher root, `preview_torch_runtime` resolved
  the CPU/Core `v2.14.0` tuple as `python3.14` with 25 exact artifacts.
- `install_version` consumed that retained preview, installed the official
  Torch wheel and dependencies, checked the installed identity and CPU
  operation, and completed successfully. `get_installed_versions` returned
  `v2.14.0`, and `switch_version` explicitly selected it.
- The same preview/install/identity/selection flow passed with Python, pip, and
  PyPy absent from the backend's child `PATH`; ordinary OS tools remained
  available. The temporary launcher root was deleted after the RPC process
  stopped.
- This validates the Linux CPU/Core install path without a host Python. It does
  not establish CUDA/device execution, Torch sidecar startup/health/owned stop,
  image-adapter or Tuldok behavior, or Windows/macOS acceptance.

## 2026-09-25 — Torch 2.14.0 managed CPU/Core RPC lifecycle

- Repeated the native Linux x86_64 run through an isolated `pumas-rpc` process
  and disposable launcher root. `get_torch_release_options` found the release;
  the retained CPU/Core preview resolved 25 exact artifacts for managed
  CPython 3.14.7. Pinned uv 0.12.18 was verified against its published Linux
  archive SHA-256 (`89eadd7c76fc063887959510d5ba0ab1264dfd5f1143b925ddb73021a40acf16`).
- The backend child had an empty `PATH` and no host Python, pip, uv, venv, or
  conda selectors. The acceptance verified runtime.json provenance, the private
  managed-Python depot, executable SHA-256, and the installed venv's direct
  `sys._base_executable` and CPython version before accepting installation.
- Installation passed exact Torch 2.14.0 identity, CPU tensor, and sidecar
  dependency probes. The runtime was explicitly selected; a managed CPU
  TorchServe profile passed startup, health, and protocol 3; generation `1` was
  stopped by its owned generation; and RPC shutdown and process exit succeeded.
- Retained `runtime.json`, exact `resolution.json`, `pip-resolution.json`,
  `probe-results.json`, `acceptance.json`, and `rpc.log` in
  [the v2.14.0 evidence directory](reports/v2.14.0-linux-cpu-rpc-acceptance/).
  The temporary runtime root was deleted after those files were collected.
- Added an opt-in `torch-native-e2e` CI matrix for Ubuntu 24.04, Windows 2025,
  and macOS 15, gated to manual dispatch or version-tag builds. It builds native
  RPC, runs the complete install/lifecycle acceptance, and uploads per-platform
  reports even on failure. The always-on Torch QA job runs the script fixtures.
  This Linux run does not substitute for the pending Windows/macOS executions.
- The Sol xhigh read-only repair review found no remaining material findings in
  Windows canonical containment, venv provenance, shutdown, evidence collection,
  or native command paths. CUDA/device, provider licensing, packaged desktop,
  and v2.14.0 Tuldok/image acceptance remain open.

## 2026-09-25 — Local Linux desktop release build

- Built the frontend, optimized `pumas-rpc`, and local Electron v0.7.0 Linux
  packages from commit `2af87420`. Outputs are
  `electron/release/Pumas.Library-0.7.0.AppImage` (SHA-256
  `e68be181a1d0ce5be44c3b44163300155614195040d777ef920d1a19bff1c2a4`) and
  `electron/release/pumas-library-electron_0.7.0_amd64.deb` (SHA-256
  `2b991659431a2da599328449cb8ca608fde379350a815257110eca61404140c2`).
- The artifact checker accepted both installers. The package smoke extracted
  each one, verified its RPC/frontend resources against the build inputs, and
  passed bundled RPC `/health` startup for both AppImage and deb.
- This is a local candidate only. The package smoke does not install Torch
  through the packaged UI; Windows/macOS builds and native runs, toolbar-link
  publication, and Torch device/image generation acceptance remain pending.

## 2026-09-25 — Automatic Python fallback and native provider test coverage

- Extracted automatic Python candidate-attempt state into a small helper used
  by the production preview path. A platform-neutral regression test verifies
  newest-first selection, success on the newest compatible candidate, fallback
  only after a definite `Unsupported` result, stop-on-inconclusive/provisioning
  failure, explicit-version behavior, and the exhausted-candidate result.
- Added Windows and macOS native managed-provider tests for cancellation,
  timeout, descendant cleanup, and closed admission. The native `torch-quality`
  matrix runs the managed-provider tests on each OS. Windows/macOS also verify
  the fallback test exists in Cargo's test list before running it; this guards
  against a test filter silently matching zero tests.
- Linux verification after these changes: all 180
  `pumas-app-manager` library tests passed; Rust formatting and
  `git diff --check` passed. Windows GNU app-manager test-target checking passed
  as a cross-target compile check only. The Sol xhigh review found no material
  lifecycle, path, or CI-filter issue after replacing a Unix-only absolute-path
  fixture with a native temporary path.
- These tests increase native CI coverage but do not count as native Windows
  MSVC or macOS execution. Native Windows/macOS install and lifecycle
  acceptance, provider license materials, packaged desktop Torch installation,
  and the toolbar-linked release update remain open; X1–X7 therefore remain
  pending.

## 2026-09-25 — Managed Python license collection and archive bound repair

- Added pinned uv 0.12.18 MIT and Apache-2.0 texts to the generated 0.7.0
  release-attribution inventory. The notice identifies uv as a runtime
  executable downloaded by Pumas, not a bundled application dependency.
- Added the archive-license acceptance collector for the full Python Build
  Standalone asset corresponding to the selected install-only CPython record.
  It verifies the official release asset identity, digest, and size before
  reading `PYTHON.json`; it retains the metadata and all licenses in the
  archive license directory plus declared file references with hashes.
- The retained Linux x86_64 CPython 3.14.7 full archive verified at 126,709,151
  bytes and SHA-256
  `46a9e98d2c7fd2c5b9ca67510b5a0699f5ccad5fb750957961c22c52e07b6cb0`.
  Collection retained 19 license texts and `PYTHON.json` under
  `reports/managed-python-license-collection/linux-x86_64-cpython-3.14.7/`.
  This evidence is not yet merged into release attribution: selected Windows
  and macOS archives remain uncollected.
- Rebuilt the retained manifest from the captured official release API record
  rather than synthesizing its asset URL, preserving the upstream `%2B` path
  encoding exactly. Added a fixture requiring percent-encoded build separators
  to survive asset selection and appear unchanged in the manifest.
- Astra high reproduced the PAX gap in the previous cap: `tarfile` consumes
  PAX bytes without exposing them as ordinary member payloads. Added a reader
  that bounds bytes delivered from the decompressed tar stream, including
  headers and PAX metadata. The PAX fixture failed before the repair and now
  passes by rejecting at a 12,000-byte limit while the plain fixture remains
  accepted. The CPython 3.14 zstd fixture exercises the production decompressor.
  Acceptance fixtures pass on Python 3.12 and 3.14; Ruff check/format pass.
- Updated the QA workflow's acceptance-tool runtime to CPython 3.14 because
  `compression.zstd` and the acceptance collector require the standard-library
  zstd reader introduced in Python 3.14. This does not change the packaged
  application runtime.
- Open gates remain native Windows/macOS provisioning and lifecycle acceptance,
  full three-target provider attribution, and packaged desktop install
  acceptance. Local release packaging does not update the public toolbar link.

## 2026-09-25 — Rebuilt local Linux release candidate

- Rebuilt frontend production assets and `pumas-rpc --release`, staged the new
  backend, and packaged v0.7.0 AppImage and deb with the updated uv third-party
  notices. No artifacts were published.
- `scripts/release/check-artifacts.mjs electron/release linux` verified both
  expected installers. `scripts/release/smoke-linux-packages.py electron/release`
  verified their packaged resources against the current frontend, backend,
  license, and third-party notice inputs; both packaged RPC backends passed
  `/health`.
- AppImage SHA-256:
  `cd4cb2c0e168ce207d79693e30fd3a02f8152870c0baea2275ab0a35323b21dd`
  (154,752,684 bytes). Deb SHA-256:
  `0591e1950bbb2761a086c437e51680275dc598c1fc76560091e1557dfef6e6ea`
  (120,406,924 bytes). The public v0.7.0 release and desktop toolbar link remain
  unchanged; packaged Torch installation through the UI is still untested.
- Additional checks passed: 49 launcher tests, 12 Electron tests, 20 managed
  Python acceptance fixtures under Python 3.12 and CPython 3.14, Ruff, release
  attribution check and test, and the final file diff whitespace check.

## 2026-09-25 — Windows and macOS CPython archive candidates

- Used the bounded collector to query the official Python Build Standalone
  release API and download the exact full archives for CPython 3.14.7
  `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`. Both archive sizes and
  SHA-256 values matched the official asset records before extraction. Their
  manifests preserve the API URLs, release/asset IDs, raw `PYTHON.json`, and
  hashes for all 19 license files each.
- Windows archive: 49,262,780 bytes, SHA-256
  `5363ec4aab59c24417f9877217aae95ca17f9ae6eb99c3bbfb25e4a76dcadafe`.
  macOS archive: 60,724,957 bytes, SHA-256
  `185fa676e14b648bd736ce7f20f9b11e201131b3216ec5a39d4ecd8ab8a71112`.
- The Windows license files differ from the Linux/macOS files only in line
  endings and match after newline normalization. These are candidate-version
  captures only; Windows/macOS native installation has not
  confirmed CPython 3.14.7 as the selected interpreter. They are retained for
  review but not yet added to release attribution. See the
  [target archive evidence index](reports/managed-python-license-collection/README.md).

## 2026-09-25 — Linux managed Torch restart acceptance

- Extended the native RPC acceptance to launch a fresh backend process against
  the same launcher root after install, explicit selection, CPU probe, and
  generation-owned sidecar stop. The second process verified the active Torch
  version and managed CPU profile, then revalidated and compared ten CPython
  identity fields across sessions, including provider archive digest, source
  URL, catalog key, canonical executable path, and executable SHA-256.
- After comparing interpreter identity, the restarted acceptance process ran
  a new Torch 2.14.0 CPU tensor operation through the persisted venv interpreter
  (`sum([1, 4, 9]) == 14`). The RPC probe endpoint then revalidated its retained
  install-time CPU probe report and runtime context. The process repeated the
  protocol 3 sidecar trial, stopped the sidecar by its generation, and shut
  down gracefully. Both backend logs and install/resolution reports are
  retained in
  [Linux restart acceptance evidence](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md).
  The selected CPython full-archive hash matches the already-retained Linux
  license evidence.
- On Linux x86_64 with managed CPython 3.14.7 and pinned uv 0.12.18, the full
  two-session run passed; 36 acceptance fixtures, Ruff lint/format, and
  `git diff --check` passed. This adds Linux restart evidence only. Native
  Windows x64 and macOS arm64 installation/lifecycle acceptance remains open.
- A fresh-root retry initially exceeded the acceptance script's 180-second
  HTTP wait while uv was still extracting CPython 3.14.7. Release-option
  discovery provisions Python before starting its 60-second wheel scan, while
  the provider permits up to 300 seconds for interpreter installation. The
  acceptance request budget is now 900 seconds to cover the bounded cold-start
  stages; backend timeouts and product behavior are unchanged. The full
  fresh-root run then passed and replaced the retained report files above.

## 2026-09-25 — Cross-platform acceptance follow-up

- Fixed Windows report handling to decode captured output as UTF-8 and write
  reports with LF line endings. Added the macOS cleanup regression for a
  zombie-only `EPERM` result, where the extinct group may drain and release.
  When `EPERM` is followed by live descendants, process custody and the
  cleanup lease remain in place.
- Targeted tests and reviews passed. The latest local Linux x86_64 two-session
  Torch 2.14.0 CPU acceptance remains passing, including an exact `v2.14.0`
  release-list `tagName` preflight in `get_available_versions`; its refreshed
  [evidence](reports/v2.14.0-linux-cpu-rpc-restart-acceptance/README.md) is
  retained. Native Windows and macOS full acceptance remains pending.

## 2026-09-25 — Native Windows acceptance and Linux CI diagnosis

- Manual workflow run `36214227835` at `a0658131` passed native
  `windows-2025` x64 two-session RPC acceptance for Torch `v2.14.0` CPU/Core.
  Managed CPython 3.14.7 persisted across restart; a fresh CPU operation
  returned 14, and the sidecar trial, generation-owned stop, and graceful
  shutdown passed. The [Windows report](reports/v2.14.0-windows-cpu-rpc-restart-acceptance/README.md)
  retains acceptance, backend, install, probe, and selected archive manifest
  evidence. Windows license integration remains open; the report retains only
  the manifest, without license text collection.
- The Linux E2E job in that run passed exact-tag preflight in 1.625 seconds,
  then release options returned an incomplete scan before preview or install.
  Its diagnostic issues were truncated, so the decisive cause was unavailable.
  Diagnostic commit `e2d52939` now preserves `completeScan` and a bounded
  sample of up to three issues, each capped at 300 characters and with a
  1200-character overall cap, for a rerun. The separate local Linux two-session
  acceptance remains accepted.
  macOS E2E acceptance and remaining all-target gates are still pending.
- In that same [manual run](https://github.com/MrScripty/Pumas-Library/actions/runs/36214227835),
  macOS native QA passed, but macOS E2E did not. Release-list preflight found
  `v2.14.0` after an initial `rate_limited` response reporting a 691-second
  retry; release options returned, then `preview_torch_runtime` failed with
  `validation_failed: The resolved wheel report failed validation.` No macOS
  preview or install completed. The generic rejection does not identify its
  cause.
- Bounded, allowlisted resolver failure diagnostics have been committed on the
  branch and independently reviewed. They have not yet been exercised by a
  manual native RPC E2E rerun; the Linux and macOS E2E failures remain open.

## 2026-09-25 — PR native QA follow-up repairs

- PR workflow run
  [36215890738](https://github.com/MrScripty/Pumas-Library/actions/runs/36215890738)
  for `298688cf` passed workflow/release contracts, frontend/desktop contracts,
  headless checks, and Linux native QA. Rust quality failed on Clippy's
  `manual_repeat_n` lint in the resolver-diagnostic regression, now fixed with
  `std::iter::repeat_n`. macOS native QA failed in the acceptance-script timeout
  cleanup with `EPERM`. Windows native QA failed in the managed-Python
  candidate step at a PID-only descendant-exit assertion after cleanup. The
  run did not execute the native RPC E2E legs because those jobs are skipped on
  pull-request events.
- Repaired the POSIX acceptance harness to keep the exited leader unreaped with
  `waitid(..., WNOWAIT)` until live members of its owned process group drain.
  Zombie-only groups are not signaled. Normal and emergency cleanup signal only
  the pinned group; persistent denial fails loudly and retains the unreaped
  `Popen` custody object. Regression fixtures cover observer failure,
  transient and persistent `EPERM`, and detached descendants; the detached
  fixture now exits naturally and is awaited without signaling its PID.
- Repaired Windows lifecycle test oracles by retaining `SYNCHRONIZE` process
  handles while descendants are alive and checking the exact handles are
  signaled immediately after cleanup. The direct Job-membership test retains
  the handle through `terminate_and_drain`. Added a macOS-only RAII test guard
  that resets injected `EPERM` and drains the standalone custody slot during
  unwinding. These are test changes; runtime lifecycle behavior is unchanged.
- Final local verification passed: 50 acceptance-script fixtures; Ruff lint,
  format, and Python compilation; 185 `pumas-app-manager` tests; workspace
  all-target/all-feature Clippy with warnings denied; Rust formatting; Windows
  GNU test compilation for `pumas-app-manager` and no-default-feature
  `pumas-library`; and `git diff --check`. Astra high and Sol xhigh completed
  read-only repair reviews with no blocking findings.
- The repaired macOS/Windows native QA jobs still need a new PR run. The Linux
  incomplete release-options scan and macOS preview-report validation failure
  remain open pending a manual native RPC E2E rerun with the retained bounded
  resolver diagnostics. Provider license integration, packaged desktop install,
  CUDA/device use, and Torch image/Tuldok generation remain unverified; X1–X7
  remain pending.

## 2026-09-25 — Torch PR metadata and lifecycle review repairs

- All Torch metadata mutations now share the permanent `.torch-versions.lock`,
  including active/default selection, installed-entry add/remove, public
  validation, startup cleanup, and startup selection normalization. Mutations
  load current metadata after acquiring the lock; detached blocking writers keep
  a cloned lease until they finish, including when the async waiter is canceled.
- Active marker and selection metadata writes run in one leased worker. Public
  state snapshots refresh while holding the lock; on contention they return one
  coherent cached generation without blocking. Initialization ignores a
  transitional marker while busy and prefers committed last-selected metadata
  over the default in that fallback. The exact fs2 contention error is
  normalized to `WouldBlock` on Windows while unrelated I/O errors are kept.
- Regressions cover independent-manager selection/validation, stale-manager
  writes after installs/removals, startup during publication/selection, prompt
  busy reads and post-release refresh, coherent version-status metadata, and
  lock-error normalization. The cache/status/startup priority tests were
  observed failing before their fixes and pass now. Astra high and Sol xhigh
  completed read-only review with no remaining blocking findings. A marker
  rollback can still fail after an underlying metadata I/O error; that rare
  error path remains best-effort and is reported as a limitation.
- Final local verification: `cargo test --all-targets --locked --quiet` passed
  for workspace default members (the app-manager run in that pass preceded the
  final startup-priority regression; the final focused app-manager rerun passed
  199 tests). `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo fmt --all -- --check`, and Windows GNU app-manager test compilation
  passed. Frontend passed 713 tests, typecheck and lint; desktop-contract
  conformance passed 48 tests; Electron passed 179 tests with one
  platform-dependent skip; managed-Python acceptance passed 50 fixtures with
  Ruff and Python compilation; release-attribution validation and
  `git diff --check` passed.
- These are local branch checks. A new PR workflow run is still required. Native
  Windows/macOS E2E, provider-license integration, packaged desktop Torch
  installation, CUDA/device execution, and v2.14.0 Tuldok/image generation are
  still open; X1–X7 remain pending.
