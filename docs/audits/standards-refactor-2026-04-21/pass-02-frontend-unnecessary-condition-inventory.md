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
and `errors/index.ts`. A model/version hook batch cleared
`useActiveModelDownload.ts`, `useAvailableVersionState.ts`,
`useInstallationProgress.ts`, `useModelDownloads.ts`,
`useModelLibraryActions.ts`, `useModels.ts`, and `useVersionFetching.ts`. A
UI component batch cleared `OllamaModelSection.tsx`,
`TorchModelSlotsSection.tsx`, `AppIndicator.tsx`, and `AppSidebar.tsx`.

## Remaining Findings
- `frontend/src/components/model-import/modelImportMetadataLookup.ts:84` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/model-import/useModelImportWorkflow.ts:248` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/model-import/useShardedSetDetection.ts:31` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/model-import/useShardedSetDetection.ts:51` - Unnecessary conditional, value is always falsy.
- `frontend/src/hooks/useAppPanelState.ts:23` - Unnecessary conditional, value is always falsy.
- `frontend/src/hooks/usePhysicsDrag.ts:185` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:97` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:151` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:58` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:206` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:39` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary optional chain on a non-nullish value.
