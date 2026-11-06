# Development

## Setup

Use the versions pinned by `rust-toolchain.toml`, `.node-version`,
`.python-version`, and the root `package.json`.

```bash
corepack enable
corepack pnpm install --frozen-lockfile
./launcher.sh --build
./launcher.sh --run
```

On Windows, use the same actions through `launcher.ps1`.

Both wrappers require Node and delegate to `scripts/launcher/cli.mjs`; neither
wrapper owns release-specific parsing or environment policy. The shared
launcher accepts `linux`, `darwin`, and `win32` and returns exit code 5 for an
unsupported operating system.

## Standards

The standards source is:

```text
/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/
```

Read `CORE-STANDARDS.md`, then use `STANDARDS-ROUTER.md` to select the canonical
workflow, topic, language, application, and boundary owners for the change.
Repository docs explain Pumas-specific facts; they do not replace the standards.

File length, directory count, and the existence of a `src/` directory are
diagnostic facts, not design or documentation requirements.

## Common Commands

### Rust

Cargo builds never download ONNX Runtime. Default core/RPC builds keep the
optional ONNX execution APIs and load a separately provisioned native library
only when execution is requested. Missing or invalid runtime files return a
`runtime_library` backend error; metadata/library-only use needs no native SDK.

Provision and verify the native distribution before running ONNX: the current
qualification pin is Microsoft's ONNX Runtime 1.24.2, with C API 24 and the
correct target/CPU variant. Record the official release identity and trusted
archive SHA-256, verify before extraction, and retain the selected library hash
and upstream notices. The existing acquisition owner handles any explicitly
authorized setup transfer; neither Cargo/build scripts nor app settings fetch it.
There is no automatic source fallback or new version-manager installation flow.

Set `ORT_DYLIB_PATH` to the absolute `.so`/`.dylib`/`.dll` file before process
startup, or package the platform's named library beside the executable. For
example, with an already verified Linux SDK:

```bash
ORT_DYLIB_PATH=/absolute/onnxruntime-1.24.2/lib/libonnxruntime.so \
  cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library
```

If its dependent libraries require an OS loader path, supply that only to the
invocation; do not change system loader configuration. `ORT_LIB_PATH` and
`ORT_PREFER_DYNAMIC_LINK` belonged to build-time linking and are not runtime
selectors in this mode. The binding retains its process-wide loaded library;
runtime replacement requires restarting the process: a load whose verified
library differs from the one ORT already adopted (or whose file changed after
verification) is refused with that reason. Applications embedding
Pumas keep ownership of ORT environment settings and must select the same
library before any other direct ORT calls.

Consumers that only need model metadata can declare `pumas-library` with
`default-features = false` to omit ONNX Runtime; add `s3` only when they call
the S3 reader. The [Rust guide](../rust/README.md#features-and-variants) owns
what each feature actually gates. Cargo features unify
additively, so audit the entire consumer graph: another dependency enabling ORT
defaults can reintroduce its optional download capability and is unsupported.
`scripts/release/check-dependency-features.py` guards the workspace's default,
all-feature, binding and explicit ONNX graphs on all declared desktop targets.

```bash
./scripts/rust/check.sh
cargo test --manifest-path rust/Cargo.toml -p pumas-library <test-filter>
```

The aggregate script runs formatting, all-target/all-feature Clippy (which also
compiles every target), workspace tests and docs, and no-default compilation. It
excludes `pumas_rustler`, which needs an Erlang/OTP host.

Integration tests are modules of a few test executables rather than one
executable per file, because each executable links the whole library:

- `pumas-library`: `tests/integration/` (add a file and declare it in
  `main.rs`), `tests/registry_override/` for tests that set
  `PUMAS_REGISTRY_DB_PATH` (serialized by its `RegistryOverride`), and the
  focused `tests/physical_store_lifetime.rs` harness.
- `pumas-rpc`: `tests/integration/`.

Filter by module, for example
`cargo test -p pumas-library --test integration intent_ipc_tests::`. A test that
re-runs its executable as a child names itself with
`support::exact_test_name(module_path!(), "...")`.

### Frontend

```bash
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w frontend build
npm run -w frontend build:library-only
```

`pnpm --dir frontend test:desktop-contract` also exercises the model import UI,
built privileged preload, native S3 RPC and owned loopback HTTPS objects. It
builds the locked, inference-disabled S3 RPC offline and uses Cargo's reported
executable; the locked dependencies must already be cached. For an independently
pinned candidate, set `PUMAS_S3_PUBLIC_RPC_BIN` to that exact executable. This
mounted React/jsdom and native-process check establishes import behavior, not a
native browser/Electron window, hosted S3 service, or model inference.

### Electron and Launcher

```bash
npm run -w electron lint
npm run -w electron test
npm run -w electron validate
npm run test:launcher
node scripts/launcher/cli.mjs --help
```

The release smoke owns the process tree it starts. At maximum uptime it requests
graceful POSIX process-group termination, escalates to a forced group kill after
the grace window, and fails if forced cleanup is not observed by the final
deadline. Windows does not expose an equivalent graceful tree operation to this
launcher, so it reports that mechanism unavailable and escalates to bounded,
observed `taskkill.exe /t /f`. These are failure/cleanup semantics, not evidence
that the packaged application works on a target OS.

### Torch Sidecar

```bash
python3 -m ruff check torch-server
python3 -m ruff format --check torch-server
python3 -m unittest discover -s torch-server/tests
```

The unit suite can install local fakes for missing runtime packages. Use a real
resolved Torch environment for ASGI, middleware, loading, or inference claims.

## Frontend Conventions

- Backend responses and persisted browser data remain `unknown` until decoded.
- Hooks own async work, cancellation, current-invocation checks, and cleanup.
- Components render backend-owned state; they do not invent durable success.
- Prefer native semantic controls. When richer interaction is needed, use an
  established accessible pattern with keyboard, focus, naming, dismissal, and
  state behavior verified together.
- The ESLint policy requires React Aria `useHover` rather than raw mouse hover
  handlers.

Theme tokens live in `frontend/src/index.css` as CSS custom properties. Use
semantic tokens rather than hard-coded colors. Do not claim contrast or
reduced-motion compliance without current representative evidence.

## Verification Design

Every permanent gate needs a named claim, proof boundary, adequate oracle,
known overlap, and enough marginal value to justify its maintenance. Examples:

- a typecheck proves static type consistency, not runtime payload validity;
- jsdom component tests do not prove real focus or layout behavior;
- a cross-platform build does not prove runtime behavior on that platform;
- a startup smoke proves startup, not a user workflow; and
- generated bindings do not prove that a host can load and call them.

Choose targeted evidence first, then broaden where the changed boundary or
release risk requires it.

## Permanent Verification Inventory

Repository hooks provide optional early local feedback when a contributor
installs them. They can block that local Git operation, but Git bypass remains
operator authority and the hooks do not establish repository acceptance.

The `Build` workflow runs on pull requests targeting every branch, pushes to
`main`, `v*` tag pushes, and manual dispatches. Source quality and contract jobs
run for every trigger. A *candidate run* is a `v*` tag push or a manual dispatch
with `build_candidates=true` (which also requires `expected_source`): only
candidate runs build optimized releases, packages, package smoke checks and the
release candidate. Pull requests, pushes to `main` and ordinary manual
dispatches instead run the bounded review jobs, so the merged commit gets the
same gates as its pull request. A failing step blocks its workflow and dependants; whether
a workflow is required to merge is external branch-protection state and is not
asserted here.

Setup, cache, download, upload, and artifact-transport steps are prerequisites,
not independent evidence claims. The table lists the semantic gates that are
intended to decide a property.

| Gate and schedule | Claim and deciding oracle | Overlap and marginal value | Blocking authority and environment |
| --- | --- | --- | --- |
| `check-commit-message` at installed `commit-msg` | The first non-comment subject follows the project conventional-commit grammar in `CONTRIBUTING.md`; the repository shell matcher is the executable oracle. | Earlier feedback than review/CI; it does not prove change quality. | Blocks the local commit unless bypassed; representative local Bash/Git environment. |
| `trailing-whitespace` and `end-of-file-fixer` at installed `pre-commit` | Selected text files contain no trailing whitespace and end with the tool's normalized final newline; the post-transform bytes are the oracle. | Covers docs/config formats outside language formatters; mutation is exposed for restaging. | Blocks the local commit until changes are restaged unless bypassed; supported local pre-commit environment. |
| `check-yaml` at installed `pre-commit` | Selected YAML parses as YAML; the pinned hook parser is the oracle. | Actionlint is deeper for GitHub workflows, while this covers other YAML syntax only. | Local/bypassable; supported local pre-commit environment. |
| `check-merge-conflict` at installed `pre-commit` | Selected text has no unresolved merge-marker pattern; the pinned scanner is the oracle. | Git already rejects unresolved index entries; this catches accidentally staged marker text. | Local/bypassable; supported local pre-commit environment. |
| `check-json` at installed `pre-commit` | Selected strict-JSON files outside the declared frontend/`.vscode` exclusions parse; the pinned parser is the oracle. | Package/tool consumers also parse files they read; this supplies earlier staged-file diagnosis for the remaining scope. | Local/bypassable; supported local pre-commit environment. |
| `detect-private-key` at installed `pre-commit` | Selected staged text has no key signature recognized by the pinned detector. | Low-cost defense for known signatures; it does not prove secret absence or safe diagnostics. | Local/bypassable; supported local pre-commit environment. |
| Actionlint in `lint-workflows` | GitHub workflow structure and expressions satisfy Actionlint's model; Actionlint is the independent parser/static oracle. | Deeper than generic YAML parsing for `.github/workflows`; does not execute jobs. | Workflow-blocking on Linux CI for every trigger. |
| `check:dependency-ownership` in `lint-workflows` | Root owns no runtime/dev packages and each workspace declares the tool set the repository policy assigns it; manifests are inputs and the repository checker owns the mapping. | Package installation proves resolution, not declaration ownership. | Workflow-blocking, deterministic Node check on Linux CI; local on demand. |
| `check:release-versions` in `lint-workflows` and release preparation | Root, frontend, Electron, and Rust workspace release versions are identical and version tags match them; their manifest values are the authoritative inputs. | Packaging may expose a mismatch later; this provides direct causal diagnosis before builds. | Workflow-blocking, deterministic Node check on Linux CI; locally required before release. |
| Attribution and release-script tests in `lint-workflows` (`check-attribution.cjs` default and `--features s3`, `node --test scripts/release/*.test.mjs`, `test_s3_build_provenance.py`) | Checked-in notices match the current default and S3 dependency graphs; release scripts satisfy their unit assertions. | Packaging re-checks attribution later; this diagnoses stale notices before any build. | Workflow-blocking on Linux CI for every trigger. |
| `scripts/rust/check.sh` in `rust-quality` | Formatting, all-target/all-feature Clippy (which includes compilation), workspace tests, doctests, and no-default compilation each satisfy their named tool/assertion contract. | Complements release target builds with static, debug, docs, and feature evidence; excludes the separately owned Rustler host claim. | Workflow-blocking on representative Linux CI; local aggregate command. |
| No-default and S3 tests in `headless` | `pumas-library` and `pumas-rpc` pass without inference plugins (also with `hf-client`, and with `s3` for the reader, acquisition, prefix and import-workflow tests); the Linux audio read-confinement proof passes; the inference-disabled release backend builds and `smoke-rpc.py --inference-disabled` sees health without inference routes. | The no-default graph is otherwise compiled only by `check.sh`; this executes it. | Workflow-blocking on Linux CI for every trigger. |
| Frontend ESLint, `check:types` and `test:run` in `build-frontend` | Lint rules, the no-emit TypeScript program and Vitest component/unit assertions pass; each tool is the oracle for only its own rules. | Decides local renderer behavior only; representative browser/Electron workflows remain separately owned. | Workflow-blocking on representative Linux CI for every trigger; local on demand. |
| Desktop contract gates in `build-frontend` (`check:desktop-contract`, `test:desktop-contract-generator`, `test:desktop-contract-conformance`, frontend `test:desktop-contract`) | Checked-in generated decoders equal a fresh export from current Rust source and `electron/contracts/`; constructor-produced fixtures pass the generated validators through the real bundled preload and renderer. | Typecheck proves only static consistency; these prove the shipped decoders match the producer. | Workflow-blocking on representative Linux CI for every trigger. |
| Electron ESLint, `validate` and `test` in `build-frontend` | Electron TypeScript satisfies lint and type rules; Node tests prove the IPC allowlist/request, launcher, and process-boundary assertions. | Packaging jobs re-run `test` on each packaging OS for target-specific value. | Workflow-blocking on representative Linux CI for every trigger; also on each packaging OS in candidate runs. |
| `test:launcher` in `build-frontend` and packaging jobs | Shared CLI parsing, closed platform selection, action delegation, package-manager invocation, wrapper delegation, and controlled max/grace/force/process-tree outcomes satisfy their Node and child-process assertions. | Release smoke traverses a real startup path; Linux execution does not prove Windows or macOS process behavior. | Workflow-blocking on Linux CI for every trigger; also on each packaging OS in candidate runs. |
| Frontend default and library-only builds in `build-frontend` (`test:library-only`, `check-frontend-variant.mjs`) | The library-only Vitest suites pass; Vite emits both renderer bundles; the library-only bundle, built into its own directory, lacks string literals unique to the inference-plugin modules that its mode replaces, and the default bundle has them. The default bundle is the input to every Electron package job. | Does not prove renderer behavior. | Workflow-blocking on Linux CI for every trigger; the `frontend-dist` upload is candidate runs only. |
| `dependency-audit` (`scripts/release/dependency-audit.mjs`) | `cargo-deny` (advisories and licenses, `rust/deny.toml`), `pnpm audit` and `pip-audit` find no unaccepted advisory in the gating scans listed in [Security](SECURITY.md#dependency-scans); results are the `current-vulnerability-results` artifact. | Attribution checks prove notices match the graph, not that it is free of advisories. Results depend on the advisory databases on the day of the scan. | Advisory (`continue-on-error`) on pull requests and pushes to `main`; blocking for version tags and `build_candidates` runs, where `release-candidate` also needs a passing review of the same commit. Linux CI. |
| Torch Ruff, release-contract and unit tests in the `torch` shard of `torch-quality` | `torch-server`, `scripts/release` and `scripts/rust` satisfy Ruff lint and format; release-metadata and headless-owner unit tests pass; sidecar unit assertions pass with their declared fakes. | Does not prove ASGI, middleware, model loading, GPU, or inference in a resolved Torch environment. | Workflow-blocking on native Linux, Windows and macOS for every trigger. |
| Native cargo tests in `torch-quality` (`scripts/rust/native_qualification.py`) | Focused `pumas-library`, `pumas-app-manager` and `pumas-rpc` tests for Torch selection, acquisition and import custody, retained-download reconciliation, registry ownership, process ownership, HTTP drainage and RPC shutdown pass on each host OS. Each filter matches at least one runnable test on that host. | Linux workspace tests overlap; these expose OS path, handle and process behavior on every trigger rather than only in candidate runs. Three shards per OS (`torch`, `default-features`, `inference-disabled`) run in parallel; each builds its cargo configurations once. | Workflow-blocking on native Linux, Windows and macOS for every trigger. |
| `review-inference-linux` | The exact review head builds through the strict inference producer, the extracted archive matches its authenticated build identity, and a deterministic untrained ONNX graph imports, loads, executes and unloads through it. | A review-only archive; it cannot satisfy the release inventory or qualify pretrained models or other OSes. | Pull requests, pushes to `main` and ordinary manual dispatches, on Linux CI. |
| `review-audio-dependencies` | The frozen non-model audio dependency cohort executes under real Linux confinement and its controlled session lifecycle passes. | Does not qualify a pretrained speech model. | Pull requests, pushes to `main` and ordinary manual dispatches, on Linux CI. |
| `torch-native-e2e` | Managed Python and Torch CPU install, restart and sidecar lifecycle pass through the real native RPC (`torch-managed-python-acceptance.py`). | Real-network install evidence; not CUDA/MPS or image adapters, and `torch-server/tests_native` is not run (see [Security](SECURITY.md#dependency-scans)). The acceptance tooling runs on the exact CPython in `scripts/release/acceptance.python-version`, because it reads `.tar.zst` archives with 3.14's `compression.zstd`. | Ordinary manual dispatches and `v*` tags (not `build_candidates` runs) on native Linux, Windows and macOS. |
| Release target builds and tests in `build-rust` and `windows-release` | The non-Rustler workspace compiles warning-free and in release mode for each target, its tests pass under the `ci-test` profile, and the staged backend starts with the pinned ONNX Runtime (`onnx_runtime_probe.py`, `smoke-rpc.py`). | Linux debug tests overlap; runner-specific execution exposes OS behavior; startup is not inference. | Candidate runs only: Linux x64 and macOS arm64 in `build-rust`, Windows x64 in `windows-release`. |
| Headless archives in `headless-rpc` and `headless-inference` | Each no-inference and inference headless archive is produced with exactly the inventory entry `check-artifacts.mjs` expects; the inference producer binds source, schema and runtime bytes and runs authenticated owner startup and graceful shutdown, and an untrained synthetic ONNX graph runs through each extracted inference archive. | No pretrained-model or embedding-quality evidence. | Candidate runs only, native on Linux x64, macOS arm64 and Windows x64. |
| Electron-builder package steps in `build-electron`, `build-electron-no-inference`, `windows-release` and `windows-no-inference` | Each runner produces the full or no-inference installers the artifact plan names; `check-artifacts.mjs` rejects missing, empty, duplicate, and unexpected files. | Does not prove installation, startup, contents, signing, or user behavior. | Candidate runs only, on Linux, macOS, and Windows. |
| Package smoke checks (`smoke-linux-packages.py`, `smoke-macos-package.py`, `smoke-windows-packages.py`, each with `--variant no-inference` for that line) | Linux installers extract with resources matching build inputs and start their RPC; the macOS DMG copy and Windows NSIS install match build inputs and start RPC and desktop (Windows also the portable executable); no-inference packages lack inference routes. | Native startup evidence; signing, notarization, SmartScreen, interactive workflows, and packaged inference require separate acceptance. | Candidate runs only, on each native OS. |
| Staged release backend plus `launcher.sh --release-smoke` in `verify-launcher` | The Linux release layout builds and the Electron process starts under the recorded Xvfb smoke conditions for its bounded observation. | Build/startup only; no user workflow or non-Linux runtime claim. | Candidate runs only, on Linux Xvfb CI. |

`release-candidate` runs only after every candidate-run QA job succeeds. It
checks that the downloaded set is exactly the sixteen files in
`scripts/release/artifact-plan.json`, then writes SPDX inventories, unsigned
provenance, notices and SHA-256 checksums over the final files. Candidate
assembly never publishes a GitHub release. See [Releasing](../RELEASING.md) for
the remaining publication obligations.

### Pending Higher-Fidelity Claims

| Claim gap | Implementation owner |
| --- | --- |
| RPC DTO/error compatibility, hostile inputs, transport authorization, persistence interruption, and model-index lifecycle | [Rust library and RPC plan](plans/current-standards-remediation-2026-09-03/rust-library-and-rpc/plan.md) |
| Representative renderer workflows, accessibility interaction, decoded outcomes, cache provenance, and default/library-only behavior | [Frontend and UI plan](plans/current-standards-remediation-2026-09-03/frontend-and-ui/plan.md) |
| Generated decoder projection, host-language load/call, target runtime support, installer contents/startup, SBOM/provenance, and final release artifacts | [Desktop, release, bindings, and Torch plan](plans/current-standards-remediation-2026-09-03/desktop-release-bindings-and-torch/plan.md) |

These gaps remain pending until their focused plans record the required
contract, system, user-workflow, required-real, or release-artifact evidence.
A lower-fidelity row above cannot close them.

## Documentation Lifecycle

Update the nearest current owner document. Add a new document only when it has
a distinct durable audience or decision owner. Do not create per-directory
READMEs, duplicate command guides, or permanent implementation diaries.

Plans are temporary execution authority. Once complete or abandoned, preserve
durable decisions in an ADR/current guide and remove the plan. Audits are dated
snapshots and must name their code and standards baselines.

## Retained-download recovery qualification

The [contract](contracts/download-location-reconciliation.md) owns local acceptance;
[Opening an older library](library-upgrade.md#recover-a-failed-download-whose-files-moved)
describes the user action.
Run `cargo test --locked --manifest-path rust/Cargo.toml -p pumas-library retained_location`
with default features and `--no-default-features`. The native CI matrix runs both.
`scripts/acceptance/download_location_reconciliation.py RPC --temporary-parent PATH`
qualifies actual RPC and reopen using tiny authored fixtures. The companion
`download_location_reconciliation_ui.mjs ELECTRON FIXTURE [SOURCE_ELECTRON_DIRECTORY]`
requires that helper's authored marker and uses the actual renderer/preload/IPC/RPC
in the normal Electron sandbox. Qualify display/session and process isolation;
never direct these helpers at a real library.
