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
`TorchModelSlotsSection.tsx`, `AppIndicator.tsx`, and `AppSidebar.tsx`. A
model-import batch cleared `modelImportMetadataLookup.ts`,
`useModelImportWorkflow.ts`, and `useShardedSetDetection.ts`. A final hook
batch cleared `useAppPanelState.ts`, `usePhysicsDrag.ts`,
`usePhysicsDragPointerEvents.ts`, `usePlugins.ts`, and
`useVersionShortcutState.ts`, allowing the frontend lint rule to be enforced.

## Remaining Findings
None. `@typescript-eslint/no-unnecessary-condition` is enforced in
`frontend/eslint.config.js`.
