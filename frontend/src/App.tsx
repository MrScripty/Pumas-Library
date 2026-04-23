import { useState, useMemo } from 'react';
import { AppShell } from './components/AppShell';
import { buildAppShellPanels } from './components/AppShellPanels';
import type { ModelManagerProps } from './components/ModelManager';
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
import { useManagedApps } from './hooks/useManagedApps';
import { useModelPreferences } from './hooks/useModelPreferences';
import { useSelectedAppVersions } from './hooks/useSelectedAppVersions';
import { getLogger } from './utils/logger';

const logger = getLogger('App');

export default function App() {
  const [selectedAppId, setSelectedAppId] = useState<string | null>(
    __FEATURE_MULTI_APP__ ? null : 'comfyui'
  );

  // --- Custom Hooks ---
  const {
    status,
    systemResources,
    networkAvailable,
    modelLibraryLoaded,
    isCheckingDeps,
    refetch: refetchStatus
  } = useStatus();
  const { diskSpacePercent, fetchDiskSpace } = useDiskSpace();
  const {
    installDependencies: handleInstallDeps,
    isInstallingDeps,
  } = useDependencyInstaller({
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
  const comfyUIRunning = status?.comfyui_running || false;
  const ollamaRunning = status?.ollama_running || false;
  const torchRunning = status?.torch_running || false;
  const { launchError, launchLogPath, isStarting, isStopping, launchComfyUI, stopComfyUI, openLogPath } = useComfyUIProcess(comfyUIRunning);
  const {
    launchError: ollamaLaunchError,
    launchLogPath: ollamaLaunchLogPath,
    isStarting: ollamaIsStarting,
    isStopping: ollamaIsStopping,
    launchOllama,
    stopOllama,
    openLogPath: openOllamaLogPath
  } = useOllamaProcess(ollamaRunning);
  const {
    launchError: torchLaunchError,
    launchLogPath: torchLaunchLogPath,
    isStarting: torchIsStarting,
    isStopping: torchIsStopping,
    launchTorch,
    stopTorch,
    openLogPath: openTorchLogPath
  } = useTorchProcess(torchRunning);
  const { modelGroups, scanModels, fetchModels } = useModels();
  const { activeDownload, activeDownloadCount } = useActiveModelDownload();

  const {
    appVersions,
    comfyActiveVersion,
    comfyInstalledVersions,
    installationProgress,
    ollamaInstalledVersions,
    torchInstalledVersions,
  } = useSelectedAppVersions(selectedAppId);

  const managedAppsState = useMemo(() => ({
    systemResources,
    comfyui: {
      isRunning: comfyUIRunning,
      isStarting,
      isStopping,
      launchError,
      installedVersions: comfyInstalledVersions,
      ramMemory: status?.app_resources?.comfyui?.ram_memory,
      gpuMemory: status?.app_resources?.comfyui?.gpu_memory,
    },
    ollama: {
      isRunning: ollamaRunning,
      isStarting: ollamaIsStarting,
      isStopping: ollamaIsStopping,
      launchError: ollamaLaunchError,
      installedVersions: ollamaInstalledVersions,
      ramMemory: status?.app_resources?.ollama?.ram_memory,
      gpuMemory: status?.app_resources?.ollama?.gpu_memory,
    },
    torch: {
      isRunning: torchRunning,
      isStarting: torchIsStarting,
      isStopping: torchIsStopping,
      launchError: torchLaunchError,
      installedVersions: torchInstalledVersions,
    },
  }), [
    comfyInstalledVersions,
    comfyUIRunning,
    isStarting,
    isStopping,
    launchError,
    ollamaInstalledVersions,
    ollamaIsStarting,
    ollamaIsStopping,
    ollamaLaunchError,
    ollamaRunning,
    status?.app_resources?.comfyui?.gpu_memory,
    status?.app_resources?.comfyui?.ram_memory,
    status?.app_resources?.ollama?.gpu_memory,
    status?.app_resources?.ollama?.ram_memory,
    systemResources,
    torchInstalledVersions,
    torchIsStarting,
    torchIsStopping,
    torchLaunchError,
    torchRunning,
  ]);
  const {
    apps,
    deleteApp,
    reorderApps,
    addApp,
  } = useManagedApps(managedAppsState);
  const appIds = useMemo(() => apps.map((app) => app.id), [apps]);
  const { getPanelState, setShowVersionManager } = useAppPanelState(appIds);
  const depsInstalled = status?.deps_ready ?? null;
  const isPatched = status?.patched ?? false;
  const menuShortcut = status?.menu_shortcut ?? false;
  const desktopShortcut = status?.desktop_shortcut ?? false;
  const selectedApp = apps.find((app) => app.id === selectedAppId) ?? null;
  const appDisplayName = selectedApp?.displayName ?? 'App';
  const panelState = getPanelState(selectedAppId);
  const {
    excludedModels,
    starredModels,
    toggleLink: handleToggleLink,
    toggleStar: handleToggleStar,
  } = useModelPreferences({ selectedAppId });
  const activeShortcutState =
    selectedAppId === 'comfyui' ? { menu: menuShortcut, desktop: desktopShortcut } : undefined;
  const {
    closeWindow,
    minimizeWindow,
    openModelsRoot,
    chooseLibraryRoot,
  } = useAppWindowActions();
  const {
    handleImportComplete,
    handleImportDialogClose,
    handlePathsDropped,
    importPaths,
    showImportDialog,
  } = useAppImportDialog({
    onImportComplete: fetchModels,
  });
  const {
    handleLaunchApp,
    handleOpenLog,
    handleStopApp,
  } = useAppProcessActions({
    comfyUIRunning,
    launchComfyUI,
    stopComfyUI,
    launchLogPath,
    openLogPath,
    ollamaRunning,
    launchOllama,
    stopOllama,
    ollamaLaunchLogPath,
    openOllamaLogPath,
    torchRunning,
    launchTorch,
    stopTorch,
    torchLaunchLogPath,
    openTorchLogPath,
    refetchStatus,
  });

  useAppStartupChecks({
    activeVersion: comfyActiveVersion,
    checkLauncherVersion,
    fetchDiskSpace,
    refetchStatus,
  });

  // --- Handlers ---
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

  // Computed display values
  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;
  const statusMessage = status?.message || '';
  const defaultReadyText = statusMessage?.trim().toLowerCase() === 'system ready. configure options below';
  const displayStatus = statusMessage === 'Setup complete – everything is ready' || defaultReadyText ? '' : statusMessage;
  const modelManagerProps: ModelManagerProps = {
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
  };
  const panels = buildAppShellPanels({
    activeShortcutState,
    appDisplayName,
    appVersions,
    comfyUIRunning,
    connectionUrl: selectedApp?.connectionUrl,
    depsInstalled,
    diskSpacePercent,
    displayStatus,
    isCheckingDeps,
    isInstallingDeps,
    isOllamaRunning: ollamaRunning,
    isSetupComplete,
    isTorchRunning: torchRunning,
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
      header={{
        systemResources,
        appResources: status?.app_resources?.comfyui,
        launcherUpdateAvailable,
        launcherLatestVersion: launcherUpdateState?.latestVersion ?? null,
        isCheckingLauncherUpdates,
        onCheckLauncherUpdates: () => {
          void checkLauncherUpdates();
        },
        onDownloadLauncherUpdate: () => {
          void openLauncherUpdate();
        },
        onMinimize: minimizeWindow,
        onClose: closeWindow,
        networkAvailable,
        modelLibraryLoaded,
        installationProgress,
        activeModelDownload: activeDownload,
        activeModelDownloadCount: activeDownloadCount,
      }}
      sidebar={{
        apps,
        selectedAppId,
        onSelectApp: setSelectedAppId,
        onLaunchApp: handleLaunchApp,
        onStopApp: handleStopApp,
        onOpenLog: handleOpenLog,
        onDeleteApp: handleDeleteApp,
        onReorderApps: handleReorderApps,
        onAddApp: handleAddApp,
      }}
      panels={panels}
    />
  );
}
