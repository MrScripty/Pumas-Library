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
| `ModelManagerIntegrityRefresh.test.tsx` | Acceptance coverage for backend-pushed model refresh clearing integrity warning UI through fresh backend-derived data. |
| `ModelManagerRemoteDownload.ts` | Remote Hugging Face download starter for payload shaping, backend result handling, error state, and auth escalation. |
| `ModelManagerRemoteDownload.test.ts` | Unit coverage for successful remote download starts, backend failures, and auth-required download errors. |
| `ModelManagerUtils.ts` | Pure model-manager helpers for download overlays, filtering, and remote-kind mapping. |
| `ModelManagerUtils.test.ts` | Unit coverage for model-manager helper behavior such as download overlay merging and remote-kind mapping. |
| `AppShell.tsx` | Rendering-only application shell for import overlays, header, optional sidebar, and app-panel container. |
| `AppShellPanels.ts` | Pure app-panel prop builder for shared version props and app-specific panel inputs. |
| `AppShellPanels.test.ts` | Unit coverage for shared and app-specific app-panel prop construction. |
| `AppShellState.ts` | Pure App shell state projection for running flags, setup display state, managed-app inputs, header props, sidebar props, and model-manager props. |
| `AppShellState.test.ts` | Unit coverage for App shell state projection and prop-builder behavior. |
| `HeaderControls.tsx` | Header update, status badge, and window-control button groups. |
| `HeaderResourceStrip.tsx` | Header CPU/GPU/RAM/VRAM metric derivation and compact resource strip rendering. |
| `HeaderStatus.ts` | Header status projection for install progress, model downloads, network, and library states. |
| `AppSidebar.tsx` | Sidebar drag/order state owner and app-selection toolbar container. |
| `SidebarAppIcon.tsx` | Motion-wrapped app icon renderer for drag previews, delete-zone overlays, and launch/stop/log controls. |
| `AppIcon.tsx`, `ComfyUIIcon.tsx`, `AppIndicator.tsx` | Launcher icon controls with sibling native buttons for app selection and launch/stop/status actions. |
| `ConfirmationDialog.tsx` | Accessible confirmation dialog for destructive or state-changing operator actions. |
| `ConfirmationDialog.test.tsx` | Interaction coverage for confirmation, cancellation, and Escape handling. |
| `InstallDialog.tsx` | Version installation dialog orchestration and async install controls. |
| `InstallDialog.test.tsx` | Dialog naming and dismissal coverage for modal install interactions. |
| `InstallDialogFrame.tsx` | Modal/page frame, header, backdrop, Escape handling, and focus restoration for install dialog content. |
| `InstallDialogFrame.test.tsx` | Rendering and dismissal coverage for install dialog modal and page frames. |
| `InstallDialogContent.tsx` | Presentational install-dialog body for banners, progress details, and version rows. |
| `InstallDialogContent.test.tsx` | Interaction coverage for install-dialog notices, detail-view routing, and version-row action wiring. |
| `ProgressDetailsView.tsx` | Detailed install-progress view for stage, speed, package, completed-item, and failure summaries. |
| `ProgressDetailsView.test.tsx` | Interaction coverage for progress details actions plus cancellation and success summary rendering. |
| `VersionListItem.tsx` | Presentational install-version row with install, uninstall, cancel, release-note, and log affordances. |
| `VersionListItemButton.tsx` | Install-version action button states for ready, install, uninstall, pending, progress, and cancel views. |
| `VersionListItemInfo.tsx` | Install-version title, release/log links, prerelease badge, date, and error presentation. |
| `VersionListItemState.ts` | Derived install-version display state for sizes, progress rings, package labels, and network indicators. |
| `VersionListItem.test.tsx` | Interaction coverage for installable, installed, and in-progress version row behaviors. |
| `ConflictResolutionDialog.tsx` | Accessible conflict-resolution modal for model mapping collisions. |
| `ConflictResolutionDialog.test.tsx` | Interaction coverage for default and bulk conflict-resolution choices in the mapping workflow. |
| `ConflictResolutionItem.tsx` | Presentational conflict row with resolution selector, path details, and per-action descriptions. |
| `ConflictResolutionItem.test.tsx` | Rendering and interaction coverage for conflict rows and expanded details. |
| `LocalModelsList.tsx` | Grouped local-library list with model actions, related-model disclosure wiring, and metadata-modal access. |
| `LocalModelsList.test.tsx` | Interaction coverage for local-library formatting, ctrl-click metadata access, and related-model actions. |
| `LocalModelGroupHeader.tsx` | Presentational category header and count for local model groups. |
| `LocalModelGroupHeader.test.tsx` | Rendering coverage for local model group labels and counts. |
| `LocalModelRow.tsx` | Local model row coordinator for metadata, relationship disclosure, and action controls. |
| `LocalModelRowActions.tsx` | Local model related/download/installed action switcher. |
| `LocalModelDownloadActions.tsx` | Active local download pause/resume/cancel progress controls. |
| `LocalModelInstalledActions.tsx` | Local model link, recover, convert, and delete controls. |
| `LocalModelRowState.ts` | Derived local model row state for link, download, related, partial, and conversion capabilities. |
| `LocalModelsEmptyState.tsx` | Empty local-library and no-match filter states for the local model list. |
| `LocalModelsEmptyState.test.tsx` | Rendering and action coverage for local model empty states. |
| `LocalModelMetadataSummary.tsx` | Presentational format, quant, size, dependency, and partial-error metadata for local model rows. |
| `LocalModelMetadataSummary.test.tsx` | Rendering coverage for local model metadata fallbacks, dependency labels, and partial errors. |
| `LocalModelNameButton.tsx` | Local model name control with metadata modifier-click behavior and row status badges. |
| `LocalModelNameButton.test.tsx` | Interaction and badge coverage for local model name controls. |
| `RelatedModelsPanel.tsx` | Presentational related-model expansion for loading, error, empty, and remote model rows. |
| `RelatedModelsPanel.test.tsx` | Rendering and URL-opening coverage for related-model panel states. |
| `ModelMetadataModal.tsx` | Modal for stored, embedded, inference, and notes metadata with semantic dialog/backdrop controls. |
| `ModelMetadataModal.test.tsx` | Dialog naming and dismissal coverage for metadata modal interactions. |
| `ModelMetadataModalFrame.tsx` | Metadata modal shell with header actions, backdrop dismissal, Escape handling, and focus restoration. |
| `ModelMetadataModalFrame.test.tsx` | Rendering, dismissal, and refetch disabled-state coverage for the metadata modal shell. |
| `ModelNotesMarkdownPreview.tsx` | Lightweight notes markdown renderer for headings, inline marks, lists, quotes, and code fences. |
| `ModelNotesMarkdownPreview.test.tsx` | Rendering coverage for supported notes markdown blocks and the empty-notes fallback. |
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
| `VersionSelectorTriggerControls.tsx` | Extracted trigger buttons and status-icon presentation for version install/open/default actions. |
| `VersionSelectorDropdown.tsx` | Presentational dropdown menu and row items for installed/selectable versions. |
| `VersionSelectorDropdown.test.tsx` | Interaction coverage for native version switch, default, and shortcut controls. |
| `ModelImportDialog.tsx` | Import flow for local and remote model files. |
| `MappingPreview.tsx` | Mapping preview and conflict-resolution workflow. |
| `MappingPreview.test.tsx` | Loading, expansion, apply, and retry coverage for the mapping preview workflow. |
| `MappingPreviewHeader.tsx` | Header, status badge, action-count, and expansion control for mapping previews. |
| `MappingPreviewState.ts` | Derived count, issue, and status helpers for mapping preview presentation state. |
| `MappingPreviewUnavailableState.tsx` | Error and retry state for unavailable mapping preview loads. |
| `MappingPreviewDetails.tsx` | Mapping preview details coordinator for counts, warnings, action sections, and apply controls. |
| `MappingPreviewDetailsFeedback.tsx` | Apply-result, refresh/apply control, and all-linked notice rendering for mapping preview details. |
| `MappingPreviewDetailsSections.tsx` | Cross-filesystem, summary, warning, and action-list sections for mapping preview details. |
| `MappingPreviewDetailsTypes.ts` | Shared presentation types for mapping preview detail subcomponents. |
| `MappingPreviewDetails.test.tsx` | Rendering and interaction coverage for mapping preview detail sections and controls. |
| `MappingPreviewDialog.tsx` | Accessible modal wrapper for previewing and applying model-library mappings. |
| `MappingPreviewDialog.test.tsx` | Dialog naming and dismissal coverage for mapping-preview modal interactions. |
| `RemoteModelListItem.tsx` | Presentational row for one Hugging Face search result and its download controls. |
| `RemoteModelListItemActions.tsx` | Remote model row action buttons, progress rings, and download-menu wiring. |
| `RemoteModelListItemState.ts` | Derived download flags, options, labels, retry hints, and selected-file totals for remote rows. |
| `RemoteModelListItem.test.tsx` | Interaction coverage for remote row URL, download-option, hydration, and cancellation actions. |
| `RemoteModelSummary.tsx` | Presentational remote model metadata summary for developer, kind, size, engine, auth, and retry details. |
| `RemoteModelSummary.test.tsx` | Rendering and interaction coverage for remote metadata, developer search, auth prompts, and retry hints. |
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
- Integrity warning labels in the model manager must clear only when refreshed
  backend model data no longer reports the issue metadata.

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
