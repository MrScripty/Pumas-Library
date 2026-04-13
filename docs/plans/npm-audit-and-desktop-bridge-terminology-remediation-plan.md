# Plan: npm Audit and Desktop Bridge Terminology Remediation

## Objective

Eliminate the current full-workspace `npm ci` vulnerability findings and remove
legacy PyWebView-first terminology from the desktop bridge without breaking the
existing Electron renderer contract or the repo's cross-platform launcher and
packaging paths.

The resulting implementation must comply with:
- `PLAN-STANDARDS.md`
- `COMMIT-STANDARDS.md`
- `CODING-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `DEPENDENCY-STANDARDS.md`
- `DOCUMENTATION-STANDARDS.md`
- `TOOLING-STANDARDS.md`
- `TESTING-STANDARDS.md`

## Scope

### In Scope

- Upgrade the direct Node/Electron dependencies needed to clear the current
  workspace audit findings.
- Refresh the root lockfile and any affected workspace manifest versions in a
  deterministic, standards-compliant way.
- Keep the Electron desktop bridge contract stable while renaming PyWebView-era
  primary types, comments, and docs to current Electron/desktop bridge terms.
- Preserve `window.electronAPI` as the canonical runtime facade.
- Keep `window.pywebview.api` only as an explicitly deprecated compatibility
  alias if runtime compatibility still requires it.
- Update repo documentation and plan traceability artifacts for the renamed
  bridge contract and dependency floor changes.
- Verify the remediation through the existing frontend, Electron, launcher, and
  release checks.

### Out of Scope

- Reworking the updater feature beyond what is required to preserve existing
  behavior after the dependency and terminology cleanup.
- Introducing a new frontend-backend transport or removing the current preload
  bridge architecture.
- Replacing npm with a different package manager.
- Large UI redesign work unrelated to the bridge naming cleanup.
- Broad Rust backend changes unrelated to the audit or bridge-contract
  terminology drift.

## Inputs

### Problem

The repo currently has two kinds of drift:

1. Full-workspace `npm ci` still installs vulnerable build/dev dependencies,
   primarily from outdated Electron packaging and frontend tooling versions.
2. The desktop renderer bridge still uses PyWebView-first naming in types,
   comments, and docs even though the desktop app is now Electron-based.

This creates security noise in CI and release prep, while also preserving an
outdated architecture story that makes current code harder to understand.

### Constraints

- The current production dependency set is already clean under
  `npm audit --omit=dev`; remediation should stay targeted and should not add
  unnecessary runtime dependencies.
- Lockfile-driven installs must remain deterministic and committed.
- Platform-specific behavior must remain isolated at wrapper/preload or thin
  platform-module boundaries, not spread through application logic.
- The canonical desktop renderer facade must remain `window.electronAPI`.
- Any retained `window.pywebview.api` surface must be treated as a deprecated
  compatibility alias, not as the primary contract.
- Linux and Windows remain required desktop targets; macOS remains best-effort.
- Paths with spaces and the shared launcher entry flow must continue to work.
- No new polling loops or background update ownership changes should be
  introduced by this remediation pass.

### Assumptions

- Upgrading `electron` to a patched `39.8.x` line or newer and
  `electron-builder` to `26.8.1` or newer is the primary fix for the Electron
  packaging audit findings.
- Refreshing the frontend toolchain and lockfile will resolve the remaining
  Vite-era transitive advisories without requiring a transport or architecture
  rewrite.
- Some PyWebView references are pure documentation drift, while a smaller set
  are deliberate compatibility shims that should remain only as deprecated
  aliases in this pass.
- Existing launcher, frontend, and Electron checks are sufficient to verify the
  remediation if run together as one acceptance path.

### Dependencies

- Root `package.json` and `package-lock.json`
- `frontend/package.json`
- `electron/package.json`
- `frontend/src/types/api.ts`
- `frontend/src/api/adapter.ts`
- `electron/src/preload.ts`
- Desktop-shell and update-check UI tests
- Standards documents listed in Objective

### Affected Structured Contracts

- Frontend bridge type surface currently centered on `PyWebViewAPI`
- Renderer globals exposed by preload:
  - `window.electronAPI` as the canonical facade
  - `window.pywebview.api` as the compatibility facade if still required
- Header/update-check API typings used by the renderer
- npm workspace manifest and lockfile resolution contract

### Affected Persisted Artifacts

- Root `package-lock.json`
- Workspace `package.json` manifests under `frontend/` and `electron/`
- Documentation that describes the desktop bridge contract
- Plan index content under `docs/plans/README.md`

### Concurrency and Race-Risk Review

- This remediation should not introduce new polling, retry, or background task
  ownership. Update checks remain explicit one-shot actions initiated by the
  current UI flow.
- The bridge rename must not create two independently evolving renderer
  contracts. One canonical type/facade remains authoritative; any legacy alias
  must delegate to it and stay behaviorally identical.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| `electron-builder` major-version upgrade changes packaging behavior or config requirements | High | Upgrade the packaging chain in its own logical slice, run targeted Electron validation plus release build checks, and re-plan if packaging semantics change materially. |
| Lockfile refresh resolves to vulnerable or duplicate versions unexpectedly | High | Prefer direct dependency upgrades first, inspect the resolved tree after each change, and use narrow `overrides` only when a direct upgrade cannot clear a transitive advisory. |
| Renaming the bridge types breaks renderer code or hidden compatibility consumers | High | Use facade-first preservation: rename the primary type, keep deprecated aliases temporarily, and verify the full renderer-to-preload path before removing any compatibility surface. |
| Docs and code drift again because the alias remains visible | Medium | Mark the alias as deprecated in code comments/docs and update canonical examples to use the new bridge terminology only. |
| Cross-platform packaging or launcher flows regress while clearing audit findings | Medium | Re-run launcher tests, Electron validation, and release smoke/build checks after dependency changes; avoid OS-specific behavior changes outside the existing abstraction boundaries. |

## Definition of Done

- Full-workspace `npm audit` is clean, or any unavoidable upstream-only residue
  is explicitly documented with a re-plan decision and no production exposure.
- The direct dependency floors required for the audit remediation are updated in
  workspace manifests and lockfile.
- The desktop bridge uses current Electron/desktop bridge terminology as the
  primary contract in code and docs.
- `window.electronAPI` remains the canonical runtime facade.
- Any retained `window.pywebview.api` compatibility path is clearly marked as
  deprecated and remains behaviorally identical to the canonical bridge.
- Frontend, Electron, launcher, and release verification checks pass after the
  remediation.
- Implementation proceeds in atomic, standards-compliant commits after this
  plan.

## Milestones

### Milestone 1: Freeze Dependency and Bridge Boundaries

**Goal:** Lock the remediation boundary before changing versions or bridge
names so the implementation preserves architecture and compatibility.

**Tasks:**
- [x] Record the direct dependency upgrade targets required by the current audit
  output.
- [x] Record the public-facade preservation decision:
  `window.electronAPI` stays canonical and any `pywebview` surface becomes a
  deprecated compatibility alias only.
- [x] Identify the documentation and directory-traceability updates required by
  the touched frontend/Electron areas.
- [x] Decide whether likely transitive fixes should come from direct version
  bumps, lockfile refresh, or narrow npm `overrides`.
- [x] Confirm no new polling, retries, or background ownership changes are in
  scope for this pass.

**Verification:**
- Review the chosen facade and compatibility boundary against
  `ARCHITECTURE-PATTERNS.md` and `CROSS-PLATFORM-STANDARDS.md`.
- Review the dependency approach against `DEPENDENCY-STANDARDS.md`.
- Confirm the planned file/module footprint still fits
  `CODING-STANDARDS.md`.

**Status:** Completed on 2026-04-12

### Milestone 2: Remediate Workspace Audit Findings

**Goal:** Clear the vulnerable Node/Electron dependency chain without adding
unnecessary runtime footprint or destabilizing packaging.

**Tasks:**
- [x] Upgrade the direct Electron packaging dependencies to patched versions.
- [x] Refresh the frontend toolchain and lockfile resolution to remove the
  vulnerable Vite-era transitive packages.
- [x] Add narrow npm `overrides` only if direct upgrades and lockfile refresh
  do not eliminate a remaining transitive advisory.
- [x] Keep manifest and lockfile updates deterministic and reviewable.
- [x] Update CI or release-tooling references only if the dependency upgrades
  require them.

**Verification:**
- `npm ci --include=optional`
- `npm audit`
- `npm audit --omit=dev`
- `npm run -w frontend check:types`
- `npm run -w frontend test:run`
- `npm run -w electron validate`
- `npm run test:launcher`

**Status:** Completed on 2026-04-12

### Milestone 3: Rename the Primary Desktop Bridge Contract

**Goal:** Replace PyWebView-first naming with current desktop bridge naming
while preserving runtime compatibility.

**Tasks:**
- [x] Rename the primary bridge interface/types to Electron/desktop
  bridge-first terminology.
- [x] Update adapter, preload, comments, and error text to point at the
  canonical bridge contract.
- [x] Retain a deprecated type alias and deprecated `window.pywebview.api`
  compatibility exposure only where runtime compatibility still needs it.
- [x] Update examples and tests so new code paths use the canonical bridge
  names.
- [x] Add or update any directory `README.md` files required by
  `DOCUMENTATION-STANDARDS.md` for touched source directories.

**Verification:**
- `npm run -w frontend check:types`
- Targeted frontend tests covering the header/update path and bridge adapter
- Electron preload/build validation via `npm run -w electron validate`
- One cross-layer acceptance path that proves renderer calls still reach the
  backend bridge through the preload facade

**Status:** Completed on 2026-04-12

### Milestone 4: Documentation and Release-Safety Closure

**Goal:** Make the repo docs and release workflow reflect the cleaned
dependency and bridge story, then close with a full verification pass.

**Tasks:**
- [x] Remove stale PyWebView-first terminology from repo docs that describe the
  current Electron desktop flow.
- [x] Update plan index or other traceability docs for the new remediation
  artifact.
- [x] Verify that README and contributor docs describe the canonical bridge and
  current install/build reality.
- [x] Run the full remediation verification stack, including at least one
  release-oriented launcher path.
- [x] Inspect unpushed history before each new implementation commit to keep
  commit bodies and cleanup compliant.

**Verification:**
- Review docs against `DOCUMENTATION-STANDARDS.md`
- `bash launcher.sh --build-release`
- `bash launcher.sh --release-smoke`
- `git log --format='%h %s%n%b%n---' origin/main..HEAD`

**Status:** Completed on 2026-04-12

## Execution Notes

Update during implementation:
- 2026-04-12: Plan created before dependency and bridge remediation work.
- 2026-04-12: Backfilled detailed bodies on the current unpushed local commits
  so the branch history matches `COMMIT-STANDARDS.md` before new implementation
  slices begin.
- 2026-04-12: Cleared workspace npm audit findings and refreshed the lockfile
  via `d017cfc7`.
- 2026-04-12: Renamed the canonical renderer bridge to `DesktopBridgeAPI`,
  preserved the deprecated compatibility alias, and updated touched module
  READMEs via `04bccac7`.
- 2026-04-12: Updated repo-facing architecture and contributor docs to match
  the current Electron desktop bridge via `873a87ff`.
- 2026-04-12: Re-ran the release-safety closure, including `npm audit`,
  `npm audit --omit=dev`, `bash launcher.sh --build-release`, and
  `bash launcher.sh --release-smoke`.

## Commit Cadence Notes

- Commit after each logical slice is complete and verified.
- Keep dependency upgrades, bridge contract renames, and documentation updates
  in separate atomic commits where the verification scope remains obvious.
- Re-check unpushed history for regression/fix pairs before every new commit.
- Include detailed commit bodies and `Agent: codex` footer on implementation
  commits produced during this plan.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | Reason: This remediation is tightly coupled across one workspace lockfile and one bridge contract. | Revisit trigger: A later implementation slice becomes independently parallelizable without overlapping writes. | N/A |

## Re-Plan Triggers

- A required dependency upgrade introduces packaging/config breakage that
  changes the planned sequencing.
- `npm audit` cannot be cleared without a broader workspace-tooling migration
  or an unacceptable compatibility break.
- The bridge rename uncovers runtime callers that still depend on the legacy
  `pywebview` surface beyond a temporary compatibility alias.
- Documentation updates reveal a larger architecture contract change that
  should be captured in an ADR rather than only this execution plan.

## Recommendations

- Prefer a canonical name such as `DesktopBridgeAPI` with a deprecated
  `type PyWebViewAPI = DesktopBridgeAPI` alias during the transition. This is
  facade-first, append-only, and keeps the migration mechanically simple.
- Prefer direct dependency upgrades and lockfile refresh before using npm
  `overrides`. This keeps the workspace easier to audit and reduces long-term
  maintenance burden.

## Completion Summary

### Completed

- Milestone 1: remediation boundary, compatibility posture, and documentation
  footprint captured in this plan before implementation started.
- Milestone 2: workspace dependency graph upgraded and lockfile refreshed until
  both `npm audit` variants reported zero vulnerabilities.
- Milestone 3: canonical bridge naming moved to `DesktopBridgeAPI` while
  preserving `PyWebViewAPI` and `window.pywebview.api` as deprecated
  compatibility aliases.
- Milestone 4: repo-facing docs aligned to the current Electron desktop flow
  and the release build plus bounded smoke path completed successfully.

### Deviations

- Added a direct root `jsdom` devDependency during Milestone 2.
- Reason: the refreshed workspace Vitest resolution needed `jsdom` available at
  the workspace root after the dependency graph changed.
- Revisit trigger: remove the extra root dependency if the workspace test
  runner is restructured to resolve environment packages per workspace without
  a root-level install.

### Follow-Ups

- Remove the deprecated `PyWebViewAPI`/`window.pywebview.api` compatibility
  alias once downstream callers no longer depend on it.
- Investigate the current Vite bundle-size warning emitted during
  `bash launcher.sh --build-release`.
- Review whether the root `jsdom` compatibility pin can be removed in a later
  workspace-tooling cleanup.

### Verification Summary

- `npm ci --include=optional`: passed during dependency remediation.
- `npm audit`: passed with 0 vulnerabilities after remediation and again at
  release-safety closure.
- `npm audit --omit=dev`: passed with 0 vulnerabilities after remediation and
  again at release-safety closure.
- `npm run -w frontend check:types`: passed after dependency remediation and
  after the bridge rename.
- `npm run -w frontend test:run`: passed after dependency remediation and after
  the bridge rename.
- `npm run -w electron validate`: passed after dependency remediation and after
  the bridge rename.
- `npm run test:launcher`: passed after dependency remediation and after the
  bridge rename.
- `bash launcher.sh --build-release`: passed at release-safety closure.
- `bash launcher.sh --release-smoke`: passed at release-safety closure.
- `git log --format='%h %s%n%b%n---' origin/main..HEAD`: reviewed before each
  post-plan implementation commit.

### Traceability Links

- Module README updated: `frontend/src/types/README.md`,
  `frontend/src/api/README.md`, `frontend/src/errors/README.md`,
  `frontend/src/components/README.md`, `electron/src/README.md`
- ADR added/updated: None
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: Not applicable in
  local branch execution state

## Brevity Note

Keep this plan focused on the dependency remediation and bridge terminology
cleanup. Expand detail only if a re-plan trigger or packaging risk requires it.
