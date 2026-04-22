# Pass 01 - Global Governance, Documentation, Tooling

## Standards Consulted
- `CODING-STANDARDS.md`
- `DOCUMENTATION-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `TESTING-STANDARDS.md`
- `TOOLING-STANDARDS.md`
- `DEPENDENCY-STANDARDS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `RELEASE-STANDARDS.md`
- `PLAN-STANDARDS.md`

## Repository Shape
Pumas Library is a multi-runtime desktop app:

- `frontend/`: React 19 + Vite + Vitest renderer.
- `electron/`: Electron main/preload wrapper.
- `rust/`: Cargo workspace with core library, app manager, RPC server, UniFFI, Rustler.
- `torch-server/`: Python FastAPI inference sidecar.
- `scripts/launcher/` and `launcher.sh`: canonical launcher entrypoint and support scripts.
- `bindings/csharp/`: host-language binding docs and smoke harness.

## Findings

### G01 - Source Directory README Coverage Is Incomplete
Status: addressed for audited source/support roots as of 2026-04-22

The standards require every directory under source roots to contain a `README.md`. The audit found 17 relevant source/support directories without local README contracts:

```text
rust/crates
rust/crates/pumas-rpc
rust/crates/pumas-rpc/tests
rust/crates/pumas-uniffi
rust/crates/pumas-app-manager
rust/crates/pumas-rustler
rust/crates/pumas-core/tests
rust/crates/pumas-core/tests/fixtures
rust/crates/pumas-core/tests/fixtures/dependency_requirements
torch-server
torch-server/loaders
scripts/templates
scripts/dev
bindings
bindings/csharp-test
bindings/csharp/Pumas.NativeSmoke
launcher-data/plugins
```

Existing READMEs under many Rust submodules are useful, but they do not satisfy the top-level source-root coverage requirement or the standards' required sections for API consumer contracts and structured producer contracts.

Rectification:
- Add READMEs to each listed directory.
- Use explicit `None` statements with `Reason:` and `Revisit trigger:` where sections do not apply.
- Give `launcher-data/plugins` a structured producer/consumer contract because plugin JSON is machine-consumed.
- Give `bindings/csharp/Pumas.NativeSmoke` a host-facing contract because it validates generated binding artifacts.

Implementation notes:
- README contracts were added for the listed Rust crate, Torch sidecar, script, binding, and plugin directories.
- `rust/README.md` now documents the Cargo workspace boundary, default member policy, lockfile contract, and generated `target/` exclusion.
- `scripts/dev/check-readme-coverage.sh` verifies README coverage for the audited source/support roots and skips generated dependency, cache, and build output directories.

### G02 - File Size Decomposition Triggers Are Systemic
Status: non-compliant, requires staged refactor

The standards set a 500-line target and require explicit decomposition review above that threshold. Current hotspots:

Rust files over 500 lines:

```text
8533 rust/crates/pumas-core/src/model_library/library.rs
2107 rust/crates/pumas-core/src/model_library/importer.rs
1891 rust/crates/pumas-uniffi/src/bindings.rs
1710 rust/crates/pumas-core/src/model_library/hf/download.rs
1554 rust/crates/pumas-core/src/index/model_index.rs
1537 rust/crates/pumas-core/src/api/reconciliation.rs
1531 rust/crates/pumas-core/src/model_library/dependencies.rs
1377 rust/crates/pumas-core/src/model_library/model_type_resolver.rs
1348 rust/crates/pumas-core/src/api/hf.rs
1295 rust/crates/pumas-app-manager/src/version_manager/installer.rs
1252 rust/crates/pumas-core/src/api/state.rs
1071 rust/crates/pumas-core/src/models/model.rs
1063 rust/crates/pumas-core/src/model_library/identifier.rs
993 rust/crates/pumas-core/src/registry/library_registry.rs
989 rust/crates/pumas-core/src/model_library/hf_cache.rs
961 rust/crates/pumas-rpc/tests/integration_tests.rs
960 rust/crates/pumas-core/src/model_library/mapper.rs
```

Frontend non-test files over 500 lines:

```text
2176 frontend/src/types/api.ts
563 frontend/src/App.tsx
507 frontend/src/components/model-import/useModelImportWorkflow.ts
```

Frontend UI components over the 250-line component trigger include `App.tsx`, `LocalModelsList.tsx`, `ModelManager.tsx`, `InstallDialog.tsx`, `ConflictResolutionDialog.tsx`, `TorchModelSlotsSection.tsx`, `ImportLookupCard.tsx`, `RemoteModelListItem.tsx`, `AppSidebar.tsx`, and others.

Rectification:
- Do not split files mechanically. Start with contract extraction and state-machine ownership, then split along those boundaries.
- Add a decomposition-review register for any file that remains over threshold after the first extraction round.

### G03 - Project Standards Are Duplicated Instead of Adopted as a Single Traceable Contract
Status: remediated for adoption-map traceability

The repo has local standards docs:

- `docs/CODING_STANDARDS.md`
- `docs/REACT_ARIA_ENFORCEMENT.md`
- `docs/TESTING.md`
- `docs/SECURITY.md`

These do not yet map directly to the external standards library. For example, `docs/CODING_STANDARDS.md` focuses on React Aria while the external coding standards also cover file size, layering, backend-owned data, composition roots, configuration, validation, and error handling.

Rectification:
- Completed: `docs/STANDARDS_ADOPTION.md` maps external standards to project status, enforcement, follow-ups, exception policy, completed adoption steps, and revisit triggers.
- Completed: root `README.md` and `CONTRIBUTING.md` link the adoption map from the primary onboarding paths.
- Completed: module README and audit updates have accompanied standards-impacting implementation commits in this pass.

### G04 - Tooling Enforcement Is Incomplete
Status: partially remediated

Missing or partial enforcement at audit time:

- `.editorconfig` coverage was too narrow for the repository's language mix;
- no `lefthook.yml` found;
- no committed commit-message validation was present before the enforcement pass;
- no `.github/` CI workflow found in the file inventory;
- frontend package used an ESLint 9 flat config with a legacy `--ext` lint command at audit time;
- Electron package also used a legacy ESLint command shape and did not own a flat config before the enforcement pass;
- frontend explicitly disables `max-lines`, `max-lines-per-function`, and `complexity`, despite active decomposition violations;
- Rust workspace lacks `[workspace.lints]` and member `[lints] workspace = true` opt-ins;
- no visible Rust audit policy for `cargo audit`, `cargo deny`, duplicate dependencies, or unused dependencies;
- no Criterion benchmarks despite performance-sensitive model indexing, metadata, download, and conversion paths.

Rectification:
- Add `.editorconfig` from the standards template, adjusted for Rust/TypeScript/Python/shell.
- Add `lefthook.yml` with fast pre-commit checks and slower pre-push checks.
- Add commit-message validation for conventional commit subjects.
- Add CI matrix for Linux and Windows at minimum.
- Enable Rust workspace lints in a staged mode that initially warns on legacy issues, then ratchets.
- Restore frontend complexity/file-size enforcement after decomposition baselines are set.

Implementation notes:
- The existing `.editorconfig` was expanded to cover the standards template's TypeScript, Rust, Python, shell, C#, YAML/JSON, Docker, Make, and Markdown formatting boundaries.
- Electron linting now uses a package-local ESLint 9 flat config, a command that avoids the legacy `--ext` flat-config pitfall, and CI coverage in the Electron packaging job.
- The repository uses `pre-commit` instead of Lefthook today; `scripts/dev/check-commit-message.sh` has been added as a commit-msg hook to enforce conventional commit subjects while broader hook migration remains a separate tooling task.
- Frontend file-size enforcement now uses `frontend/scripts/check-file-size.js` with a committed baseline ratchet and runs in the frontend CI job.

### G05 - Dependency Ownership Does Not Match Workspace Execution Boundaries
Status: remediated for Node workspaces

The dependency standards require each package to declare the tools it executes. Current problems:

- root `package.json` declares `typescript` and `@types/node`, while `frontend/package.json` runs `tsc --noEmit` and `electron/package.json` runs `tsc`, but neither workspace declares `typescript`;
- `electron/package.json` has a `lint` script but does not declare ESLint or TypeScript ESLint dependencies;
- root package owns only `test:launcher`, so root dev dependencies should be limited to tools needed by root scripts unless intentionally shared via catalog plus package-local declarations.

Rectification:
- Move execution-owned TypeScript, ESLint, and type dependencies to the workspaces that run those commands.
- Use `pnpm-workspace.yaml` catalog entries for shared versions only, not hidden ownership.
- Verify package-local commands via workspace-scoped commands.

Implementation notes:
- Frontend and Electron now declare TypeScript/ESLint tooling in the packages that execute those commands.
- Electron linting has a package-local flat config and passes through `corepack pnpm --filter ./electron lint`.
- `scripts/dev/check-workspace-dependency-ownership.mjs` verifies root package manifests remain tool-free and workspace manifests keep the TypeScript, ESLint, Vite, Vitest, Electron, and related tools they execute.
- CI runs the dependency ownership check in the workflow lint stage before workspace jobs fan out.

### G06 - Release Governance Is Present but Not Complete
Status: partially remediated

Positive evidence:

- versions are aligned at `0.4.0` across root, frontend, electron, and Rust workspace;
- `CHANGELOG.md`, `RELEASING.md`, SBOM files, and third-party notices exist;
- Electron package includes Linux, Windows, and macOS packaging targets.

Gaps:

- release CI matrix was not visible in the initial repository inventory;
- checksum generation needed an explicit self-exclusion guard;
- release artifact naming policy needed to tie desktop app, native libraries, generated bindings, and SBOMs together;
- generated C# binding packaging exists, but release identity needs to distinguish product-native library from binding framework artifacts per language-binding standards.

Rectification:
- Add `docs/release-artifact-contract.md` describing artifact names, platform targets, checksum/SBOM naming, and native-library/binding compatibility.
- Add CI release jobs or document why release automation is currently manual.

Implementation notes:
- `docs/contracts/release-artifacts.md` defines the release artifact matrix, checksum file contract, SBOM contract, and native binding compatibility rules.
- `.github/workflows/build.yml` stages release artifacts and generates `checksums-sha256.txt` without including the checksum file in its own digest list.
- `scripts/dev/check-release-version-alignment.mjs` verifies release-facing versions stay aligned across root, frontend, Electron, and the Rust workspace; CI runs it before release-capable jobs fan out.
- Remaining: release-published SBOM generation is still manual and should be automated once the SBOM generator no longer depends on a pre-existing local virtual environment.

## Pass 01 Refactor Inputs
These findings feed the synthesis plan as foundation work:

- documentation contract pass;
- tooling/enforcement pass;
- dependency ownership pass;
- release artifact contract pass;
- decomposition review pass.
