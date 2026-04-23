# Pass 02 - Frontend Unnecessary Condition Inventory

## Context
This inventory records the remaining trial failures for enabling
`@typescript-eslint/no-unnecessary-condition` after the first low-risk cleanup
batch. The batch cleared `HeaderResourceStrip.tsx`, `MappingPreviewState.ts`,
`MigrationReportSummaries.tsx`, and `libraryModels.ts`.

## Remaining Findings
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx:95` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx:98` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx:89` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx:92` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/AppIndicator.tsx:196` - Unnecessary conditional, comparison is always true, since `"uninstalled" === "uninstalled"` is true.
- `frontend/src/components/AppSidebar.tsx:264` - Unnecessary conditional, the types have no overlap.
- `frontend/src/components/AppSidebar.tsx:268` - Unnecessary conditional, the types have no overlap.
- `frontend/src/components/HuggingFaceAuthDialog.tsx:115` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/InstallDialog.tsx:147` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/InstallDialogFrame.tsx:96` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/LinkHealthDetails.tsx:44` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/LinkHealthDetails.tsx:50` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/LinkHealthDetails.tsx:63` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/LinkHealthDetails.tsx:87` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/LinkHealthStatus.tsx:144` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/LinkHealthStatus.tsx:146` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/LinkHealthStatus.tsx:147` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/MappingPreview.tsx:74` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/MappingPreview.tsx:86` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/MappingPreviewDialog.tsx:136` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/MigrationReportsPanel.tsx:66` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/model-import/ImportMetadataDetailsState.ts:41` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/model-import/modelImportMetadataLookup.ts:84` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/model-import/modelImportWorkflowHelpers.ts:56` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/model-import/useModelImportWorkflow.ts:248` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/model-import/useShardedSetDetection.ts:31` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/model-import/useShardedSetDetection.ts:51` - Unnecessary conditional, value is always falsy.
- `frontend/src/components/ModelMetadataGrid.tsx:72` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/ModelMetadataModal.tsx:98` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/ModelMetadataModal.tsx:228` - Unnecessary conditional, value is always falsy.
- `frontend/src/components/ModelMetadataModal.tsx:228` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/ModelMetadataModal.tsx:231` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/ProgressDetailsView.tsx:61` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/VersionListItemButton.tsx:193` - Unnecessary conditional, value is always truthy.
- `frontend/src/components/VersionListItemState.ts:40` - Unnecessary conditional, the types have no overlap.
- `frontend/src/components/VersionListItemState.ts:40` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/VersionListItemState.ts:63` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/components/VersionListItemState.ts:110` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/VersionSelectorDefaultButton.tsx:20` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/VersionSelectorDefaultButton.tsx:20` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/components/VersionSelectorState.ts:55` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/errors/index.ts:15` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/installationProgressTracking.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/modelDownloadState.ts:65` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/modelDownloadState.ts:66` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/modelDownloadState.ts:77` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/modelDownloadState.ts:78` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useActiveModelDownload.ts:53` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useActiveModelDownload.ts:101` - Unnecessary conditional, the types have no overlap.
- `frontend/src/hooks/useAppPanelState.ts:23` - Unnecessary conditional, value is always falsy.
- `frontend/src/hooks/useAvailableVersionState.ts:75` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useAvailableVersionState.ts:79` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useInstallationProgress.ts:58` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useInstallDialogLinks.ts:39` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useModelDownloads.ts:46` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModelDownloads.ts:68` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModelLibraryActions.ts:104` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useModels.ts:55` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useModels.ts:184` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useNetworkStatus.ts:62` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useNetworkStatus.ts:69` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useNetworkStatus.ts:72` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useNetworkStatus.ts:73` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useNetworkStatus.ts:75` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/usePhysicsDrag.ts:185` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:97` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:151` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:58` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/usePlugins.ts:206` - Unnecessary conditional, expected left-hand side of `??` operator to be possibly null or undefined.
- `frontend/src/hooks/useStatus.ts:49` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useStatus.ts:60` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useStatus.ts:75` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionFetching.ts:94` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:38` - Unnecessary optional chain on a non-nullish value.
- `frontend/src/hooks/useVersionShortcutState.ts:39` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary conditional, value is always truthy.
- `frontend/src/hooks/useVersionShortcutState.ts:96` - Unnecessary optional chain on a non-nullish value.
