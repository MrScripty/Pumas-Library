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

```bash
./scripts/rust/check.sh
cargo test --manifest-path rust/Cargo.toml -p pumas-library <test-filter>
```

The aggregate script runs formatting, all-target/all-feature checks, Clippy,
workspace tests and docs, and no-default compilation. It excludes
`pumas_rustler`, which needs an Erlang/OTP host.

### Frontend

```bash
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w frontend build
npm run -w frontend build:library-only
```

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

Theme tokens live in `frontend/src/index.css`; programmatic mappings live in
`frontend/src/config/theme.ts`. Use semantic tokens rather than hard-coded
colors. Do not claim contrast or reduced-motion compliance without current
representative evidence.

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
operator authority and the hooks do not establish repository acceptance. CI
runs on pull requests targeting `main`, pushes to `main`, and version-tag
pushes, and manual pre-tag rehearsals. A failing step blocks its workflow and dependants; whether a workflow
is required to merge is external branch-protection state and is not asserted
here.

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
| Actionlint in `lint-workflows` | GitHub workflow structure and expressions satisfy Actionlint's model; Actionlint is the independent parser/static oracle. | Deeper than generic YAML parsing for `.github/workflows`; does not execute jobs. | Workflow-blocking on Linux CI for every CI trigger. |
| `check:dependency-ownership` in `lint-workflows` | Root owns no runtime/dev packages and each workspace declares the tool set the repository policy assigns it; manifests are inputs and the repository checker owns the mapping. | Package installation proves resolution, not declaration ownership. | Workflow-blocking, deterministic Node check on Linux CI; local on demand. |
| `check:release-versions` in `lint-workflows` and release preparation | Root, frontend, Electron, and Rust workspace release versions are identical and version tags match them; their manifest values are the authoritative inputs. | Packaging may expose a mismatch later; this provides direct causal diagnosis before builds. | Workflow-blocking, deterministic Node check on Linux CI; locally required before release. |
| Cross-target Rust release build in `build-rust` | The non-Rustler workspace compiles in release mode for each declared target and produces the named native/RPC files; Cargo and file production are the oracle. | Rust quality checks debug/all-feature contracts; target builds uniquely prove target compilation/artifact creation, not runtime support. | Workflow-blocking on Linux x64, macOS arm64, and Windows x64; Windows runs in `windows-candidate`. |
| Cross-platform Rust release tests in `build-rust` | The non-Rustler workspace tests pass in release mode on each runner OS; test assertions are the behavior oracles. | Linux debug tests overlap intentionally; runner-specific execution can expose OS/path/process behavior. | Workflow-blocking on Linux, macOS, and Windows. |
| `scripts/rust/check.sh` in `rust-quality` | Formatting, all-target/all-feature compilation, Clippy warnings, workspace tests, doctests, and no-default compilation each satisfy their named tool/assertion contract. | Complements release target builds with static, debug, docs, and feature evidence; excludes the separately owned Rustler host claim. | Workflow-blocking on representative Linux CI; local aggregate command. |
| Frontend ESLint in `build-frontend` | Type-aware and frontend lint rules accept the configured source scope; ESLint's AST/type analysis is the oracle for only those rules. | TypeScript overlaps type facts, not lint-specific source/interaction policy. | Workflow-blocking on representative Linux CI; local on demand. |
| Frontend `check:types` in `build-frontend` | The configured renderer TypeScript program type-checks without emit; the compiler is authoritative for static consistency. | Build also type-checks while emitting; the no-emit command gives direct diagnosis before tests/build. | Workflow-blocking on representative Linux CI; local on demand. |
| Frontend `test:run` in `build-frontend` | Vitest component/unit assertions pass in jsdom or their selected simulation. | Decides local renderer behavior only; representative browser/Electron workflows remain separately owned. | Workflow-blocking on representative Linux CI; local on demand. |
| Frontend default build in `build-frontend` | Vite emits the default renderer bundle consumed by Electron; build completion and output production are the oracle. | Does not prove renderer behavior or library-only mode. | Workflow-blocking on Linux CI; output is an input to all Electron package jobs. |
| `test:launcher` in `verify-launcher` | Shared CLI parsing, closed platform selection, action delegation, package-manager invocation, wrapper delegation, and controlled max/grace/force/process-tree outcomes satisfy their Node and child-process assertions. | Release smoke traverses a real startup path; the focused suite diagnoses contract and cleanup failures but Linux execution does not prove Windows or macOS process behavior. | Workflow-blocking on representative Linux CI; local on demand. |
| Torch Ruff lint and format checks in `torch-quality` | Python source satisfies the selected Ruff diagnostics and formatting projection. | Static support only; unit and real-runtime claims remain distinct. | Workflow-blocking on representative Linux CI; local on demand. |
| Torch unit suite in `torch-quality` | Sidecar unit assertions pass with their declared local fakes/substitutes. | Does not prove ASGI, middleware, model loading, GPU, or inference in a resolved Torch environment. | Workflow-blocking on representative Linux CI; local on demand. |
| Staged release backend plus `launcher.sh --release-smoke` in `verify-launcher` | The Linux release layout builds and the Electron process starts under the recorded Xvfb smoke conditions for its bounded observation. | Build/startup only; no user workflow or non-Linux runtime claim. | Workflow-blocking on Linux Xvfb CI. |
| Electron ESLint in `build-electron` | Electron TypeScript/JavaScript satisfies the configured lint rules. | Deterministic lint runs once; platform tests retain target-specific value. | Workflow-blocking on the Linux Electron matrix member. |
| Electron `test` in `build-electron` | TypeScript emits and Node tests prove the current IPC allowlist/request, launcher, and process-boundary assertions on each packaging OS. | Its build is the package input, so a second unchanged compile had no marginal value and was removed. | Workflow-blocking on Linux x64, macOS arm64, and Windows x64; Windows runs in `windows-candidate`. |
| Electron-builder package step in `build-electron` | Each runner produces an installer/archive accepted by electron-builder for its configured target. | Does not prove installation, startup, contents, signing, or user behavior; those remain platform/release claims. | Workflow-blocking on Linux, macOS, and Windows; the artifact-plan inventory rejects missing, empty, duplicate, and unexpected installers. |
| `smoke-linux-packages.py` in `build-electron` | Both exact Linux installers extract, contain resources matching build inputs, and start their bundled RPC server. | Does not prove system installation, desktop user flows, signing, or packaged model inference. | Workflow-blocking on Linux; native macOS and Windows use their respective installer smoke checks. |
| `smoke-windows-packages.py` in `windows-candidate` | The NSIS candidate installs, installed resources match build inputs, and the installed RPC, installed desktop, and portable desktop start successfully. | Native installation/startup evidence; signing, SmartScreen, interactive workflows, and packaged inference require separate acceptance. | Workflow-blocking on native Windows x64; both Windows installers are required for combined candidate assembly. |

The replacement workflow also runs no-default core and RPC tests in separate
Cargo invocations, builds a headless release, and checks health plus disabled
inference routes without the desktop toolchain. `release-candidate` assembles
only the required Linux/macOS installer set in `scripts/release/artifact-plan.json` and generates
SHA-256 checksums; all required QA jobs must succeed first. These steps run before
tags as well as on tags. The separate non-blocking `windows-candidate` job emits both Windows installers
and checksums only if its native checks pass. Candidate assembly never publishes
a GitHub release.
See [Releasing](../RELEASING.md) for the remaining publication obligations.

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
