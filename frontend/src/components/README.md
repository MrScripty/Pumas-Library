# frontend components

## Purpose
UI components for dashboards, dialogs, status displays, app panels, and
model-management workflows. This directory holds presentation and thin
interaction layers over backend-owned state exposed through the preload/API
bridge.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `ModelManager.tsx` | Main model management screen and interactions. |
| `ModelManagerUtils.ts` | Pure model-manager helpers for download overlays, filtering, and remote-kind mapping. |
| `ModelManagerUtils.test.ts` | Unit coverage for model-manager helper behavior such as download overlay merging and remote-kind mapping. |
| `AppIcon.tsx`, `ComfyUIIcon.tsx`, `AppIndicator.tsx` | Launcher icon controls with sibling native buttons for app selection and launch/stop/status actions. |
| `ConfirmationDialog.tsx` | Accessible confirmation dialog for destructive or state-changing operator actions. |
| `ConfirmationDialog.test.tsx` | Interaction coverage for confirmation, cancellation, and Escape handling. |
| `InstallDialog.tsx` | Version installation dialog orchestration and async install controls. |
| `InstallDialogContent.tsx` | Presentational install-dialog body for banners, progress details, and version rows. |
| `InstallDialogContent.test.tsx` | Interaction coverage for install-dialog notices, detail-view routing, and version-row action wiring. |
| `ProgressDetailsView.tsx` | Detailed install-progress view for stage, speed, package, completed-item, and failure summaries. |
| `ProgressDetailsView.test.tsx` | Interaction coverage for progress details actions plus cancellation and success summary rendering. |
| `VersionListItem.tsx` | Presentational install-version row with install, uninstall, cancel, release-note, and log affordances. |
| `VersionListItem.test.tsx` | Interaction coverage for installable, installed, and in-progress version row behaviors. |
| `ConflictResolutionDialog.tsx` | Accessible conflict-resolution modal for model mapping collisions. |
| `ConflictResolutionDialog.test.tsx` | Interaction coverage for default and bulk conflict-resolution choices in the mapping workflow. |
| `LocalModelsList.tsx` | Grouped local-library list with model actions, related-model disclosure, and metadata-modal access. |
| `LocalModelsList.test.tsx` | Interaction coverage for local-library formatting, ctrl-click metadata access, and related-model actions. |
| `ModelMetadataModal.tsx` | Modal for stored, embedded, inference, and notes metadata with semantic dialog/backdrop controls. |
| `ModelMetadataModal.test.tsx` | Dialog naming and dismissal coverage for metadata modal interactions. |
| `MigrationReportsPanel.tsx` | Displays migration dry-run and execution artifacts and dispatches migration actions. |
| `MigrationReportsPanel.test.tsx` | Integration coverage for migration report loading, action dispatch, validation, and operator feedback. |
| `MigrationReportControls.tsx` | Migration action buttons, prune controls, and flash-message presentation for the report panel. |
| `MigrationReportControls.test.tsx` | Interaction coverage for migration control actions, busy-state disabling, and message rendering. |
| `MigrationReportSummaries.tsx` | Summaries for the latest dry-run and execution reports, including integrity status details. |
| `MigrationReportSummaries.test.tsx` | Rendering coverage for dry-run counts, execution summaries, and integrity error disclosure. |
| `MigrationReportArtifactList.tsx` | Report artifact rows with open and delete affordances for JSON and Markdown outputs. |
| `MigrationReportArtifactList.test.tsx` | Interaction coverage for empty state, artifact actions, busy labels, and path shortening. |
| `ModelKindIcon.tsx` | Renders model/task-kind tokens into consistent icons and labels. |
| `VersionSelector.tsx` | Version install/switch/update UI. |
| `VersionSelectorTrigger.tsx` | Presentational trigger shell for the active-version selector and action buttons. |
| `VersionSelectorDropdown.tsx` | Presentational dropdown menu and row items for installed/selectable versions. |
| `ModelImportDialog.tsx` | Import flow for local and remote model files. |
| `MappingPreview.tsx` | Mapping preview and conflict-resolution workflow. |
| `MappingPreviewDialog.tsx` | Accessible modal wrapper for previewing and applying model-library mappings. |
| `MappingPreviewDialog.test.tsx` | Dialog naming and dismissal coverage for mapping-preview modal interactions. |
| `RemoteModelListItem.tsx` | Presentational row for one Hugging Face search result and its download controls. |
| `RemoteModelDownloadMenu.tsx` | Extracted remote-download option menu for grouped file and quantized model selections. |
| `RemoteModelDownloadMenu.test.tsx` | Interaction coverage for grouped-file and quantized remote download menu actions. |
| `ui/` | Small reusable primitives (buttons, tooltips, list items). |
| `app-panels/` | App-specific panel renderers and sections. |

## Problem
Render backend-owned launcher, model-library, and migration state in a way that
is interactive for operators without allowing React components to become a
second source of truth for business state.

## Constraints
- Backend-owned data must remain authoritative.
- Components must coordinate with the existing preload/API boundary rather than
  talking to infrastructure directly.
- Desktop drag/drop surfaces should prefer the Electron preload bridge for path
  resolution and only fall back to URI-list parsing for compatibility with
  platform file managers.
- Large screens such as `ModelManager.tsx` still need decomposition reviews when
  they accumulate multiple responsibilities.
- Migration and import workflows must reflect real backend state and not use
  optimistic updates for persisted library data.

## Decision
- Keep feature-level composition in high-level components; primitives stay in
  `ui/`.
- Keep long-running async interactions in hooks, API wrappers, or backend
  methods rather than burying lifecycle ownership in leaf components.
- Preserve additive interpretation components such as `ModelKindIcon.tsx` so
  backend classification improvements can be surfaced without rewriting the
  full page component.

## Alternatives Rejected
- Store authoritative model-library workflow state inside React components:
  rejected because it would violate backend-owned data rules and create drift.
- Collapse all model-management UI into generic primitives only: rejected
  because page-level orchestration still needs feature-aware components.

## Invariants
- Components display backend-owned library and migration state rather than
  inventing alternate business state locally.
- Operator actions go through explicit backend/API calls.
- Cross-platform file import components must keep the Electron bridge as the
  primary path source and treat URI-list parsing as a compatibility fallback.
- Presentation-only state such as expansion, hover, or modal visibility may be
  local, but persisted model-library truth is not.

## Revisit Triggers
- `ModelManager.tsx` grows enough that helper extraction is no longer adequate.
- A new frontend state-management boundary is introduced for backend-owned data.
- Migration, import, and library status surfaces diverge enough to require a
  dedicated feature subdirectory split.

## Dependencies
**Internal:** hooks, shared types, API wrappers, config.
**External:** React and styling utilities.

## Related ADRs
- None identified as of 2026-04-10.
- Reason: frontend component structure is currently governed by README guidance
  and implementation plans rather than formal ADRs.
- Revisit trigger: a new frontend architecture or state-management decision
  becomes cross-team or long-lived enough to require an ADR.

## Usage Examples
```tsx
<ModelManager />
```

## API Consumer Contract
- Components in this directory consume typed frontend API wrappers and shared UI
  model types.
- They must treat returned backend state as authoritative.
- Errors should be rendered as operator feedback and not silently converted into
  local fallback state that changes business meaning.
- Compatibility expectation is internal-to-repo but additive: component props
  and shared usage patterns should evolve without hidden breaking semantics for
  sibling callers.

## Structured Producer Contract
- Most components in this directory do not produce persisted structured
  artifacts.
- `ModelKindIcon.tsx` and migration/report views do produce user-visible
  interpretations of backend enums and labels, so label/icon mapping changes
  must stay aligned with backend semantics.
- Revisit trigger: a component starts generating machine-consumed JSON, config,
  or persisted view state.
