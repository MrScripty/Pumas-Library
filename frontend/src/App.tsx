import { useState, useMemo } from 'react';
import { AppShell } from './components/AppShell';
import { ModelServeDialog } from './components/ModelServeDialog';
import type { ModelInfo } from './types/apps';
import { buildAppShellPanels } from './components/AppShellPanels';
import {
  buildAppShellHeader,
  buildAppShellSidebar,
  buildManagedAppsState,
  buildModelManagerProps,
  getAppRunningState,
  getLauncherLatestVersion,
  getSelectedAppShellState,
} from './components/AppShellState';
import { useStatus } from './hooks/useStatus';
import { useDiskSpace } from './hooks/useDiskSpace';
import { useOllamaProcess } from './hooks/useOllamaProcess';
import { useTorchProcess } from './hooks/useTorchProcess';
import { useModels } from './hooks/useModels';
import { useActiveModelDownload } from './hooks/useActiveModelDownload';
import { useAppImportDialog } from './hooks/useAppImportDialog';
import { useAppPanelState } from './hooks/useAppPanelState';
import { useAppProcessActions } from './hooks/useAppProcessActions';
import { useAppStartupChecks } from './hooks/useAppStartupChecks';
import { useAppWindowActions } from './hooks/useAppWindowActions';
import { useLauncherUpdates } from './hooks/useLauncherUpdates';
import { useLlamaCppRuntimeViewState } from './hooks/useLlamaCppRuntimeViewState';
import { useManagedApps } from './hooks/useManagedApps';
import { useModelPreferences } from './hooks/useModelPreferences';
import { useRuntimeProfiles } from './hooks/useRuntimeProfiles';
import { useSelectedAppVersions } from './hooks/useSelectedAppVersions';
import { useServingStatus } from './hooks/useServingStatus';
export default function InferencePluginsApp() {
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [servingModel, setServingModel] = useState<ModelInfo | null>(null);

  const {
    status,
    systemResources,
    networkAvailable,
    modelLibraryLoaded,
    refetch: refetchStatus
  } = useStatus();
  const { diskSpacePercent, fetchDiskSpace } = useDiskSpace();
  const {
    checkLauncherUpdates,
    checkLauncherVersion,
    isCheckingLauncherUpdates,
    launcherUpdateAvailable,
    launcherUpdateState,
    openLauncherUpdate,
  } = useLauncherUpdates();
  const runningState = useMemo(() => getAppRunningState(status), [status]);
  const { launchError: ollamaLaunchError, launchLogPath: ollamaLaunchLogPath, isStarting: ollamaIsStarting, isStopping: ollamaIsStopping, ...ollamaActions } =
    useOllamaProcess(runningState.ollamaRunning);
  const { launchError: torchLaunchError, launchLogPath: torchLaunchLogPath, isStarting: torchIsStarting, isStopping: torchIsStopping, ...torchActions } =
    useTorchProcess(runningState.torchRunning);
  const { modelGroups, libraryLoadStatus, scanModels, fetchModels } = useModels();
  const { activeDownload, activeDownloadCount } = useActiveModelDownload();
  const runtimeProfiles = useRuntimeProfiles();
  const servingStatus = useServingStatus();

  const {
    appVersions,
    installationProgress,
    llamaCppInstalledVersions,
    ollamaInstalledVersions,
    torchInstalledVersions,
  } = useSelectedAppVersions(selectedAppId);
  const { connectionUrl: llamaCppConnectionUrl, runtimeState: llamaCppRuntimeState } =
    useLlamaCppRuntimeViewState({
      profiles: runtimeProfiles.profiles,
      runtimeStatuses: runtimeProfiles.statuses,
      servedModels: servingStatus.servedModels,
      servingEndpoint: servingStatus.endpoint,
    });

  const managedAppsState = useMemo(() => buildManagedAppsState({
    running: runningState,
    status,
    systemResources,
    ollama: {
      isStarting: ollamaIsStarting,
      isStopping: ollamaIsStopping,
      launchError: ollamaLaunchError,
      installedVersions: ollamaInstalledVersions,
    },
    llamaCpp: {
      isRunning: llamaCppRuntimeState.isRunning,
      isStarting: llamaCppRuntimeState.isStarting,
      isStopping: llamaCppRuntimeState.isStopping,
      launchError: llamaCppRuntimeState.launchError,
      installedVersions: llamaCppInstalledVersions,
    },
    torch: {
      isStarting: torchIsStarting,
      isStopping: torchIsStopping,
      launchError: torchLaunchError,
      installedVersions: torchInstalledVersions,
    },
  }), [
    llamaCppRuntimeState,
    llamaCppInstalledVersions,
    ollamaInstalledVersions,
    ollamaIsStarting,
    ollamaIsStopping,
    ollamaLaunchError,
    runningState,
    status,
    systemResources,
    torchInstalledVersions,
    torchIsStarting,
    torchIsStopping,
    torchLaunchError,
  ]);
  const { apps } = useManagedApps(managedAppsState);
  const appIds = useMemo(() => apps.map((app) => app.id), [apps]);
  const { getPanelState, setShowVersionManager } = useAppPanelState(appIds);
  const selectedAppShellState = useMemo(
    () => getSelectedAppShellState(apps, selectedAppId),
    [apps, selectedAppId]
  );
  const panelState = getPanelState(selectedAppId);
  const {
    excludedModels,
    starredModels,
    toggleLink: handleToggleLink,
    toggleStar: handleToggleStar,
  } = useModelPreferences({ selectedAppId });
  const { closeWindow, minimizeWindow, openModelsRoot, chooseLibraryRoot } = useAppWindowActions();
  const {
    handleImportComplete,
    handleImportDialogClose,
    handlePathsDropped,
    importPaths,
    showImportDialog,
  } = useAppImportDialog({
    onImportComplete: fetchModels,
  });
  const { handleLaunchApp, handleOpenLog, handleStopApp } = useAppProcessActions({
    ollamaRunning: runningState.ollamaRunning,
    launchOllama: ollamaActions.launchOllama,
    stopOllama: ollamaActions.stopOllama,
    ollamaLaunchLogPath,
    openOllamaLogPath: ollamaActions.openLogPath,
    torchRunning: runningState.torchRunning,
    launchTorch: torchActions.launchTorch,
    stopTorch: torchActions.stopTorch,
    torchLaunchLogPath,
    openTorchLogPath: torchActions.openLogPath,
    refetchStatus,
  });

  useAppStartupChecks({
    activeVersion: appVersions.activeVersion,
    checkLauncherVersion,
    fetchDiskSpace,
    refetchStatus,
  });

  const handleShowVersionManager = (show: boolean) => {
    if (!selectedAppId) {
      return;
    }
    setShowVersionManager(selectedAppId, show);
  };

  const modelManagerProps = buildModelManagerProps({
    modelGroups,
    libraryLoadStatus,
    starredModels,
    excludedModels,
    onToggleStar: handleToggleStar,
    onToggleLink: handleToggleLink,
    selectedAppId,
    onAddModels: scanModels,
    onOpenModelsRoot: openModelsRoot,
    onServeModel: setServingModel,
    onModelsImported: fetchModels,
    activeVersion: appVersions.activeVersion,
    onChooseExistingLibrary: chooseLibraryRoot,
    servingEndpoint: servingStatus.endpoint,
    servedModels: servingStatus.servedModels,
    routerProfiles: servingStatus.routerProfiles,
    servingControlObservation: servingStatus.controlObservation,
  });
  const panels = buildAppShellPanels({
    appDisplayName: selectedAppShellState.appDisplayName,
    appVersions,
    connectionUrl:
      selectedAppId === 'llama-cpp'
        ? llamaCppConnectionUrl
        : selectedAppShellState.connectionUrl,
    diskSpacePercent,
    isOllamaRunning: runningState.ollamaRunning,
    isTorchRunning: runningState.torchRunning,
    modelGroups,
    modelManagerProps,
    panelState,
    selectedAppId,
    onShowVersionManager: handleShowVersionManager,
  });

  return (
    <>
      <AppShell
      importPaths={importPaths}
      showImportDialog={showImportDialog}
      showSidebar={true}
      onImportComplete={handleImportComplete}
      onImportDialogClose={handleImportDialogClose}
      onPathsDropped={handlePathsDropped}
      header={buildAppShellHeader({
        activeModelDownload: activeDownload,
        activeModelDownloadCount: activeDownloadCount,
        installationProgress,
        isCheckingLauncherUpdates,
        launcherLatestVersion: getLauncherLatestVersion(launcherUpdateState),
        launcherUpdateAvailable,
        modelLibraryLoaded,
        networkAvailable,
        status,
        systemResources,
        onCheckLauncherUpdates: checkLauncherUpdates,
        onClose: closeWindow,
        onDownloadLauncherUpdate: openLauncherUpdate,
        onMinimize: minimizeWindow,
      })}
      sidebar={buildAppShellSidebar({
        apps,
        selectedAppId,
        onSelectApp: setSelectedAppId,
        onLaunchApp: handleLaunchApp,
        onStopApp: handleStopApp,
        onOpenLog: handleOpenLog,
      })}
        panels={panels}
      />
      {servingModel && (
        <ModelServeDialog model={servingModel} onClose={() => setServingModel(null)} />
      )}
    </>
  );
}
