# Plan: Workspace Tooling Dependency Ownership Refactor

## Objective

Refactor the Node workspace tooling boundary so workspace-owned commands resolve
their runtime and test-environment dependencies from the owning `frontend`
workspace rather than from incidental root-level availability.

The resulting implementation must remove the root `jsdom` compatibility pin,
preserve the current developer-facing command contract, and comply with:
- `PLAN-STANDARDS.md`
- `COMMIT-STANDARDS.md`
- `CODING-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `DEPENDENCY-STANDARDS.md`
- `TOOLING-STANDARDS.md`
- `TESTING-STANDARDS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `DOCUMENTATION-STANDARDS.md`

## Scope

### In Scope

- Refactor the frontend workspace test-tooling setup so `vitest` and `jsdom`
  are owned and resolved from the `frontend` workspace boundary.
- Migrate the repo's Node workspace installation path from npm lockfile-driven
  installs to a pnpm workspace layout because standard npm install strategies do
  not satisfy the required ownership boundary cleanly.
- Remove the root-level `jsdom` devDependency from the workspace root once the
  frontend test command no longer depends on it.
- Update root/workspace scripts, lockfiles, CI, launcher tooling, and
  package-manager settings needed to make package ownership and execution
  boundaries align.
- Add verification that package-local commands do not rely on unrelated
  root-level devDependencies.
- Update plan traceability artifacts and any touched documentation that
  describes workspace dependency ownership or verification.

### Out of Scope

- Broad dependency upgrades unrelated to test-tooling ownership.
- Frontend runtime feature changes unrelated to test/build tooling.
- Electron, Rust, or launcher behavior changes beyond what is required to keep
  existing verification paths working after the tooling refactor.

## Inputs

### Problem

The repo currently violates the new dependency-ownership standard at the
frontend test boundary:

1. `frontend/package.json` declares `vitest` and `jsdom`, so the frontend
   workspace is the logical owner of the test runner and test environment.
2. Under the current npm workspace layout, `npm run -w frontend test:run`
   effectively resolves `vitest` from the hoisted workspace install, and the
   test environment only remains stable because the root `package.json` also
   carries `jsdom`.
3. That makes a workspace-local command succeed because of unrelated root-level
   dependency availability, which is exactly the cross-boundary coupling the new
   standards now forbid.
4. Testing the standard npm install strategies showed that `nested` and
   `shallow` still collapse resolution back to root-owned paths, while `linked`
   creates the needed workspace-local boundary but is marked experimental by
   npm and therefore does not meet the repo's long-term stability bar.

This means the original npm-scoped refactor assumption is invalid. The
standards-compliant long-term path is now a broader Node workspace tooling
migration that preserves the current command facade while replacing the install
and lockfile model underneath.

### Constraints

- The final state must remove the root `jsdom` compatibility pin rather than
  documenting it as permanent debt.
- The current developer-facing commands must remain valid:
  - `npm run -w frontend test:run`
  - `npm run -w frontend check:types`
  - `bash launcher.sh --build-release`
  - `bash launcher.sh --release-smoke`
- The solution must keep installs deterministic under committed lockfiles.
- The solution must not rely on accidental hoisting or incidental root
  dependency presence for correctness.
- The final package-manager path must not rely on experimental install-layout
  features for correctness.
- Cross-platform desktop support must remain intact for Linux and Windows, with
  macOS staying best-effort.
- Any package-manager configuration changes must be explicit, committed, and
  verifiable in CI and local development.

### Assumptions

- A pnpm workspace can preserve the current frontend command facade while
  producing the required workspace-local tool boundary.
- The repo can use `corepack` plus a pinned `packageManager` field to keep the
  package-manager runtime deterministic across local development, launcher
  tooling, and CI.
- The frontend workspace should remain the sole owner of `vitest` and `jsdom`
  unless another workspace begins using them directly.
- Existing frontend, Electron, launcher, and release checks are sufficient to
  verify the refactor if augmented with one ownership-focused verification path.

### Dependencies

- Root `package.json`
- `package-lock.json`
- `electron/package-lock.json`
- `pnpm-lock.yaml`
- `pnpm-workspace.yaml`
- `frontend/package.json`
- `frontend/vitest.config.ts`
- Launcher scripts and platform services under `scripts/launcher/`
- GitHub Actions workflow config under `.github/workflows/`
- Rust launcher updater logic under `rust/crates/pumas-core/src/launcher/`
- Existing launcher scripts and CI/release verification paths
- `docs/plans/README.md`
- Standards documents listed in Objective

### Affected Structured Contracts

- Workspace dependency ownership contract between the root manifest and
  `frontend/package.json`
- Frontend test command contract: `npm run -w frontend test:run`
- Root install/update contract for the launcher, CI, and local development
- Any package-manager or script resolution contract introduced to ensure
  workspace-local tooling execution
- CI/local verification contract that proves package-local commands do not rely
  on unrelated root-owned dependencies

### Affected Persisted Artifacts

- Root `package.json`
- `frontend/package.json`
- `package-lock.json`
- `electron/package-lock.json`
- `pnpm-lock.yaml`
- `pnpm-workspace.yaml`
- Any new or updated package-manager config files or workspace tool wrappers
- CI or validation scripts if required by the final ownership check
- Plan index content under `docs/plans/README.md`

### Concurrency and Race-Risk Review

- This refactor should not introduce any new polling, background jobs, retry
  loops, or long-lived subprocess ownership.
- The main operational risk is command-resolution ambiguity, not async overlap.
- Verification should continue to run commands serially where package-manager
  state or build output could interfere with concurrent runs.

### Public Facade Preservation Note

- Preserve the current developer-facing commands and launcher entrypoints.
- This is a facade-first tooling refactor: ownership and execution mechanics may
  change underneath, but the user-facing command surface should remain stable.
- If preserving the exact command surface proves impossible without violating
  the new dependency standards, re-plan before implementation continues.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| The package-manager migration changes install semantics or lockfile behavior in unexpected ways | High | Keep the migration explicit, commit the new lockfile and workspace metadata together, and verify install/build/test/release paths before each milestone closes. |
| Removing the root `jsdom` pin breaks frontend tests, CI, or release checks unexpectedly | High | Make root-pin removal a milestone-closing change only after ownership-correct frontend execution is verified locally. |
| pnpm's build-script approval model blocks Electron or esbuild installation in CI or local bootstrap | High | Encode the trusted build-script policy explicitly and verify release-oriented installs after the migration. |
| Root/workspace script changes silently drift from launcher or CI usage | Medium | Preserve existing command names and rerun launcher/release verification after the refactor. |
| npm-facing user commands stop working after the migration | Medium | Preserve the existing command facade deliberately and verify it explicitly alongside the new install path. |

## Definition of Done

- The root `package.json` no longer declares `jsdom`.
- The frontend workspace owns and successfully executes its test runner and test
  environment without relying on unrelated root-level devDependencies.
- `npm run -w frontend test:run` remains the canonical frontend test command and
  passes without the root `jsdom` pin.
- The repo uses a deterministic, committed pnpm workspace lock/config path
  instead of the previous npm lockfile path.
- Frontend, Electron, launcher, and release verification all pass after the
  refactor.
- The final state complies with the dependency-ownership rules now added to the
  Coding Standards repo.

## Milestones

### Milestone 1: Freeze the Ownership Contract

**Goal:** Record the exact ownership boundary, execution contract, and broadened
package-manager refactor constraints before changing tooling behavior.

**Tasks:**
- [ ] Record the current root/workspace dependency ownership and command surface
  that must be preserved.
- [ ] Identify the exact resolution path that causes frontend test execution to
  depend on root-level `jsdom`.
- [ ] Record the evidence that standard npm install strategies do not satisfy
  the ownership boundary cleanly.
- [ ] Decide the target pnpm-based ownership model that preserves the current
  command facade.
- [ ] Confirm which config/script files may change without violating launcher or
  cross-platform standards.
- [ ] Record the acceptance rule that the root `jsdom` pin must be removed in
  the final implementation state.

**Verification:**
- Review the chosen ownership model against `DEPENDENCY-STANDARDS.md`.
- Review the command-preservation approach against `TOOLING-STANDARDS.md`.
- Confirm the proposed scope still preserves the current facade contract.

**Status:** Not started

### Milestone 2: Refactor Frontend Test Tool Ownership

**Goal:** Establish the pnpm workspace boundary and remove the root `jsdom` pin
without breaking the current frontend command facade.

**Tasks:**
- [ ] Add the pnpm workspace metadata and pinned package-manager configuration.
- [ ] Remove the root `jsdom` devDependency once the frontend workspace can
  execute independently under pnpm.
- [ ] Keep `vitest` and `jsdom` ownership explicit in `frontend/package.json`
  unless a broader shared-owner rule becomes genuinely true.
- [ ] Replace the npm lockfiles with the committed pnpm workspace lock/config
  artifacts.
- [ ] Preserve the existing `npm run -w frontend ...` facade while proving the
  actual install path is now pnpm-owned.

**Verification:**
- `corepack pnpm install --frozen-lockfile`
- `npm run -w frontend check:types`
- `npm run -w frontend test:run`
- One ownership-specific check that proves the frontend test path no longer
  depends on root-owned `jsdom`

**Status:** Not started

### Milestone 3: Verification and Tooling Closure

**Goal:** Align launcher/CI/update tooling with pnpm and prove the refactor
works through the repo’s existing release-oriented paths.

**Tasks:**
- [ ] Refactor launcher dependency-install paths and package-manager abstractions
  to use the pinned pnpm workspace install flow.
- [ ] Refactor any updater/install code paths that still assume `npm install`.
- [ ] Update CI to cache and install with pnpm instead of npm lockfiles.
- [ ] Re-run the launcher- and release-oriented checks after the ownership
  refactor.
- [ ] Update any touched README or tooling documentation required by
  `DOCUMENTATION-STANDARDS.md`.
- [ ] Update the plan artifact and plan index for implementation traceability.
- [ ] Inspect unpushed history before each implementation commit to keep
  commit bodies and cleanup compliant.
- [ ] Confirm the remaining root `package.json` devDependencies are truly
  root-owned shared tooling only.

**Verification:**
- `npm run test:launcher`
- `bash launcher.sh --build-release`
- `bash launcher.sh --release-smoke`
- `git log --format='%h %s%n%b%n---' origin/main..HEAD`

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-04-12: Plan created before the workspace-tooling ownership refactor.
- 2026-04-12: Re-plan triggered after testing npm install strategies.
- 2026-04-12: `nested` and `shallow` still resolved frontend tooling through
  root-owned paths; `linked` created the needed boundary but npm marked it
  experimental.
- 2026-04-12: A pnpm workspace install preserved `npm run -w frontend test:run`
  while removing the root `jsdom` declaration, so the implementation scope was
  broadened to a pnpm workspace migration.

## Commit Cadence Notes

- Commit after each logical slice is complete and verified.
- Keep ownership-boundary changes, lockfile/config updates, and
  documentation/traceability updates in separate atomic commits where the
  verification scope stays obvious.
- Re-check unpushed history for regression/fix pairs before every new commit.
- Include detailed commit bodies and `Agent: codex` footer on implementation
  commits produced during this plan.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | Reason: The tooling boundary, lockfile, and command contract are tightly coupled in one workspace. | Revisit trigger: A later implementation slice becomes independently parallelizable without overlapping writes. | N/A |

## Re-Plan Triggers

- The pnpm migration breaks the preserved `npm run -w frontend ...` facade.
- A pnpm workspace install cannot be made deterministic across local, launcher,
  and CI paths.
- The trusted build-script policy for pnpm cannot be expressed non-interactively
  and committed safely.
- The refactor requires changing launcher or release contracts beyond the
  current preserved facade.

## Recommendations

- Prefer a real workspace package-manager boundary over script-level shims.
  This is better for maintainability because ownership is enforced by the
  installation model rather than by wrapper logic alone.
- Prefer preserving the current command names while changing the implementation
  underneath. This keeps developer ergonomics stable without compromising the
  new standards.
- Prefer a committed, pinned `corepack` + pnpm path over experimental npm
  install layouts when long-term stability is the priority.

## Completion Summary

### Completed

- None yet.
- Reason: Implementation has not started.

### Deviations

- None yet.
- Reason: No execution deviation exists before implementation.

### Follow-Ups

- None yet.
- Reason: Follow-up scope depends on the final ownership model chosen during
  implementation.

### Verification Summary

- None yet.
- Reason: Verification runs will be recorded during implementation.

### Traceability Links

- Module README updated: Pending implementation review
- ADR added/updated: None
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: Pending

## Brevity Note

Keep this plan focused on the workspace dependency-ownership refactor. Expand
detail only if npm workspace behavior or cross-platform tooling risk requires a
re-plan.
