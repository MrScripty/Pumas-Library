import { useState, useMemo } from 'react';
import { AppShell } from './components/AppShell';
import { buildAppShellPanels } from './components/AppShellPanels';
import {
  buildAppShellHeader,
  buildAppShellSidebar,
  buildManagedAppsState,
  buildModelManagerProps,
  getAppRunningState,
  getLauncherLatestVersion,
  getSelectedAppShellState,
  getSetupDisplayState,
} from './components/AppShellState';
import type { AppConfig } from './types/apps';
import { useStatus } from './hooks/useStatus';
import { useDiskSpace } from './hooks/useDiskSpace';
import { useComfyUIProcess } from './hooks/useComfyUIProcess';
import { useDependencyInstaller } from './hooks/useDependencyInstaller';
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
import { getLogger } from './utils/logger';

const logger = getLogger('App');

export default function App() {
  const [selectedAppId, setSelectedAppId] = useState<string | null>(
    __FEATURE_MULTI_APP__ ? null : 'comfyui'
  );

  const {
    status,
    systemResources,
    networkAvailable,
    modelLibraryLoaded,
    isCheckingDeps,
    refetch: refetchStatus
  } = useStatus();
  const { diskSpacePercent, fetchDiskSpace } = useDiskSpace();
  const { installDependencies: handleInstallDeps, isInstallingDeps } = useDependencyInstaller({
    refetchStatus,
  });
  const {
    checkLauncherUpdates,
    checkLauncherVersion,
    isCheckingLauncherUpdates,
    launcherUpdateAvailable,
    launcherUpdateState,
    openLauncherUpdate,
  } = useLauncherUpdates();
  const runningState = useMemo(() => getAppRunningState(status), [status]);
  const { launchError, launchLogPath, isStarting, isStopping, launchComfyUI, stopComfyUI, openLogPath } = useComfyUIProcess(runningState.comfyUIRunning);
  const { launchError: ollamaLaunchError, launchLogPath: ollamaLaunchLogPath, isStarting: ollamaIsStarting, isStopping: ollamaIsStopping, ...ollamaActions } =
    useOllamaProcess(runningState.ollamaRunning);
  const { launchError: torchLaunchError, launchLogPath: torchLaunchLogPath, isStarting: torchIsStarting, isStopping: torchIsStopping, ...torchActions } =
    useTorchProcess(runningState.torchRunning);
  const { modelGroups, scanModels, fetchModels } = useModels();
  const { activeDownload, activeDownloadCount } = useActiveModelDownload();
  const runtimeProfiles = useRuntimeProfiles();
  const servingStatus = useServingStatus();

  const {
    appVersions,
    comfyActiveVersion,
    comfyInstalledVersions,
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
    comfyui: {
      isStarting,
      isStopping,
      launchError,
      installedVersions: comfyInstalledVersions,
    },
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
    comfyInstalledVersions,
    isStarting,
    isStopping,
    llamaCppRuntimeState,
    llamaCppInstalledVersions,
    launchError,
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
  const { apps, deleteApp, reorderApps, addApp } = useManagedApps(managedAppsState);
  const appIds = useMemo(() => apps.map((app) => app.id), [apps]);
  const { getPanelState, setShowVersionManager } = useAppPanelState(appIds);
  const selectedAppShellState = useMemo(
    () => getSelectedAppShellState(apps, selectedAppId),
    [apps, selectedAppId]
  );
  const setupDisplayState = useMemo(
    () => getSetupDisplayState(status, selectedAppId),
    [status, selectedAppId]
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
    comfyUIRunning: runningState.comfyUIRunning,
    launchComfyUI,
    stopComfyUI,
    launchLogPath,
    openLogPath,
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
    activeVersion: comfyActiveVersion,
    checkLauncherVersion,
    fetchDiskSpace,
    refetchStatus,
  });

  const handleDeleteApp = (appId: string) => {
    if (appId === 'comfyui') {
      logger.warn('Attempt to delete ComfyUI app prevented', { appId });
      return;
    }
    logger.info('Deleting app', { appId });
    deleteApp(appId);
  };

  const handleReorderApps = (reorderedApps: AppConfig[]) => {
    reorderApps(reorderedApps);
  };

  const handleAddApp = (insertAtIndex: number) => {
    addApp(insertAtIndex);
  };

  const handleShowVersionManager = (show: boolean) => {
    if (!selectedAppId) {
      return;
    }
    setShowVersionManager(selectedAppId, show);
  };

  const modelManagerProps = buildModelManagerProps({
    modelGroups,
    starredModels,
    excludedModels,
    onToggleStar: handleToggleStar,
    onToggleLink: handleToggleLink,
    selectedAppId,
    onAddModels: scanModels,
    onOpenModelsRoot: openModelsRoot,
    onModelsImported: fetchModels,
    activeVersion: appVersions.activeVersion,
    onChooseExistingLibrary: chooseLibraryRoot,
    servingEndpoint: servingStatus.endpoint,
    servedModels: servingStatus.servedModels,
  });
  const panels = buildAppShellPanels({
    activeShortcutState: setupDisplayState.activeShortcutState,
    appDisplayName: selectedAppShellState.appDisplayName,
    appVersions,
    comfyUIRunning: runningState.comfyUIRunning,
    connectionUrl:
      selectedAppId === 'llama-cpp'
        ? llamaCppConnectionUrl
        : selectedAppShellState.connectionUrl,
    depsInstalled: setupDisplayState.depsInstalled,
    diskSpacePercent,
    displayStatus: setupDisplayState.displayStatus,
    isCheckingDeps,
    isInstallingDeps,
    isOllamaRunning: runningState.ollamaRunning,
    isSetupComplete: setupDisplayState.isSetupComplete,
    isTorchRunning: runningState.torchRunning,
    modelGroups,
    modelManagerProps,
    panelState,
    selectedAppId,
    onInstallDeps: handleInstallDeps,
    onShowVersionManager: handleShowVersionManager,
  });

  return (
    <AppShell
      importPaths={importPaths}
      showImportDialog={showImportDialog}
      showSidebar={__FEATURE_MULTI_APP__}
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
        onDeleteApp: handleDeleteApp,
        onReorderApps: handleReorderApps,
        onAddApp: handleAddApp,
      })}
      panels={panels}
    />
  );
}
