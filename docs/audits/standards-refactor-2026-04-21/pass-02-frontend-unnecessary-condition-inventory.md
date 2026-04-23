# Pass 02 - Frontend Unnecessary Condition Inventory

## Context
This inventory records the remaining trial failures for enabling
`@typescript-eslint/no-unnecessary-condition` after the first low-risk cleanup
batch. Subsequent batches cleared `HeaderResourceStrip.tsx`,
`MappingPreviewState.ts`, `MigrationReportSummaries.tsx`, `libraryModels.ts`,
`LinkHealthDetails.tsx`, `LinkHealthStatus.tsx`, `MappingPreview.tsx`,
`MigrationReportsPanel.tsx`, `ImportMetadataDetailsState.ts`, and
`modelImportWorkflowHelpers.ts`. A version-management batch also cleared
`VersionListItemButton.tsx`, `VersionListItemState.ts`,
`VersionSelectorDefaultButton.tsx`, and `VersionSelectorState.ts`. A hook-helper
batch cleared `installationProgressTracking.ts`, `modelDownloadState.ts`,
`useInstallDialogLinks.ts`, `useNetworkStatus.ts`, and `useStatus.ts`. A
component/modal batch cleared `HuggingFaceAuthDialog.tsx`,
`InstallDialog.tsx`, `InstallDialogFrame.tsx`, `MappingPreviewDialog.tsx`,
`ModelMetadataGrid.tsx`, `ModelMetadataModal.tsx`, `ProgressDetailsView.tsx`,
and `errors/index.ts`.

## Remaining Findings
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx:95` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx:98` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx:89` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx:92` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/AppIndicator.tsx:196` - Unnecessary conditional, comparison is always true, since `"uninstalled" === "uninstalled"` is true.
- `frontend/src/components/AppSidebar.tsx:264` - Unnecessary conditional, the types have no overlap.
- `frontend/src/components/AppSidebar.tsx:268` - Unnecessary conditional, the types have no overlap.
- `frontend/src/components/model-import/modelImportMetadataLookup.ts:84` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/model-import/useModelImportWorkflow.ts:248` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/model-import/useShardedSetDetection.ts:31` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/model-import/useShardedSetDetection.ts:51` - Unnecessary conditional, value is always falsy.
- `frontend/src/hooks/useActiveModelDownload.ts:53` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useActiveModelDownload.ts:101` - Unnecessary conditional, the types have no overlap.
- `frontend/src/hooks/useAppPanelState.ts:23` - Unnecessary conditional, value is always falsy.
- `frontend/src/hooks/useAvailableVersionState.ts:75` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useAvailableVersionState.ts:79` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useInstallationProgress.ts:58` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useModelDownloads.ts:46` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModelDownloads.ts:68` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModelLibraryActions.ts:104` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useModels.ts:55` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModels.ts:184` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePhysicsDrag.ts:185` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:97` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:151` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:58` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:206` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useVersionFetching.ts:94` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:39` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary optional chain on a non-nullish value.
