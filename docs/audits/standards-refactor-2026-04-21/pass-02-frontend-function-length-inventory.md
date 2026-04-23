# Pass 02 - Frontend Function Length Inventory

## Context
This inventory records the failed trial run for lowering
`max-lines-per-function` to 80 effective lines. The frontend now enforces a
coarse 275-line function ratchet after splitting the oversized
`AppIndicator.test.tsx` suite. The entries below are the next decomposition
queue before the threshold can be lowered to the 80-line target.

## Trial Rule
`max-lines-per-function: ["error", { "max": 80, "skipBlankLines": true, "skipComments": true }]`

## Findings
- `frontend/src/hooks/usePhysicsDrag.ts:30` - 271 effective lines.
- `frontend/src/components/MigrationReportsPanel.tsx:39` - 259 effective lines.
- `frontend/src/components/ModelMetadataModal.tsx:22` - 257 effective lines.
- `frontend/src/App.tsx:36` - 255 effective lines.
- `frontend/src/components/model-import/modelImportWorkflowHelpers.test.ts:8` - 255 effective lines.
- `frontend/src/components/ModelManager.tsx:52` - 234 effective lines.
- `frontend/src/components/model-import/useModelImportWorkflow.ts:34` - 230 effective lines.
- `frontend/src/components/AppSidebar.tsx:30` - 229 effective lines.
- `frontend/src/hooks/useModelDownloads.test.ts:36` - 226 effective lines.
- `frontend/src/hooks/useVersionFetching.ts:46` - 222 effective lines.
- `frontend/src/components/AppIndicator.tsx:19` - 221 effective lines.
- `frontend/src/components/AppIndicator.test.tsx:27` - 203 effective lines.
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx:35` - 215 effective lines.
- `frontend/src/hooks/useModelLibraryActions.ts:50` - 215 effective lines.
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx:35` - 212 effective lines.
- `frontend/src/components/MappingPreviewDialog.tsx:41` - 211 effective lines.
- `frontend/src/components/HuggingFaceAuthDialog.tsx:27` - 209 effective lines.
- `frontend/src/components/InstallDialog.tsx:58` - 209 effective lines.
- `frontend/src/components/VersionSelector.tsx:35` - 205 effective lines.
- `frontend/src/hooks/useRemoteModelSearchHydration.test.ts:46` - 197 effective lines.
- `frontend/src/hooks/useActiveModelDownload.test.ts:27` - 195 effective lines.
- `frontend/src/components/ProgressDetailsView.tsx:53` - 194 effective lines.
- `frontend/src/hooks/useModels.test.ts:83` - 190 effective lines.
- `frontend/src/hooks/useInstallationManager.ts:45` - 187 effective lines.
- `frontend/src/hooks/useModelLibraryActions.test.ts:108` - 187 effective lines.
- `frontend/src/components/ConflictResolutionDialog.tsx:39` - 185 effective lines.
- `frontend/src/components/app-panels/sections/ModelSelectorSection.tsx:43` - 184 effective lines.
- `frontend/src/components/model-import/ImportReviewStep.tsx:76` - 181 effective lines.
- `frontend/src/components/ModelMetadataModalContent.tsx:73` - 176 effective lines.
- `frontend/src/hooks/useModelDownloads.ts:25` - 166 effective lines.
- `frontend/src/components/ModelInferenceSettingsEditor.tsx:41` - 165 effective lines.
- `frontend/src/components/AppIcon.test.tsx:6` - 164 effective lines.
- `frontend/src/utils/dragAnimations.test.ts:4` - 163 effective lines.
- `frontend/src/hooks/useVersions.test.ts:83` - 162 effective lines.
- `frontend/src/hooks/useStatus.ts:22` - 161 effective lines.
- `frontend/src/components/Header.test.tsx:14` - 160 effective lines.
- `frontend/src/hooks/useModels.ts:36` - 159 effective lines.
- `frontend/src/hooks/useRemoteModelSearch.ts:36` - 158 effective lines.
- `frontend/src/components/MappingPreview.tsx:37` - 155 effective lines.
- `frontend/src/components/ComfyUIIcon.test.tsx:6` - 154 effective lines.
- `frontend/src/components/AppShellState.test.ts:70` - 153 effective lines.
- `frontend/src/hooks/useAvailableVersionState.ts:32` - 153 effective lines.
- `frontend/src/hooks/useVersionShortcutState.test.ts:31` - 152 effective lines.
- `frontend/src/components/app-panels/sections/TorchServerConfigSection.tsx:19` - 150 effective lines.
- `frontend/src/components/LinkHealthStatus.tsx:57` - 149 effective lines.
- `frontend/src/components/model-import/modelImportWorkflowHelpers.test.ts:9` - 146 effective lines.
- `frontend/src/hooks/useNetworkStatus.test.ts:28` - 142 effective lines.
- `frontend/src/components/ModelManagerUtils.test.ts:50` - 139 effective lines.
- `frontend/src/components/ModelMetadataGrid.tsx:78` - 134 effective lines.
- `frontend/src/hooks/usePhysicsDragPointerEvents.ts:31` - 133 effective lines.
- `frontend/src/components/ModelImportDialog.tsx:29` - 131 effective lines.
- `frontend/src/components/ModelSearchBar.tsx:29` - 131 effective lines.
- `frontend/src/components/model-import/useModelImportWorkflow.test.ts:103` - 128 effective lines.
- `frontend/src/hooks/useInstallationProgress.ts:31` - 128 effective lines.
- `frontend/src/components/InstallDialogContent.tsx:37` - 127 effective lines.
- `frontend/src/components/AppIndicator.tsx:95` - 125 effective lines.
- `frontend/src/hooks/useInstallationManager.test.ts:90` - 124 effective lines.
- `frontend/src/components/ConflictResolutionItem.tsx:63` - 123 effective lines.
- `frontend/src/components/LocalModelsList.test.tsx:41` - 122 effective lines.
- `frontend/src/components/LinkHealthDetails.tsx:19` - 121 effective lines.
- `frontend/src/hooks/useVersionShortcutState.ts:20` - 121 effective lines.
- `frontend/src/components/MigrationReportsPanel.test.tsx:43` - 118 effective lines.
- `frontend/src/components/model-import/ImportResultStep.tsx:19` - 118 effective lines.
- `frontend/src/components/RemoteModelDownloadMenu.tsx:26` - 115 effective lines.
- `frontend/src/components/model-import/modelImportMetadataSpecs.test.ts:8` - 113 effective lines.
- `frontend/src/hooks/useAvailableVersionState.test.ts:30` - 110 effective lines.
- `frontend/src/components/ConfirmationDialog.tsx:16` - 109 effective lines.
- `frontend/src/components/ModelImportDropZone.tsx:117` - 109 effective lines.
- `frontend/src/hooks/useInstallationProgress.test.ts:45` - 109 effective lines.
- `frontend/src/components/RemoteModelDownloadMenu.test.tsx:20` - 107 effective lines.
- `frontend/src/components/model-import/modelImportMetadataLookup.ts:24` - 105 effective lines.
- `frontend/src/components/RemoteModelSummary.tsx:71` - 104 effective lines.
- `frontend/src/components/ui/HoldToDeleteButton.tsx:20` - 103 effective lines.
- `frontend/src/hooks/usePhysicsDrag.test.tsx:113` - 102 effective lines.
- `frontend/src/components/InstallDialogFrame.tsx:13` - 100 effective lines.
- `frontend/src/components/StatusFooter.tsx:39` - 100 effective lines.
- `frontend/src/components/app-panels/VersionManagementPanel.tsx:18` - 99 effective lines.
- `frontend/src/hooks/useStatus.test.tsx:35` - 99 effective lines.
- `frontend/src/hooks/useVersionFetching.test.ts:83` - 99 effective lines.
- `frontend/src/hooks/useVersions.ts:62` - 98 effective lines.
- `frontend/src/components/model-import/modelImportMetadataSpecs.test.ts:26` - 97 effective lines.
- `frontend/src/hooks/useRemoteModelSearch.test.ts:42` - 97 effective lines.
- `frontend/src/components/app-panels/ComfyUIPanel.tsx:27` - 95 effective lines.
- `frontend/src/components/RemoteModelsList.tsx:35` - 95 effective lines.
- `frontend/src/components/RemoteModelListItemActions.tsx:137` - 93 effective lines.
- `frontend/src/components/SidebarAppIcon.tsx:114` - 93 effective lines.
- `frontend/src/components/LocalModelRow.tsx:32` - 92 effective lines.
- `frontend/src/hooks/physicsDragUtils.test.ts:56` - 92 effective lines.
- `frontend/src/hooks/useRemoteModelSearchHydration.test.ts:161` - 92 effective lines.
- `frontend/src/hooks/useAppProcessActions.ts:22` - 90 effective lines.
- `frontend/src/components/LocalModelsList.tsx:43` - 89 effective lines.
- `frontend/src/components/app-panels/GenericAppPanel.tsx:52` - 88 effective lines.
- `frontend/src/components/model-import/useShardedSetDetection.test.ts:55` - 88 effective lines.
- `frontend/src/components/VersionSelectorTrigger.tsx:37` - 88 effective lines.
- `frontend/src/hooks/useLauncherUpdates.ts:24` - 88 effective lines.
- `frontend/src/hooks/useInstallationAccess.ts:14` - 87 effective lines.
- `frontend/src/hooks/useModels.test.ts:207` - 87 effective lines.
- `frontend/src/components/VersionSelectorDropdown.tsx:20` - 86 effective lines.
- `frontend/src/hooks/useManagedProcess.ts:42` - 86 effective lines.
- `frontend/src/components/ModelNotesEditor.tsx:17` - 84 effective lines.
- `frontend/src/hooks/useInstallationAccess.test.ts:36` - 83 effective lines.
- `frontend/src/components/MigrationReportControls.tsx:20` - 81 effective lines.
- `frontend/src/components/model-import/modelImportMetadataLookup.test.ts:72` - 81 effective lines.
