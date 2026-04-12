# Plan: Cross-Platform Desktop Launcher Facade

## Objective

Create a truly cross-platform desktop entry workflow for Pumas Library by
moving launcher behavior into one shared implementation with thin
platform-specific wrappers, then update the root README to point at that
canonical flow.

The resulting implementation must comply with:
- `PLAN-STANDARDS.md`
- `CODING-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `LAUNCHER-STANDARDS.md`
- `DOCUMENTATION-STANDARDS.md`
- `TOOLING-STANDARDS.md`
- `TESTING-STANDARDS.md`

## Scope

### In Scope

- Define one canonical cross-platform launcher contract for local desktop
  workflows.
- Introduce a shared launcher core implementation that owns parsing,
  dependency checks, build/run orchestration, and error reporting.
- Keep `launcher.sh` as the required Unix entry point, but reduce it to a thin
  wrapper.
- Add a Windows wrapper with the same CLI contract.
- Update README quick start to document a real cross-platform path.
- Add verification for the shared launcher core and both wrappers.
- Add any needed directory README or traceability updates required by the
  documentation standards.

### Out of Scope

- Reworking the Electron, frontend, or Rust build systems beyond what is needed
  to expose a shared launcher flow.
- Changing release artifact formats or packaging strategy.
- Replacing the existing `launcher.sh` contract with a different CLI shape.
- Adding macOS-specific shell wrappers beyond what is needed to keep the shared
  launcher architecture compatible.
- Solving every cross-platform packaging issue in one pass.

## Inputs

### Problem

The root README currently presents a desktop quick start that only works on
Unix-like shells. That violates the documentation requirement that docs
describe the real supported path, and it creates a split between the launcher
standard and the actual developer/operator experience on Windows.

### Constraints

- Root `launcher.sh` must continue to exist and remain the primary Unix entry
  point per `LAUNCHER-STANDARDS.md`.
- Platform-specific logic must be isolated behind a single abstraction boundary
  per `CROSS-PLATFORM-STANDARDS.md`.
- The shared implementation must not scatter platform checks through business
  logic.
- The CLI contract must preserve the existing base lifecycle flags:
  - `--install`
  - `--build`
  - `--build-release`
  - `--run`
  - `--run-release`
  - `--help`
- Paths with spaces must remain supported.
- README quick start must only document workflows that are actually supported.
- New directories or non-obvious implementation areas must include `README.md`
  files if required by `DOCUMENTATION-STANDARDS.md`.

### Assumptions

- Node is the best cross-platform runtime for the shared launcher core because
  the repo already depends on Node for frontend and Electron workflows.
- PowerShell is the correct first-class Windows wrapper format.
- Bash remains the Unix wrapper format.
- The shared launcher core can be implemented without changing user-facing
  build outputs.
- macOS can use the Unix wrapper path without a separate dedicated wrapper in
  the first pass.

### Dependencies

- Existing root `launcher.sh`
- Root workspace `package.json`
- `electron/package.json`
- Existing build paths for Rust, frontend, and Electron
- Existing README rewrite work in `README.md`
- Standards documents listed in Objective

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Shared launcher core grows into an unstructured script blob | High | Keep parsing, dependency checks, build orchestration, and runtime launch in separate files/modules; enforce file size review thresholds. |
| Platform differences leak into shared flow | High | Keep platform detection in wrapper/factory layers only; isolate per-platform process/path behavior in dedicated files. |
| Wrapper contracts drift from shared core | High | Define one executable CLI contract first and make both wrappers delegate to the same core entry. |
| README gets updated before behavior is actually cross-platform | High | Update README only after wrapper and verification work is complete. |
| Windows quoting or path handling breaks in directories with spaces | High | Use platform-native argument forwarding and path APIs; add path-with-spaces smoke coverage. |
| Existing Unix workflows regress | Medium | Preserve current `launcher.sh` flags and semantics; verify Unix wrapper behavior against current commands. |
| Multiple entry points become equally canonical and confusing | Medium | Document one shared launcher contract and make raw npm/cargo commands secondary implementation detail. |

## Definition of Done

- A shared launcher core exists and is the single owner of launcher behavior.
- `launcher.sh` is a thin Unix wrapper over the shared core.
- A Windows wrapper exists and exposes the same CLI contract.
- Platform-specific behavior is isolated in dedicated wrapper/platform modules,
  not mixed into orchestration logic.
- The root README quick start documents a real cross-platform workflow.
- Verification covers the shared core and both wrapper entry points.
- Any new non-obvious directories include standards-compliant `README.md`.
- The change is split into atomic commits by logical slice.

## Milestones

### Milestone 1: Freeze The Launcher Contract

**Goal:** Define the shared desktop launcher contract before implementation so
wrappers and docs do not drift.

**Tasks:**
- [x] Document the canonical launcher contract: flags, exit codes, passthrough
  semantics, dependency model, and wrapper responsibilities.
- [x] Decide the shared implementation location and architectural role.
- [x] Record facade preservation: existing `launcher.sh` contract stays stable
  while implementation moves behind it.
- [x] Record wrapper boundary: wrappers detect platform and delegate; the
  shared core owns orchestration.
- [x] Identify any directory README or ADR traceability updates required.

**Verification:**
- Review contract against `LAUNCHER-STANDARDS.md`.
- Review boundary design against `CROSS-PLATFORM-STANDARDS.md` and
  `ARCHITECTURE-PATTERNS.md`.
- Confirm planned file layout stays within `CODING-STANDARDS.md`
  decomposition guidance.

**Status:** Complete

### Milestone 2: Build The Shared Launcher Core

**Goal:** Move launcher behavior into a platform-neutral implementation with
clean internal separation.

**Tasks:**
- [x] Create a shared launcher core entrypoint using Node.
- [x] Split core responsibilities into small modules:
  - CLI parsing and validation
  - dependency checks and install
  - build orchestration
  - run and run-release orchestration
  - process spawning and exit-code handling
- [x] Define a narrow platform service contract for any platform-specific
  behavior.
- [x] Implement platform-specific modules in separate files where behavior
  differs.
- [x] Preserve current lifecycle flags and error semantics.
- [x] Ensure argument forwarding and path handling work with spaces.

**Verification:**
- Run formatter and targeted checks for the shared launcher implementation.
- Add and run unit tests for CLI parsing and action validation.
- Add and run tests for path quoting and passthrough arg handling.
- Review module boundaries against `CODING-STANDARDS.md` and
  `ARCHITECTURE-PATTERNS.md`.

**Status:** Complete

### Milestone 3: Add Thin Platform Wrappers

**Goal:** Keep platform entry points minimal and standards-compliant while
delegating all logic to the shared core.

**Tasks:**
- [x] Refactor `launcher.sh` into a thin Bash wrapper.
- [x] Add `launcher.ps1` as the Windows wrapper with the same CLI contract.
- [x] Keep wrapper responsibilities limited to environment setup, locating the
  shared core, forwarding args, and propagating exit codes.
- [x] Ensure wrapper help behavior stays aligned with the shared core.
- [x] If needed, add minimal platform adapter files rather than inline
  branching in wrappers.

**Verification:**
- Smoke-test `launcher.sh --help`, `--build`, and `--run-release` behavior on
  Unix.
- Smoke-test `launcher.ps1 --help` and argument forwarding behavior in
  PowerShell-compatible execution.
- Confirm wrapper files do not duplicate orchestration logic.
- Confirm platform-specific behavior remains in one wrapper or platform file
  per platform where applicable.

**Status:** Complete

### Milestone 4: Update Documentation And Traceability

**Goal:** Make the README and any module docs accurately describe the new
supported cross-platform flow.

**Tasks:**
- [x] Update root `README.md` quick start to document the real cross-platform
  launcher entry path.
- [x] Add a launcher/tooling directory `README.md` if the new implementation
  location requires one under documentation standards.
- [x] Document platform-specific invocation examples without presenting
  Linux-only syntax as universal.
- [x] Add operator-facing notes for Windows execution policy or wrapper
  invocation if needed.
- [x] Add ADR documentation if the new launcher architecture introduces a
  lasting boundary decision that should be traceable.

**Verification:**
- Review docs against `DOCUMENTATION-STANDARDS.md`.
- Confirm README examples map to real, tested commands.
- Confirm no duplicated or contradictory quick-start paths remain.

**Status:** Complete

### Milestone 5: Verification And Release-Safety Pass

**Goal:** Prove the new launcher flow is safe to adopt as the canonical
cross-platform desktop quick start.

**Tasks:**
- [x] Add launcher-focused verification commands if missing.
- [x] Consider adding `--release-smoke` if the project qualifies under
  `LAUNCHER-STANDARDS.md`.
- [x] Run shared-core tests, wrapper smoke checks, frontend/electron
  validation, and at least one release build path.
- [x] Confirm the README-documented commands are the same ones being verified.
- [x] Prepare atomic commits per logical slice.

**Verification:**
- Shared launcher tests pass.
- Unix wrapper smoke checks pass.
- Windows wrapper smoke coverage passes or is validated in CI-compatible form.
- Existing frontend/electron validation still passes.
- Release build path still succeeds.
- Commit boundaries follow `COMMIT-STANDARDS.md`.

**Status:** Complete

## Execution Notes

Update during implementation:
- 2026-04-12: Plan created before launcher refactor begins.
- 2026-04-12: Added `scripts/launcher/` shared Node core plus launcher tests in
  commit `3a9314c`.
- 2026-04-12: Replaced duplicated root shell logic with thin Bash and
  PowerShell wrappers in commit `80deb8b`.
- 2026-04-12: Updated root README quick start and plan progress for the shared
  wrapper contract.
- 2026-04-12: Added launcher `--release-smoke` support plus bounded Electron
  startup exit in commit `e6ac10d`.
- 2026-04-12: Verified `npm run test:launcher`, `bash launcher.sh --help`,
  `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke`.
  The release smoke required unsandboxed execution because Electron's Linux
  sandbox cannot start inside the current container sandbox.

## Commit Cadence Notes

- Commit after each verified logical slice.
- Expected commit slices:
  - launcher contract and core scaffolding
  - shared launcher implementation
  - Unix wrapper refactor
  - Windows wrapper addition
  - README and traceability updates
  - verification and smoke additions if separate
- Follow `COMMIT-STANDARDS.md` for message format and history hygiene.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| N/A | N/A | N/A | N/A |

## Re-Plan Triggers

- The shared Node-based launcher core cannot faithfully preserve the existing
  `launcher.sh` CLI contract.
- Windows wrapper limitations force materially different flags or exit
  semantics.
- Existing build commands require platform-specific branching deeper than the
  wrapper or platform layer.
- README quick-start needs diverge from the actual verified launcher contract.
- The new implementation area grows enough to require a different
  package/directory role.

## Recommendations

- Recommendation 1: Prefer a small `scripts/launcher/` implementation area
  with separate modules and its own `README.md`.
  Why: This keeps orchestration code out of shell scripts, enforces a clear
  architectural role, and aligns with decomposition/documentation standards.
  Impact: Slightly larger initial refactor, better long-term maintainability.

- Recommendation 2: Add a bounded launcher smoke action if feasible, even if
  not done in the first commit.
  Why: A cross-platform quick start should be backed by a canonical sanity
  check, not just ad hoc manual runs.
  Impact: Small additional scope, strong regression protection.

## Completion Summary

### Completed

- Milestone 1: Launcher contract, architectural boundary, and traceability
  approach defined before implementation.
- Milestone 2: Shared Node-based launcher core implemented under
  `scripts/launcher/` with separated parsing, dependency, platform, and action
  modules.
- Milestone 3: Thin Bash and PowerShell root wrappers implemented over the
  shared core.
- Milestone 4: Root README and launcher module docs updated to describe the
  real cross-platform wrapper contract.
- Milestone 5: Added `--release-smoke` and completed launcher-facing
  verification against the documented commands.

### Deviations

- PowerShell runtime smoke was validated in CI-compatible contract form via
  wrapper tests and shared-core verification, but not executed in a live
  PowerShell host in this Linux environment.
- Electron release smoke required an unsandboxed run because the current
  container sandbox blocks Electron's Linux sandbox initialization.

### Follow-Ups

- Add Windows-host execution of `launcher.ps1 --release-smoke` or equivalent CI
  coverage when a Windows runner is available.
- Consider launcher-managed isolated state for bounded verification flows if
  the desktop runtime begins mutating more operator-visible local state.

### Verification Summary

- Reviewed standards:
  - `PLAN-STANDARDS.md`
  - `CODING-STANDARDS.md`
  - `ARCHITECTURE-PATTERNS.md`
  - `CROSS-PLATFORM-STANDARDS.md`
  - `LAUNCHER-STANDARDS.md`
  - `DOCUMENTATION-STANDARDS.md`
  - `TOOLING-STANDARDS.md`
  - `TESTING-STANDARDS.md`
- Executed verification:
  - `npm run test:launcher`
  - `bash launcher.sh --help`
  - `bash launcher.sh --build-release`
  - `bash launcher.sh --release-smoke`

### Traceability Links

- Module README updated: `scripts/launcher/README.md`
- ADR added/updated: None
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A in local
  implementation workflow

## Brevity Note

This plan is intentionally detailed only where architecture, platform
boundaries, and verification strategy affect execution.
