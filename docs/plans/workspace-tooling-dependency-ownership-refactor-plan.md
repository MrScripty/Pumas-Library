# Plan: Workspace Tooling Dependency Ownership Refactor

## Objective

Refactor the frontend test-tooling boundary so workspace-owned commands resolve
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
- Remove the root-level `jsdom` devDependency from the workspace root once the
  frontend test command no longer depends on it.
- Update any root/workspace scripts, config, or package-manager settings needed
  to make package ownership and execution boundaries align.
- Add verification that package-local commands do not rely on unrelated
  root-level devDependencies.
- Update plan traceability artifacts and any touched documentation that
  describes workspace dependency ownership or verification.

### Out of Scope

- Migrating the repo away from npm workspaces unless a re-plan trigger proves
  npm cannot satisfy the required ownership boundary correctly.
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

This is stable enough for short-term release preparation, but it is not
standards-compliant and is not the desired long-term architecture.

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
- Cross-platform desktop support must remain intact for Linux and Windows, with
  macOS staying best-effort.
- Any package-manager configuration changes must be explicit, committed, and
  verifiable in CI and local development.

### Assumptions

- The repo can likely remain on npm if workspace-local test-tool execution is
  made explicit and if package-manager behavior is configured to preserve the
  ownership boundary in practice.
- If npm workspace install/layout behavior still prevents ownership-correct
  execution after explicit refactoring, the repo may need a broader Node
  workspace tooling change rather than a small script adjustment.
- The frontend workspace should remain the sole owner of `vitest` and `jsdom`
  unless another workspace begins using them directly.
- Existing frontend, Electron, launcher, and release checks are sufficient to
  verify the refactor if augmented with one ownership-focused verification path.

### Dependencies

- Root `package.json`
- `package-lock.json`
- `frontend/package.json`
- `frontend/vitest.config.ts`
- Any npm workspace config files added or changed by the refactor
- Existing launcher scripts and CI/release verification paths
- `docs/plans/README.md`
- Standards documents listed in Objective

### Affected Structured Contracts

- Workspace dependency ownership contract between the root manifest and
  `frontend/package.json`
- Frontend test command contract: `npm run -w frontend test:run`
- Any package-manager or script resolution contract introduced to ensure
  workspace-local tooling execution
- CI/local verification contract that proves package-local commands do not rely
  on unrelated root-owned dependencies

### Affected Persisted Artifacts

- Root `package.json`
- `frontend/package.json`
- `package-lock.json`
- Any new or updated npm config files or workspace tool wrappers
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
| npm workspace layout still resolves frontend tooling through root-owned paths after script cleanup | High | Verify the real runtime resolution path early, choose an explicit workspace-local execution model, and re-plan immediately if npm cannot satisfy the boundary correctly. |
| Removing the root `jsdom` pin breaks frontend tests, CI, or release checks unexpectedly | High | Make root-pin removal a milestone-closing change only after ownership-correct frontend execution is verified locally. |
| Package-manager configuration changes alter install shape or lockfile behavior in CI | High | Keep config changes explicit, committed, and verified with `npm ci` plus the full frontend/release check stack. |
| Root/workspace script changes silently drift from launcher or CI usage | Medium | Preserve existing command names and rerun launcher/release verification after the refactor. |
| The repo needs a broader Node workspace strategy change than originally scoped | Medium | Use the re-plan triggers below instead of accepting a standards-violating partial fix. |

## Definition of Done

- The root `package.json` no longer declares `jsdom`.
- The frontend workspace owns and successfully executes its test runner and test
  environment without relying on unrelated root-level devDependencies.
- `npm run -w frontend test:run` remains the canonical frontend test command and
  passes without the root `jsdom` pin.
- The lockfile and any package-manager config remain deterministic and committed.
- Frontend, Electron, launcher, and release verification all pass after the
  refactor.
- The final state complies with the dependency-ownership rules now added to the
  Coding Standards repo.

## Milestones

### Milestone 1: Freeze the Ownership Contract

**Goal:** Record the exact ownership boundary, execution contract, and refactor
constraints before changing tooling behavior.

**Tasks:**
- [ ] Record the current root/workspace dependency ownership and command surface
  that must be preserved.
- [ ] Identify the exact resolution path that causes frontend test execution to
  depend on root-level `jsdom`.
- [ ] Decide the target ownership model for frontend-local test execution under
  the current npm workspace setup.
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

**Goal:** Make frontend test tooling execute correctly from the `frontend`
boundary without root-owned `jsdom`.

**Tasks:**
- [ ] Refactor frontend test runner invocation and/or package-manager config so
  workspace-local execution no longer depends on incidental root resolution.
- [ ] Remove the root `jsdom` devDependency once the frontend workspace can
  execute independently.
- [ ] Keep `vitest` and `jsdom` ownership explicit in `frontend/package.json`
  unless a broader shared-owner rule becomes genuinely true.
- [ ] Update lockfile and any supporting config deterministically.
- [ ] Add or update any helper scripts only if they keep the current command
  surface intact and make ownership clearer.

**Verification:**
- `npm ci`
- `npm run -w frontend check:types`
- `npm run -w frontend test:run`
- One ownership-specific check that proves the frontend test path no longer
  depends on root-owned `jsdom`

**Status:** Not started

### Milestone 3: Verification and Tooling Closure

**Goal:** Prove the refactor works through the repo’s existing release-oriented
tooling paths and close documentation/traceability gaps.

**Tasks:**
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

- npm cannot be configured to satisfy ownership-correct frontend test execution
  while preserving the current command facade.
- Removing the root `jsdom` pin requires a broader package-manager migration
  than this plan assumes.
- A package-manager config change materially alters cross-platform install or CI
  behavior.
- The refactor requires changing launcher or release contracts beyond the
  current preserved facade.

## Recommendations

- Prefer one explicit workspace-local execution model over relying on npm
  hoisting behavior. This is better for maintainability because it makes
  ownership obvious and auditable.
- Prefer preserving the current command names while changing the implementation
  underneath. This keeps developer ergonomics stable without compromising the
  new standards.
- If npm cannot satisfy the required ownership boundary cleanly, prefer
  re-planning to a fuller Node workspace tooling change over accepting a
  standards-violating partial fix.

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
