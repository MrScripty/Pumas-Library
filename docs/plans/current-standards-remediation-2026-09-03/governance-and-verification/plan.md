# Plan: Current Standards Governance and Verification

**Plan status:** `Accepted`

**Current phase:** All governance milestones and objective acceptance claims
are accepted.

**Next slice:** `none`

**Acceptance status:** `satisfied`

**Execution ledger:** [execution-ledger.md](execution-ledger.md)

**Issues:** [issues.md](issues.md)

**Reports:** [Error-contract gate disposition](reports/error-contract-gate-disposition.md)
and [final governance evidence](reports/final-governance-evidence.md).

**Audit source:** Standards governance and verification (`docs/audits/current-standards-2026-09-03/governance-and-verification.md`, removed; see Git history at `0a4c0a8e`)

## Objective

Make Pumas contributor guidance and permanent verification machinery express
the current standards model, so a passing gate has a named claim, adequate
oracle, explicit schedule, and bounded meaning rather than count-based or
regex-based proxy authority.

## Baseline

- Audit code baseline: `a33c8c0efa7cd8783c7deeac9e608db205290d43`.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Audit standards baseline: `52b096ded9c53afd439a3cf0efc4cc85252da570`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- The documentation cleanup in `d84e2b35` already resolved G-01, the
  documentation portion of G-02, G-04, and G-05. Those changes are accepted
  prerequisites, not work to repeat.

## Objective Acceptance

| ID | Observable criterion | Kind | Environment | Mode | Status | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| GOV-A1 | Contributor and documentation entry points route through current Core and Router owners, and active plans have explicit lifecycle authority. | `focused` | `not-applicable` | `either` | `satisfied` | `d84e2b35`; [current guidance](../../../../CONTRIBUTING.md) |
| GOV-A2 | No blocking frontend or repository gate treats line, complexity, file, or directory counts as architecture/documentation acceptance. | `contract` | `representative` (Linux, pinned Node/pnpm) | `automated` | `satisfied` | Milestone 1 evidence in the [ledger](execution-ledger.md#2026-09-03--milestone-1-accepted-retire-count-based-blocking-gates) |
| GOV-A3 | The legacy regex error checker has an evidence-backed retain, replace, or remove disposition; any retained check has an adequate oracle and one schedule. | `focused` | `representative` (Linux, pinned Node/pnpm) | `either` | `satisfied` | [Disposition report](reports/error-contract-gate-disposition.md) and Milestone 2 evidence in the [ledger](execution-ledger.md#2026-09-03--milestone-2-accepted-remove-the-regex-error-checker) |
| GOV-A4 | Every retained permanent gate has one documented claim, proof boundary, oracle, overlap disposition, blocking authority, and schedule. | `contract` | `not-applicable` | `manual` | `satisfied` | [Permanent verification inventory](../../../DEVELOPMENT.md#permanent-verification-inventory) and Milestone 3 cross-review in the [ledger](execution-ledger.md#2026-09-03--milestone-3-accepted-publish-the-verification-inventory) |
| GOV-A5 | The affected contributor, frontend, CI, and launcher verification commands pass after obsolete mechanisms are removed. | `integration` | `representative` (Linux development environment) | `automated` | `satisfied` | [Final governance evidence](reports/final-governance-evidence.md) |

## Scope

### In Scope

- Contributor routing and active-plan discoverability.
- Frontend count/complexity and regex error-policy mechanisms.
- Package, pre-commit, and CI registrations for affected checks.
- A current claim-to-evidence inventory for retained permanent gates.
- Focused regression/static evidence for governance changes.

### Out Of Scope

- Fixing credential disclosure, RPC contracts, persistence, runtime lifecycle,
  release production, bindings, accessibility, or launcher behavior.
- Adding user-workflow, cross-platform runtime, or release tests owned by the
  other focused plans.
- Refactoring large files solely because a removed gate counted them.
- Rewriting terminal historical plans or audit evidence.

## Constraints And Assumptions

### Constraints

- Current standards are selected from `CORE-STANDARDS.md` through
  `STANDARDS-ROUTER.md`; removed legacy entry points have no authority.
- A gate is retained only if its marginal deciding value justifies its
  implementation, execution, and maintenance cost.
- Removing a weak checker does not waive any independently required type,
  contract, security, or runtime claim.
- CI edits in this plan precede later platform/release CI work to avoid shared
  ownership of `.github/workflows/build.yml`.

### Assumptions

- Current TypeScript typecheck, type-aware ESLint, Vitest, Electron tests,
  launcher tests, Rust checks, and manifest consistency checks retain useful
  claims; Milestone 3 must validate rather than inherit that assessment.
- The 31 regex reports are inventory candidates, not 31 established defects.
  Milestone 2 owns the bounded classification needed to decide the mechanism.

## Binding Decisions

| Decision | Owner | Evidence | Supersedes |
| --- | --- | --- | --- |
| `docs/DEVELOPMENT.md` owns the concise project verification inventory; executable scripts and CI own invocation details. | Governance plan | Current documentation workflow and small-doc-set decision in `d84e2b35` | Removed `docs/TESTING.md` and duplicated guides |
| Fixed line, complexity, file, and directory counts are diagnostic inputs only and cannot block on architectural/documentation claims. | Current standards Core, Architecture, and Documentation owners | G-02 and current standards | Legacy local count policy |
| The error checker is decided from classified reachable failures and oracle value, not repaired to preserve its existence. | Milestone 2 | G-03 and Verification workflow | Legacy `check:errors` assumption |
| Missing high-fidelity evidence remains an explicit gap owned by the applicable focused plan. | Program integration owner | G-06 and Verification workflow | Green-build equivalence assumption |

## Evidence And Oracle Plan

| Claim | Domain | Deciding oracle | Independent authority | Unsupported domain | Intended negative failure |
| --- | --- | --- | --- | --- | --- |
| GOV-A2 | Gate semantics | Executable config contains no blocking count rule or command; lint/type/tests still execute | Current routed Architecture and Documentation standards | Architectural quality itself | Reintroduced count gate is found during config review |
| GOV-A3 | Error-contract gate value | Classified candidate report plus targeted type/behavior evidence for any retained invariant | Receiver boundary contract and focused tests | General runtime correctness | A syntactic match without a contract failure cannot block |
| GOV-A4 | Verification ownership | Reviewed claim-to-evidence matrix agrees with package scripts, hooks, and CI | Executable configuration | Unimplemented future evidence | Unmapped or multiply scheduled gate remains pending |
| GOV-A5 | Affected tooling | Real command exit status against the changed workspace | Toolchain/configuration under test | Cross-platform and release behavior | Any removed-command reference or affected check failure blocks acceptance |

## Systemic Finding Audit

- **Invariant family and canonical owner:** permanent verification mechanisms
  are owned by a named acceptance claim and the Verification/Tooling workflows.
- **Bounded authority, representation, and reachable consumer population:**
  frontend ESLint and checker scripts, frontend/root package scripts,
  pre-commit configuration, the frontend CI job, contributor/development docs,
  and the four focused remediation plans that consume missing-evidence entries.
- **Expansion facts:** expand only if another blocking repository mechanism
  claims architectural/documentation/error correctness through the same count
  or regex authority.
- **Consumer dispositions:** obsolete mechanisms are removed; retained gates
  receive one matrix row; real defects discovered by candidate review are
  assigned to the focused plan that owns their boundary.
- **Alternatives considered:** deletion before replacement; consolidation into
  existing lint/type/test proof; stronger construction or focused behavior
  tests before new static machinery; advisory diagnostics only when no
  acceptance authority exists.
- **Evidence-backed stopping condition:** every selected mechanism and
  registration has one disposition, with no count proxy and no unclassified
  regex report remaining as blocking authority.
- **Repaired-composition comparison:** fewer independent scripts and no new
  generic gate framework; the existing package/hook/CI composition remains.

## Simplicity And Ownership Review

**Applicability:** `applicable`

- Independent concepts and dimensions: standards routing, durable
  documentation authority, gate semantics, execution scheduling, and evidence
  fidelity change independently and retain separate owners.
- State, identity, value, time, policy, and mechanism: these roles remain
  separately owned as follows.
  - Canonical authority scope and referenced authorities: routed standards own
    policy; executable config owns scheduling; `docs/DEVELOPMENT.md` projects
    current claims.
  - Version roles and owned promises: the planning standards commit fixes plan
    interpretation; tool/package versions continue to come from repository
    pins.
  - Supported compatibility overlaps and consumer matrix: local and CI
    invocations may overlap only when they provide useful feedback at their
    selected environments.
  - Material identity-invalidation effects: renaming/removing a command must
    update every package, hook, CI, and documentation consumer in the slice.
- Caller and composition-root knowledge: contributors select commands from
  root docs/package scripts; hooks and CI call only stable named commands and
  need not know checker internals.
- Representative change paths and forced owners: a claim change touches the
  matrix plus its mechanism/schedule; a schedule-only change touches executable
  configuration without redefining the claim.
- Stable Interfaces versus hidden knowledge: package-script names and CI
  result meanings are stable interfaces; source regexes and thresholds are not.
- Independent evolution, testing, failure, and replacement: each retained
  gate can run and fail independently, while the aggregate workflow reports
  rather than erases its outcome.
- Necessary complexity and containment: existing lint, type, test, hook,
  and CI mechanisms are sufficient; no registry or generator is admitted.
- Deletion and cumulative machinery result: delete count/error mechanisms
  that lack deciding value and their registrations/baselines; add only a
  concise matrix to the existing development guide.

## Milestones

### Milestone 0: Reconcile Current Authority

**Goal:** Establish current standards, documentation, and plan-lifecycle
authority without repeating already accepted cleanup.

**Allowed write set:**

- `README.md`
- `CONTRIBUTING.md`
- `RELEASING.md`
- `docs/**`
- subsystem `README.md` files changed by `d84e2b35`

**Tasks:**

- [x] Replace legacy standards routing and contradictory contract guidance.
- [x] Remove terminal plans, obsolete audits, stale snapshots, and blanket
  directory README policy.
- [x] Preserve the current audit and durable ADR.

**Acceptance gate:** Local Markdown links, Cargo metadata, shell syntax,
launcher help, diff checks, and commit hooks passed for `d84e2b35`.

**Status:** `Accepted`

### Milestone 1: Retire Count-Based Blocking Gates

**Goal:** Remove remaining count-based architecture proxies and all live
registrations while preserving useful lint, type, and test evidence.

**Allowed write set:**

- `frontend/eslint.config.js`
- `frontend/scripts/check-file-size.js`
- `frontend/scripts/file-size-baseline.json`
- `frontend/package.json`
- `frontend/README.md`
- `.github/workflows/build.yml`
- `docs/DEVELOPMENT.md`
- this plan, ledger, and issues files

**Tasks:**

- [x] Remove blocking `max-lines`, `max-lines-per-function`, and `complexity`
  rules whose thresholds claim architecture authority.
- [x] Remove `check:size`, its script/baseline, and CI/precommit registrations.
- [x] Verify no current command or guide still invokes the removed mechanism.
- [x] Run frontend lint, typecheck, and tests to prove retained gates still run.

**Acceptance gate:** GOV-A2 plus affected frontend static/test evidence.

**Status:** `Accepted`

### Milestone 2: Decide the Error-Contract Mechanism

**Goal:** Replace ambiguous regex authority with an evidence-backed mechanism
disposition and route any real defects to their owning boundary.

**Allowed write set:**

- `frontend/scripts/check-error-handling.js`
- `frontend/package.json`
- `frontend/eslint.config.js`
- `frontend/README.md`
- `.pre-commit-config.yaml`
- `.github/workflows/build.yml`
- `docs/DEVELOPMENT.md`
- `reports/error-contract-gate-disposition.md`
- this plan, ledger, and issues files

**Tasks:**

- [x] Classify every current regex report by reachable failure, contract owner,
  existing proof, and disposition.
- [x] Choose retain, replace, or remove for each intended invariant; prefer
  existing type-aware lint or focused behavior tests where adequate.
- [x] If a new implementation mechanism would be required, re-plan with its
  precise oracle and cost before adding it.
- [x] Give real defects outside this write set an owner in `issues.md` rather
  than silently broadening this milestone.

**Acceptance gate:** GOV-A3 and reviewed disposition report.

**Status:** `Accepted`

### Milestone 3: Publish the Claim-to-Evidence Inventory

**Goal:** Make the meaning and schedule of retained permanent gates explicit
and assign uncovered high-fidelity claims to their implementation plans.

**Allowed write set:**

- `docs/DEVELOPMENT.md`
- `CONTRIBUTING.md`
- `frontend/package.json`
- `electron/package.json`
- `package.json`
- `.pre-commit-config.yaml`
- `.pre-commit-hooks/check_hardcoded_colors.py`
- `.github/workflows/build.yml`
- this plan, ledger, and issues files

**Tasks:**

- [x] Inventory every retained hook, package check, and CI gate.
- [x] Record claim, boundary, oracle, overlap/marginal value, blocking authority,
  environment, and schedule in the existing development guide.
- [x] Reconcile duplicated, absent, or misleading registrations without adding
  missing system/release tests owned by other plans.
- [x] Link every pending high-fidelity claim to one focused plan owner.

**Acceptance gate:** GOV-A4 and configuration/documentation cross-review.

**Status:** `Accepted`

### Milestone 4: Governance Acceptance

**Goal:** Verify the resulting governance surface and hand off explicit
evidence gaps without claiming the broader remediation program is complete.

**Allowed write set:**

- `docs/DEVELOPMENT.md`
- this plan, ledger, issues, and reports

**Tasks:**

- [x] Run every affected retained command in its available Linux environment.
- [x] Verify removed commands and files have no live consumers.
- [x] Review the plan-level acceptance claims and link final evidence.
- [x] Record pending cross-platform, system, and release claims under their
  focused plan owners.

**Acceptance gate:** GOV-A1 through GOV-A5 are satisfied.

**Status:** `Accepted`

## Blockers

- `none`

## Re-Plan Triggers

- Candidate classification proves a real error-contract defect must be fixed
  before a checker can be safely removed.
- Another live count/regex mechanism expands the systemic population.
- A retained gate has no adequate oracle or cannot run in its claimed schedule.
- Shared CI changes from another focused plan land before Milestone 3.
- Proposed replacement machinery materially increases the admitted
  composition or propagates claim knowledge into unrelated callers.

## Final Acceptance

- Acceptance status: `satisfied`
- Deferred follow-ups: system, user-workflow, cross-platform, binding-host, and
  release-artifact evidence remain owned by their focused plans.
- Final status: `Accepted`
