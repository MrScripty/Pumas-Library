# Execution Ledger: Frontend and UI Standards Remediation

## 2026-09-06 — Standalone Backend And Link-Health Contract

Status: accepted for this bounded slice; FE-I10 is resolved.

The user prioritized API/UI contracts with the GUI as an optional consumer.
Three scoped agents implemented launcher selection, public backend projection,
and UI invocation ownership; root integrated the generator, preload, conformance,
documentation and composed verification. The existing registry now owns the
shared read, avoiding duplicate policy or a GUI-dependent backend service.

- `PUMAS_GUI=false` omits desktop build/dependency/test paths independently of
  inference plugins; direct Cargo and the RPC binary remain Node-independent.
  Run executes a selected existing artifact without implicit build. GUI-only
  release-smoke refuses headless use explicitly.
- Public `LinkRegistry::health`, the owning facade and local dispatch share the
  read-only implementation. The existing result signature and registry-wide,
  version-independent behavior are preserved. Orphan discovery and link cleanup
  semantics are not part of this slice.
- The canonical RPC projection validates success, status and wire-safe counts;
  generated immutable decoders replace the handwritten renderer response.
  Failed and superseded reads cannot leave an apparently current healthy report.
  Initial failure, refresh failure, retry, version replacement, same-scope
  supersession and unmount are covered by 12 component tests.
- Full affected Rust suites pass: 1,374 tests with defaults and 1,334 without
  defaults, with 22 existing ignored tests in each combined core/RPC run.
  The public registry tests and actual RPC handler exercise real filesystem
  reads, preserve files, and reject inspection failures without leaking paths.
  Strict all-target Clippy passes with all features and without defaults.
- Frontend: 545 tests, TypeScript and lint pass. Electron: 10 test files,
  compilation and lint pass. Launcher: 49 tests pass, including fake-executable
  GUI/plugin/debug/release delegation and exact argument/error propagation.
  Generator tests pass 6/6; freshness passes; actual producer/decoder conformance
  passes 6/6 and producer/bundled-preload/renderer conformance passes 7/7.
- Both production GUI builds pass. Isolated real Chromium runs with the actual
  bundled preload reject a contradictory producer report, display unavailable,
  and recover through native pointer retry in both GUI modes. Screenshots exposed
  low-contrast new-state text; explicit theme colors corrected it before final
  verification. Native keyboard delivery to the hidden fixture window did not
  execute; keyboard accessibility evidence is the component interaction test,
  not a claimed Chromium keyboard pass. Default build is restored.
- A real standalone binary served healthy and version-independent JSON-RPC
  reads and rejected unknown parameters over localhost using a fresh temporary
  library. No live app, model payload, user configuration or library was changed.
  The real `PUMAS_GUI=false PUMAS_INFERENCE_PLUGINS=false` debug build also passed,
  followed by launcher argument forwarding and the same standalone HTTP check
  against that plugin-disabled binary. All five canonical plan checks passed.

Evidence remains Linux-only and slice-specific. Full API migration, frontend
M4/M5, packaged platform verification and Pending cleanup replay remain open.

## Baseline

- Plan status: `Active` after the Rust-first source gate was released.
- Planning code baseline: `d84e2b3520ce3da3f39cc3df953301fa9d6d3d50`.
- Planning standards baseline: `7bf74bb5a8cb0ffccaff3ec86550051f900fb4bb`.
- Milestones 0 through 3 are accepted; FE-A3, FE-A4, and FE-A5 are satisfied.
  The selected desktop consumer checkpoint is accepted; M4 remains `Active`
  for remaining operations and complete claim evidence. XR-S1 remains
  `Verifying` for its full composed matrix. See the current checkpoint below;
  earlier dated entries preserve the evidence and rejected alternatives at
  their historical boundaries.

## 2026-09-05 — Download-to-catalog row association accepted

- The user-visible two-row regression failed before the fix (`shows one catalog
  row`: expected one, received two). Download progress now carries the exact
  destination-derived library ID through canonical list/status/push DTOs and
  the real preload. Recovery admission supplies the selected current catalog ID
  immediately. One uniquely associated activity overlays that catalog row;
  absent, cached-only or ambiguous associations remain separate.
- Review caught artifact-key association inheritance and activity collapse
  before the ambiguity guard. Both are corrected: distinct download IDs survive
  snapshot selection, optimistic admission and initial restore; same-ID updates
  retain exact controls. Repository/name/quant similarity grants no association.
- `pnpm --dir frontend test:run`: 113 files, 535 tests pass. Frontend types/lint,
  Electron build/tests (10), lint, producer/preload conformance (5), renderer
  conformance (5), and generated desktop-contract freshness pass. The renderer
  conformance uses actual Rust fixtures and bundled preload, including invalid
  pushed identity rejection and exact resume parameters.
- Both real Vite entry modes pass an isolated Electron 39.8.6/Chromium workflow:
  initial partial, recovery admission, paused push and reload each retain one
  catalog row without a separate activity label; resumed IPC names the exact
  producer download ID. A 50% partial is visible after push and restore. Native
  mouse input and captured pixels supplement DOM assertions. No renderer
  warnings/errors remained after correcting ancillary test fixtures.
- Temporary fixture window/profile and scripts are isolated under
  `/tmp/pumas-row-gui.L9RInV`; logs are `/tmp/pumas-row-gui{,-default}.log` and
  `/tmp/pumas-row-*.log`. No backend or user model files participate in that GUI
  test. The running operator app was not restarted. Permanent regression owners
  are the existing component/hook and producer/preload suites, not a new runner.
- Optimized backend and default frontend/Electron builds pass. This accepts
  only the admitted duplicate-row correction, not full M4/M5, packaged-platform
  claims, or the complete remediation program.

## Reports

- `reports/frontend-async-owner-inventory.md` — completed for M0-S1.
- `reports/renderer-harness-admission.md` — accepted for M1-S1.
- `reports/frontend-overlay-consumer-inventory.md` — accepted for M2-S1.
- `reports/launcher-root-recovery-consumer-evidence.md` — verifying XR-S1.
- `reports/renderer-contract-consumer-inventory.md` — pending Milestone 4.

## Slice Ledger

| Slice | State | Evidence | Notes |
| --- | --- | --- | --- |
| M0-S1 | `accepted` | [original evidence](#2026-09-03--m0-s1-installation-progress-owner-accepted) and [PRG-I12 repair](#2026-09-03--m3-s3a-status-reachability-terminal-retention-and-admission-order-accepted) | Requested-install polling now begins after backend admission; existing-install discovery remains immediate. |
| M1-S1 | `accepted` | [renderer-harness admission](reports/renderer-harness-admission.md) | Existing Electron/CDP selected; no dependency or permanent-tooling change admitted. |
| M2-S1 | `accepted` | [overlay consumer inventory](reports/frontend-overlay-consumer-inventory.md) | Bounded modal/popup population, non-members, owner policies, and two write-set discoveries recorded. |
| M2-S2 | `accepted` | [modal verification record](#2026-09-03--m2-s2-modal-module-and-consumer-migration-accepted) | Shared stack-aware lifecycle, six modal branch dispositions, page preservation, and old-hook deletion. |
| M2-S3 | `accepted` | [verification record](#2026-09-03--m2-s3-popover-module-and-consumer-migration-accepted-after-replan) | Consumer migration and shared cross-Module Escape arbitration accepted after red/green composition repair. |
| M2-S4 | `accepted` | [runtime record](#2026-09-03--m2-s4-representative-chromium-evidence-accepted) | Deciding Chromium accessibility/focus and outside-pointer evidence accepted. |
| M2-S2-F1 | `accepted` | [caller-test correction](#2026-09-03--m2-s2-f1-install-dialog-caller-test-corrected) | Program-approved test-only correction; no product-source change. |
| M3-S1 | `accepted` | [focused verification record](#2026-09-03--m3-s1-progress-and-terminal-outcome-semantics-accepted) | Named determinate progress and one atomic terminal outcome; caller-level M2/M3 set is green. |
| M3-S2 | `accepted` | [focused verification record](#2026-09-03--m3-s2-central-reduced-motion-policy-accepted) | One composition-root and CSS reduced-motion policy covers both variants. |
| M3-S3a | `accepted` | [repair evidence](#2026-09-03--m3-s3a-status-reachability-terminal-retention-and-admission-order-accepted) | Red→green manager/state/dialog public seams plus requested-install admission order accepted. |
| M3-S3b | `accepted` | [repair and runtime evidence](#2026-09-03--m3-s3b-popover-motion-and-terminal-semantics-accepted) | Exact Popover entry/exit repair, stale M2 caller correction, both real entries, full suite, and behavior docs accepted. |
| XR-S1 | `verifying` | [selected checkpoint](#2026-09-05--selected-desktop-consumer-checkpoint-accepted) and [historical recovery evidence](reports/launcher-root-recovery-consumer-evidence.md) | Source and Linux cold/warm GUI accepted; full startup/selection and two-entry first-visible matrix remains pending. |
| M4 selected consumers | `accepted` | [selected checkpoint](#2026-09-05--selected-desktop-consumer-checkpoint-accepted) | Catalog/FTS, ticket recovery, scoped display cache, and exact activity presentation; not all desktop consumers. |

## 2026-09-05 — Selected Desktop Consumer Checkpoint Accepted

- Reconciliation operation: `continue` this focused plan at Standards
  `1609c304`. Commit `2b081fba` integrates the selected desktop contracts and
  consumers; `2b9553a0` independently corrects backend artifact reclassification.
  This entry supersedes current provider/preload/uncommitted-source holds,
  not earlier dated observations or remaining milestone gates.
- Accepted boundary: generated Rust-owned catalog and FTS projections, exact
  ticket recovery, selected download responses, bundled sandbox preload,
  synchronous decoded startup bootstrap, and version-2 root-scoped display
  cache. Retired version-1 entries are discarded. Cached rows grant no model
  action or recovery authority; fresh related-model controls are retained.
  Scan/import records remain a distinct unmigrated contract.
- Activity presentation preserves unassociated download rows and their exact
  IDs; no repo/name/quant join is inferred. The deciding regression reproduced
  an inflated model count, then proved catalog-only counting and explicit
  download-activity status while retaining exact-ID resume controls.
- Accepted automated evidence: frontend 520 tests, types, lint, both builds;
  real-producer renderer conformance 2/2; producer-decoder conformance 5/5;
  generator 5/5; Electron 129 tests plus the separately enabled real sandbox
  preload oracle. The mandatory fixture-wrapper command is
  `pnpm --dir frontend run test:desktop-contract`; it uses actual Rust
  constructor output rather than consumer-authored authority.
- Deciding Linux/X11 built-app evidence: 83 catalog models, two labelled paused
  activities, nine partial badges with percentages, no Ready-to-finish state,
  no renderer errors, scrolling, and normal shutdown. Warm native window
  capture shows cached rows during refresh without stale controls. The
  collision correction retains 83 models and clean repeat startup.
- The [program verification record](../execution-ledger.md#2026-09-05--coordinated-desktop-contract-verification)
  owns the combined commands, results, private capture location, and data
  preservation evidence. The [collision record](../execution-ledger.md#2026-09-05--collision-correction-accepted-on-linux)
  owns the later backend-only correction; neither is a whole M4/XR-S1/M5
  acceptance claim.
- Remaining next frontend slice: classify and coordinate link-health,
  import-picker, and conversion consumers with canonical providers; preserve
  raw import records and extend existing generation rather than handwritten
  mirrors. Whole FE-A1, full cache degradation/retry runtime evidence, and
  complete startup/two-entry matrices stay pending. Windows/macOS runtime
  evidence is platform-owned and does not block independent Linux source work.

## 2026-09-03 — M0-S1 Installation Progress Owner Accepted

- Operation: continued the active focused plan after the program owner released
  frontend source work on RUST-A1 acceptance.
- Behavior: `useInstallationManager` is the sole installation synchronization
  Module. It self-schedules only after a request settles, serializes across
  superseded app/tag generations, ignores late disabled/unmounted completions,
  derives resumed work from release hints, and owns app-scoped cancellation.
- Deleted machinery: dialog-local desktop polling, caller-managed progress
  refresh, and the available-version installation-tag callback chain.
- User outcome: cancellation rejection now remains distinct from successful
  cancellation and is visible in the affected install row.
- Design review: the dialog no longer knows the desktop progress/cancellation
  mechanism. The manager Interface gained the necessary cancellation action
  while losing the scheduling action; serialization, generation, polling, and
  normalization remain hidden. Removing the manager would redistribute those
  policies across callers, so the Module passes the deletion test.
- Focused integration evidence: `npm run test:run --
  src/hooks/useInstallationManager.test.ts
  src/hooks/useInstallationProgress.test.ts src/hooks/useVersions.test.ts
  src/hooks/useVersionFetching.test.ts
  src/hooks/useAvailableVersionState.test.ts
  src/hooks/useSelectedAppVersions.test.ts
  src/components/InstallDialog.test.tsx` passed 35 tests in 7 files.
- Supporting gates: full `npm run check:types` passed; full `npm run lint`
  passed before the final two test additions, and focused ESLint over every M0
  source/test file passed after them; `git diff --check` passed.
- Independent corroboration: governance ran the then-current full frontend
  suite successfully (102 files, 446 tests) before the last two focused
  regression cases were added; it does not replace the deciding focused
  deferred-request/fake-clock oracle.
- Historical acceptance: FE-A3 and M0 were accepted on this evidence. PRG-I12
  subsequently superseded that claim by exposing a missing admission-order
  case; the accepted M3-S3a repair re-satisfied the claim.

## 2026-09-03 — M1-S1 Renderer Harness Admission Accepted

- Operation: completed the planned bounded experiment, accepted its report,
  revised the later exact harness write set, and explicitly started M2-S1 as a
  report-only consumer inventory slice.
- Subject: a production library-only Vite bundle and compiled production
  preload running in Electron 39.8.6 / Chromium 142 with isolated state and
  deterministic desktop-operation fixtures.
- Deciding surfaces: Chromium accessibility tree, actual DOM focus, browser
  input dispatch, and CDP media emulation.
- Reachable failure: the current import overlay exposed no dialog role/modal
  state, did not receive focus or dismiss on Escape, and did not restore focus.
- Runtime: build 8.29 seconds; corrected workflow 1.554 seconds; no surviving
  Electron process after exit.
- Decision: existing Electron/Vite/Node tooling plus one small custom runner;
  no dependency. See the [admission report](reports/renderer-harness-admission.md)
  for cleanup, limits, owner split, and comparison to Vitest/release smoke.
- Acceptance: the Milestone 1 stopping condition and gate are satisfied.

## 2026-09-03 — M2-S1 Overlay Consumer Inventory Accepted

- Operation: completed and accepted the report-only inventory slice; stopped
  before shared overlay source mutation pending program release.
- Population: six modal families (including page/dialog branches and nested
  confirmations), three popup families, and searched nearby non-members.
- Semantics: popups are named non-modal action dialogs, not listboxes or menus;
  feature content/actions remain outside the shared lifecycle Modules.
- Re-plan: added `model-serve/ModelServeDialogContent.tsx` so stale dialog-ref
  plumbing can be deleted with the old focus hook, and added the missing
  `RemoteModelListItemActions.test.tsx` opener-contract evidence file.
- Follow-up: in-flow link-health/report disclosures do not share overlay
  lifecycle and are routed to FE-I12 rather than widening M2.
- Evidence: [overlay consumer inventory](reports/frontend-overlay-consumer-inventory.md).

## 2026-09-03 — M2-S2 Modal Module And Consumer Migration Accepted

- Operation: implemented the admitted modal-only source slice, accepted its
  focused evidence, and stopped before Popover work.
- Module Interface: `ModalDialog` owns the portal, dialog/alertdialog state,
  initial focus, Tab containment, topmost Escape, backdrop policy,
  dismissal-disabled state, cleanup, and stack-aware restoration. Feature
  consumers retain their titles, content, actions, and backdrop choice.
- Nested behavior: the real install-frame plus confirmation test proves that
  Escape closes only the confirmation, restores its install-dialog opener,
  then closes the parent and restores the original page opener. The primitive
  also rewires restoration past a parent and opener removed with the nested
  hierarchy.
- Migrated consumers: confirmation, modal install branch, model metadata,
  modal serving branch, model import, and HuggingFace authentication. Install
  and serving page branches remain non-modal and no longer receive modal
  autofocus.
- Deleted machinery: `model-serve/useDialogFocusTrap.ts` and its stale
  `dialogRef` plumbing in `ModelServeDialogContent`.
- Focused tests: seven files and 27 tests passed, covering the Module and every
  migrated family. Focused ESLint passed for all changed M2-S2 files, full
  TypeScript checking passed, the deleted-hook/ref sentinel passed, and
  `git diff --check` passed.
- Unsupported here: Chromium acceptance remains M2-S4 after the separate
  popup slice; FE-A4 is not yet satisfied by jsdom evidence.

## 2026-09-03 — M2-S3 Popover Module And Consumer Migration Accepted After Re-Plan

- Operation: implemented the program-released popup-only source slice and
  stopped before the M2-S4 representative Chromium workflow.
- Module Interface: controlled `Popover` owns the named non-modal dialog,
  trigger `aria-controls`/expanded/has-popup relationship, focus entry and
  return, topmost Escape handling, pointer-outside dismissal, and lifecycle
  listener cleanup. Feature consumers retain domain actions and state.
- Semantics: the three mixed-action collections remain dialog popups. No
  menu, listbox, or selection abstraction was introduced.
- Migrated consumers: version actions focus the active actionable version;
  model filters focus the selected filter; remote download options use either
  the primary action or the queue-another action as the truthful opener while
  cancellation remains a direct action.
- Preserved behavior: version management/default actions remain separate;
  filter selection still closes the controlled popup; remote detail hydration
  occurs only on opening, and grouped/quant/all-file download behavior remains
  owned by `RemoteModelDownloadMenu`.
- Deleted machinery: version-selector document pointer listeners, container
  ref, local dropdown animation shell, toggle-only trigger contract, and the
  download trigger's misleading pressed-state attribute.
- Focused integration evidence: ten files and 27 tests passed across the
  `Popover` Interface, all three migrated families, remote list-item/list
  callers, and remote summary behavior. The smaller direct Interface and
  consumer set passed 17 tests in seven files.
- Supporting gates: focused ESLint passed over every M2-S3 source/test file;
  full `npm run check:types` passed; old-lifecycle deletion sentinels and
  `git diff --check` passed.
- Review contradiction: independent modal and popup document-capture listeners
  each know only their local stack. When a Popover is opened inside a modal,
  the older modal listener receives Escape first and stops propagation, closing
  the parent before the newer popup.
- Re-plan: add only `ui/OverlayEscapeStack.ts` as the private cross-Module
  arbitration owner. First demonstrate the contradiction with a composed
  `ModalDialog` + `Popover` test, then route both Modules through the shared
  topmost policy and prove two-stage close/restoration. No feature consumer or
  M2-S4 runtime scope is added.
- Red evidence: the composed regression failed one of four Popover tests before
  repair because the first Escape removed `Composed dialog` along with its
  child popup.
- Repair: `OverlayEscapeStack` owns one document Escape listener and dispatches
  only to the most recently registered modal or popup layer. Modal Tab/focus
  containment and popup pointer-outside behavior remain inside their respective
  Modules.
- Green evidence: both Module suites passed eight tests in two files, then the
  complete migrated modal/popup and caller set passed 55 tests in 17 files.
  Focused ESLint over the shared owner and both Modules/tests, full TypeScript
  checking, and `git diff --check` also passed.
- Review: the program accepted the shared private Escape layer as the correct
  cross-Module Seam; containment and pointer-outside policies remain local.
- Acceptance boundary: this record does not satisfy FE-A4. The separately
  released M2-S4 Chromium oracle remains.

## 2026-09-03 — M2-S4 Representative Chromium Evidence Accepted

- Operation: built the production library-only renderer and a temporary
  composition fixture containing the actual `ModalDialog` and `Popover`
  Modules, then ran both in Electron 39.8.6 / Chromium 142 through the real
  compiled preload. No repository source or permanent tooling changed.
- Production modal oracle: the real model-import workflow exposed a DOM and AX
  dialog named `Import Models` with modal state true. Focus entered the close
  action, an attempted background focus was contained, Tab from the last
  actionable element wrapped to the first, and Escape closed the dialog and
  restored `Import models`.
- Production popup oracle: the model-filter trigger reported expanded true,
  `aria-haspopup="dialog"`, and `aria-controls` exactly matching the named
  `Filter by category` popup id. The AX node was a non-modal dialog, focus
  entered `All Categories`, and Escape closed and restored the trigger.
- Outside-pointer observation: reopening the filter and clicking the search
  input closed the popup and left final browser focus on `Search 0 models`.
  This is the intended policy: lifecycle cleanup may restore the opener during
  unmount, then Chromium's pointer default transfers focus to the user's target.
  A subsequent Escape left that focus unchanged, corroborating listener cleanup.
- Cross-Module oracle: sequential opening matches the current production
  consumer invariant. The actual Modules exposed named modal/popup AX dialog
  nodes; first Escape closed only the popup and restored `Open runtime popup`,
  while second Escape closed the modal and restored `Open composed workflow`.
- Result: the clean Electron workflow exited 0 in 6.74 seconds after a 7.38
  second production build and 3.50 second fixture build. Ten deterministic IPC
  calls completed, and renderer console output contained one expected info log
  with no warning/error.
- Cleanup: the debugger detached, BrowserWindow was destroyed, no matching
  Electron process remained, and `/tmp/pumas-m2-acceptance` (bundles, fixture,
  diagnostics, and isolated profiles) was deleted.
- Review: the program accepted the named modal/popup AX state, focus lifecycle,
  sequential nested ordering, browser-native outside-pointer focus outcome,
  and cleanup evidence. Milestone 2 and FE-A4 are satisfied.

## 2026-09-03 — M2-S2-F1 Install Dialog Caller Test Corrected

- Operation: used the program-approved test-only follow-up in
  `InstallDialog.test.tsx`; no product source or M3 write set was broadened.
- Discovery: the M3-S1 caller-level suite reached a stale assertion for the
  pre-M2 `Dismiss install dialog` control. The accepted M2 focused evidence
  exercised the migrated `InstallDialogFrame` Interface and modal families but
  did not include this higher-level orchestration test, whose M0 assertions
  predated the frame migration.
- Correction: the caller test now drives the public M2 behavior through the
  dialog backdrop and document Escape path, matching the accepted Module and
  representative Chromium behavior.
- Evidence: the combined `ProgressDetailsView`, `InstallDialogContent`, and
  `InstallDialog` suite passed 10 tests in three files. Focused ESLint over the
  corrected caller test and M3-S1 files passed, as did full TypeScript checking.

## 2026-09-03 — M3-S1 Progress And Terminal Outcome Semantics Accepted

- Operation: implemented only the admitted progress-view source/test slice,
  then stopped before composition-root and CSS motion work.
- Behavior: overall and current-stage progress expose named, clamped
  determinate progressbar values. Terminal installation state is projected to
  exactly one outcome: failure is assertive, while cancellation and success
  are polite. Incremental progress remains outside the live region.
- Red evidence: before the source change, both focused tests failed because no
  named progressbar or terminal status/alert role was present.
- Green evidence: `ProgressDetailsView.test.tsx` passed two tests, including
  failure, cancellation, success, and an identical-success rerender that keeps
  one stable status node. The caller-level M2/M3 suite passed 10 tests in three
  files after the separately recorded test-only correction.
- Supporting gates: focused ESLint over the M3-S1 source/test and corrected
  caller test passed; full `npm run check:types` passed.
- Review: the program accepted the clamped stable progress values and the
  single atomic terminal region with outcome-appropriate politeness.
- Acceptance boundary: representative accessibility-tree announcements and
  normal/reduced motion remain M3-S3, so FE-A5 is not yet satisfied.

## 2026-09-03 — M3-S2 Central Reduced-Motion Policy Accepted

- Operation: changed only the admitted composition-root and CSS policy files;
  stopped before representative runtime and documentation work.
- Framer Motion policy: one `MotionConfig` at the renderer composition root
  uses `reducedMotion="user"`, so both default and library-only application
  trees consume the operating-system preference without teaching feature
  components about media queries.
- CSS policy: one `prefers-reduced-motion: reduce` rule makes animation and
  transition duration effectively immediate, prevents repeated animation, and
  disables smooth scrolling across elements and pseudo-elements. The short
  duration preserves lifecycle completion events while suppressing visible
  repeated motion.
- Focused gates: ESLint passed for the composition root and M3 status source;
  full TypeScript checking and the two ProgressDetailsView tests passed.
- Build evidence: default and library-only production Vite builds passed in
  6.19 and 5.97 seconds respectively.
- Review: the program accepted the two central policy boundaries and the
  near-zero-duration/one-iteration lifecycle behavior.
- Acceptance boundary: browser media emulation must still demonstrate the
  computed normal/reduced difference and terminal accessibility tree in M3-S3.

## 2026-09-03 — M3-S3 Runtime Admission Stopped For Unreachable Status View

- Operation: inspected the real default and library-only entry paths before
  creating a temporary harness; stopped without runtime or README mutation.
- Contradiction: `useInstallationState` initializes and resets `viewMode` to
  `list`, while the complete production source population contains no call
  that sets it to `details`. `ProgressDetailsView` therefore has no production
  entry transition even though direct component tests can render it.
- Terminal loss: `useInstallationManager` retains unsuccessful terminal
  progress but clears successful completion; `InstallDialog` clears its local
  presentation tag for any terminal payload. Adding a view transition alone
  would still leave success unreachable and terminal failure/cancellation too
  short-lived for the existing presentation timer to govern.
- Oracle consequence: a temporary component fixture would prove Chromium can
  render the component, not that a user of either real entry can receive the
  status. It is rejected as manufactured evidence.
- Mode classification: installation UI is intentionally compiled out of the
  library-only entry, so its progress/outcome portion is not applicable. The
  central reduced-motion policy remains applicable to both real entries.
- Re-plan request: keep ownership in the current manager/dialog/state chain,
  retain all terminal outcomes long enough for one deterministic presentation,
  and provide a real transition into that presentation. Exact source/tests
  were accepted as `useInstallationManager`, `useInstallationState`, and
  `InstallDialog` with their colocated tests. Their public output, presentation
  state, and rendered dialog are the confirmed TDD seams; only desktop
  operations and time may be mocked.

## 2026-09-03 — M3-S3a Status Reachability, Terminal Retention, And Admission Order Accepted

- Operation: implemented only the accepted `useInstallationManager`,
  `useInstallationState`, and `InstallDialog` source/test slice and stopped
  before Chromium or README work.
- Manager behavior: the current lifecycle now retains normalized success just
  as it retains failure/cancellation, while success still refreshes version
  state and exposes idle network status. The existing superseded-app test now
  supplies a late successful terminal payload and confirms it cannot mutate the
  new lifecycle.
- PRG-I12 counterevidence: integration review found `installVersion` could
  start its progress read before `install_version` acknowledged the new
  backend lifecycle. A fast read of a prior terminal payload could invalidate
  the new generation and strand the subsequently accepted install. This
  reopens the earlier FE-A3/M0 claim until the correction is reviewed.
- Admission-order red evidence: with `install_version` held by a controlled
  deferred Adapter, the public hook called `get_installation_progress` once
  before admission when the expected count was zero.
- Admission-order repair: requested-install polling starts only after a
  successful backend response. Existing-release discovery still begins
  polling immediately because that lifecycle already exists. The focused
  regression observes no pre-admission read, then exactly one current-lifecycle
  read after success.
- Presentation behavior: a new installation identity enters details once when
  progress first appears; Back remains on the list through later active updates;
  the active-to-terminal transition enters details once more, and a later Back
  remains respected. Identity consists only of the installation tag/start time
  and active/terminal transition, hidden inside the state owner.
- Dialog behavior: auto-presented progress exposes both determinate bars. On a
  terminal payload the local tag remains until the existing outcome timer
  expires, allowing the single terminal live region to include the correct
  version before the view returns to the list.
- Red evidence: the manager regression received `null` instead of successful
  terminal progress; the two state regressions received `list` instead of
  initial/terminal `details`; and the rendered dialog could not find the
  terminal `status` after completion.
- Green evidence: the manager/state/dialog seams and their direct progress and
  content callers passed 31 tests in six files. This includes the corrected
  admission-order regression and the existing maximum-one-request,
  supersession, terminal-retention, presentation, and outcome-timer cases.
- Supporting gates: focused ESLint over all M3/FE-I13/PRG-I12 source/tests and
  full TypeScript checking passed. Sequential default and library-only
  production Vite builds passed in 2.76 and 2.58 seconds. `git diff --check`
  passed.
- Independent review: root reran 21 direct tests plus focused lint, types, and
  diff checking, confirmed the repair preserved immediate discovery of an
  existing lifecycle, and accepted FE-A3/M0 and M3-S3a.
- Acceptance boundary: real-entry Chromium and README claims remain M3-S3b;
  FE-A5 is not yet satisfied.

## 2026-09-03 — M3-S3b Runtime Stopped For Visible Reduced-Motion Displacement

- Operation: built both production Vite modes into isolated `/tmp` directories
  and ran their real entries with the compiled production preload in Electron
  39.8.6 / Chromium 142. Media emulation was applied from a neutral page before
  the production bundle's first evaluation.
- Accepted sub-evidence, not an aggregate acceptance: the default entry exposed
  exactly two DOM and AX progressbars named `Overall installation progress`
  and `Downloading progress`, with ranges 0–100 and values 37 and 64. Its
  terminal success exposed exactly one DOM `status` with polite live behavior,
  atomic true, and the retained `v0.22.1` tag; the AX status reported live
  polite and atomic true.
- CSS comparison in both entries: normal mode exposed a 0.15-second transition
  and a 1.2-second/infinite scan animation. Reduced mode exposed 0.01-millisecond
  duration, zero delay, and one animation iteration. Each comparison had a
  positive DOM observation.
- Framer counterevidence: timer-based computed-style sampling recorded 40
  samples per scenario. Normal default mode recorded visible translated entry
  frames. Reduced default mode still recorded `translateY(-6px)` at nonzero
  opacity before snapping to rest, even when the preference was applied before
  module evaluation. The earlier hidden-window result with zero reduced
  samples was rejected as vacuous rather than accepted.
- Consequence: central CSS policy is proven, but FE-A5's Framer movement claim
  is false for the representative Popover. README mutation and M3 acceptance
  remain unavailable pending a bounded source/test write-set re-plan and a
  green four-scenario rerun.
- Accepted re-plan: change only shared `Popover.tsx` and its colocated test so
  the operating-system reduced-motion preference selects zero entry and exit
  translation while normal mode retains `-6px`; require non-vacuous open and
  dismiss sampling. Root also admitted `ModelMetadataModal.test.tsx` only to
  replace its stale deleted-backdrop-label query and await initial async work,
  because that unchanged full-suite caller failed after the accepted M2
  migration.
- Repository boundary: no package, permanent harness, README, or motion source
  changed during the experiment. Temporary bundles, fixture, and profile state
  remain under `/tmp/pumas-m3-s3b` until the deciding repair rerun, after which
  they must be removed. Renderer console output had no application/module
  errors; Electron emitted its development-only CSP warning when the unpackaged
  `file://` bundle was used.

## 2026-09-03 — M3-S3b Popover Motion And Terminal Semantics Accepted

- Operation: implemented the admitted Popover-only motion repair and the exact
  stale metadata-modal caller correction, rebuilt both production entries,
  reran the strengthened real Chromium oracle, updated only accepted behavior
  claims in `frontend/README.md`, and stopped before M4.
- Focused red→green: under the reduced preference, the Popover test first
  received entry and exit `y=-6` where zero was required. `Popover` now uses
  Framer's operating-system preference hook to select `y=0` for both reduced
  entry and dismissal while retaining opacity and normal-mode `y=-6`.
  Popover plus metadata-modal caller tests pass eight tests in two files.
- Stale caller: the metadata-modal test now awaits initial metadata state,
  drives the aria-hidden `data-modal-backdrop` through `mousedown`, and sends
  Escape to `document`. This matches the accepted ModalDialog Interface and
  removes the prior async act warning; no product source changed for FE-I16.
- Motion runtime: all four default/library-only × normal/reduced production
  entry scenarios ran in Electron 39.8.6 / Chromium 142 with media emulation
  established before bundle evaluation. Every entry scenario recorded 40
  positive open samples and 17–18 positive dismiss samples. Normal mode showed
  14–16 visible translated entry frames and 13–15 visible translated dismiss
  frames. Reduced mode showed zero translated entry or dismiss frames.
- CSS runtime: both entries reported 0.15-second transitions and
  1.2-second/infinite scan animation in normal mode; reduced mode reported
  0.01-millisecond duration, zero delay, and one iteration.
- Status runtime: the default entry exposed two DOM and AX progressbars named
  `Overall installation progress` and `Downloading progress`, with ranges
  0–100 and values 37 and 64. Terminal success exposed exactly one DOM status
  with polite/atomic semantics and the retained `v0.22.1` tag; Chromium AX
  reported live polite and atomic true. Installation is not applicable to the
  library-only entry.
- Clean run: the deciding renderer workflow exited zero in 9.6 seconds with no
  renderer application or module console warning/error. The expected
  unpackaged Electron CSP diagnostic was separated from application output.
- Verification: full frontend lint and TypeScript checking pass; the full
  frontend suite passes 109 files and 473 tests. Fresh production builds used
  by the deciding run passed in 3.20 seconds default and 2.41 seconds
  library-only. `git diff --check` passed after documentation reconciliation.
- Cleanup: the debugger detached and every BrowserWindow was destroyed; no
  matching Electron process remained. The exact `/tmp/pumas-m3-s3b` harness,
  bundles, and isolated profile state were removed after the deciding run.
- Acceptance: root accepted FE-A5, M3-S3b, Milestone 3, FE-I15, and FE-I16 on
  the non-vacuous bidirectional four-scenario evidence. No M4 source or
  permanent runner/package change started.

## 2026-09-03 — XR-S1 Launcher-Root Recovery Consumer Verifying

- Operation: consumed only the accepted launcher-root startup/selection
  Interface in the exact frontend write set and stopped before M4 catalog or
  permanent renderer-runner work.
- TDD progression: public provider tests first failed on the missing owner,
  premature Electron child mount, absent persisted/explicit outcomes,
  StrictMode duplicate reads, missing sequential polling cleanup, incorrect retry/back
  actions, unobserved invocation rejection, overlapping selection, focus, and
  legacy direct selection. Each vertical slice was made green before the next.
- Module result: one composition-root provider owns the immediate startup read,
  sequential initializing polls, selection single-flight, cancellation
  restoration, typed terminal projection, and late completion cleanup. Browser
  mode is not applicable and renders children; Electron children do not mount
  until the decoded state is ready.
- Presentation result: one lightweight view supplies an accessible named
  region, polite atomic status, deterministic heading focus, exact action
  availability, and existing frameless minimize/close controls. Expected typed
  selection outcomes do not enter the console-error path.
- Platform counterevidence: the first composed run found the production
  sandboxed preload could not load a new relative CommonJS decoder import. The
  platform owner corrected that producer boundary, removed the duplicate
  decoder authority, and independently passed its real OS-sandbox oracle and
  full 77-test Electron gate before refreezing.
- First-frame counterevidence: after the preload correction, the hardened
  default run recorded one checking frame before content (22.8 milliseconds),
  while library-only recorded zero. Program review admitted only a provider-
  local atomic terminal commit; no timing threshold, App, index, platform, or
  manifest expansion was accepted.
- No-bridge counterevidence and repair: a real Electron renderer with a missing
  preload first mounted valid-looking backend content. A public red test then
  drove the provider to distinguish Electron identity from intentional browser
  mode. The repaired real renderer showed only focused `Desktop bridge
  unavailable`, made no root-state request, hid unsupported Minimize, retained
  working Close, and mounted no model-list content.
- Rejected timing claim: a provider-local `flushSync` experiment then produced
  zero checking frames in one default and one library-only sample, but review
  correctly rejected this as construction proof because an async IPC reply can
  lose the first paint.
- Handshake red/green: five of 23 provider tests failed before the renderer had
  commit notification, timeout subscription, and terminal-latch behavior. The
  initial provider repair passed 24/24, derives the closed signal type from the existing
  startup projection, never acknowledges initializing, acknowledges ready,
  recovery, or unavailable once from a layout effect, removes renderer-owned
  deadline/attempt policy, and ignores a late reply after the main watchdog.
  Focused lint and full TypeScript checking passed at that boundary.
- Compositor counterevidence: a real delayed-default Electron 39.8.6 run used
  the compiled sandboxed preload and `beginFrameSubscription` NativeImage
  presentation callbacks. It captured hidden Checking, but the latest frame
  synchronously available when the production owner called `showWindow` was
  still Checking after the renderer's proposed two-rAF delay. The harness
  rejected that run, and a deliberately early acknowledgement was also
  rejected. Waiting for an invalidated next hidden frame produced no callback
  under the production preferences. Renderer timing therefore cannot own the
  actual-presentation decision.
- Corrected renderer boundary: 3 of 27 provider tests failed after replacing
  the disproved rAF contract with direct terminal layout-commit expectations.
  Removing the delay made all 27 pass while preserving one-shot StrictMode
  notification, timeout replacement of resolved or in-flight ready state, and
  suppression of late results after unmount. Main now owns the separate in-
  frame compositor challenge before native reveal.
- Frozen frontend gates: 34 tests across the provider, recovery view, and
  window-action seams pass; the full frontend suite passes 111 files and 504
  tests; lint and TypeScript checking pass; fresh default and library-only
  production builds pass in 3.40 and 2.48 seconds. Composed runtime evidence
  remains pending the corrected platform owner.
- Composed verification gate: after the platform handshake freezes, the real
  oracle must cover delayed ready in both modes, watchdog unavailable,
  recovery, missing preload, and acknowledgement failure without a hidden
  hang. It must also pass all nine startup and nine selection values through
  the canonical producer, compiled preload decoder, and frontend semantics,
  while malformed/extra payloads fail before presentation.
- Claim boundary: neither prior timing samples nor the rejected two-rAF repair
  decide FE-A8. M5 still owns the
  admitted reusable renderer runner, packaged-target execution remains with
  the platform plan, and XR-S1 remains `Verifying` pending the composed gate.
