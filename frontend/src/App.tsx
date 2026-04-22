import { useState, useEffect, useMemo } from 'react';
import { Header } from './components/Header';
import { AppSidebar } from './components/AppSidebar';
import { ModelImportDropZone } from './components/ModelImportDropZone';
import { ModelImportDialog } from './components/ModelImportDialog';
import { AppPanelRenderer } from './components/app-panels/AppPanelRenderer';
import type { ModelManagerProps } from './components/ModelManager';
import type { AppConfig } from './types/apps';
import { useVersions } from './hooks/useVersions';
import { useStatus } from './hooks/useStatus';
import { useDiskSpace } from './hooks/useDiskSpace';
import { useComfyUIProcess } from './hooks/useComfyUIProcess';
import { useOllamaProcess } from './hooks/useOllamaProcess';
import { useTorchProcess } from './hooks/useTorchProcess';
import { useModels } from './hooks/useModels';
import { useActiveModelDownload } from './hooks/useActiveModelDownload';
import { useAppImportDialog } from './hooks/useAppImportDialog';
import { useAppPanelState } from './hooks/useAppPanelState';
import { useAppProcessActions } from './hooks/useAppProcessActions';
import { useAppWindowActions } from './hooks/useAppWindowActions';
import { useLauncherUpdates } from './hooks/useLauncherUpdates';
import { useManagedApps } from './hooks/useManagedApps';
import { useModelPreferences } from './hooks/useModelPreferences';
import { api, isAPIAvailable } from './api/adapter';
import { getLogger } from './utils/logger';
import { APIError, ProcessError } from './errors';
import { getAppVersionState } from './utils/appVersionState';

const logger = getLogger('App');

export default function App() {
  const [selectedAppId, setSelectedAppId] = useState<string | null>(
    __FEATURE_MULTI_APP__ ? null : 'comfyui'
  );

  // --- UI State ---
  const [isInstalling, setIsInstalling] = useState(false);

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

  const comfyVersions = useVersions({
    appId: 'comfyui',
    trackAvailableVersions: selectedAppId === 'comfyui',
  });
  const ollamaVersions = useVersions({
    appId: 'ollama',
    trackAvailableVersions: selectedAppId === 'ollama',
  });
  const torchVersions = useVersions({
    appId: 'torch',
    trackAvailableVersions: selectedAppId === 'torch',
  });

  // Map app IDs to their version hooks - only supported apps have versions
  const activeVersions = useMemo(() => {
    if (selectedAppId === 'comfyui') return comfyVersions;
    if (selectedAppId === 'ollama') return ollamaVersions;
    if (selectedAppId === 'torch') return torchVersions;
    // For unsupported apps or no selection, return comfyVersions as placeholder
    // (getAppVersionState will return UNSUPPORTED_VERSION_STATE anyway)
    return comfyVersions;
  }, [selectedAppId, comfyVersions, ollamaVersions, torchVersions]);

  const appVersions = getAppVersionState(selectedAppId, activeVersions);

  const { installedVersions: comfyInstalledVersions, activeVersion: comfyActiveVersion } =
    comfyVersions;
  const { installedVersions: ollamaInstalledVersions } = ollamaVersions;
  const { installedVersions: torchInstalledVersions } = torchVersions;
  const installationProgress = appVersions.installationProgress;

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

  // --- Effects ---
  useEffect(() => {
    let waitTimeout: NodeJS.Timeout | null = null;
    let updateTimeout: NodeJS.Timeout | null = null;

    const startPolling = () => {
      // Delay update check to not block initial render
      updateTimeout = setTimeout(() => {
        checkLauncherVersion(false).catch((err: unknown) => {
          logger.debug('Background update check failed', { error: err });
        });
      }, 3000);
      void fetchDiskSpace();
    };

    const waitForApi = () => {
      if (isAPIAvailable()) {
        startPolling();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (waitTimeout) clearTimeout(waitTimeout);
      if (updateTimeout) clearTimeout(updateTimeout);
    };
  }, [checkLauncherVersion, fetchDiskSpace]);

  // Launch error flash effect is handled by AppIndicator component

  // Refetch status when active version changes
  useEffect(() => {
    if (comfyActiveVersion && isAPIAvailable()) {
      void refetchStatus(false);
    }
  }, [comfyActiveVersion]);

  // --- Handlers ---
  const handleInstallDeps = async () => {
    if (!isAPIAvailable()) return;

    setIsInstalling(true);
    try {
      await api.install_deps();
      await refetchStatus();
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error installing dependencies', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof ProcessError) {
        logger.error('Process error installing dependencies', { error: error.message, exitCode: error.exitCode });
      } else if (error instanceof Error) {
        logger.error('Unexpected error installing dependencies', { error: error.message });
      } else {
        logger.error('Unknown error installing dependencies', { error });
      }
    } finally {
      setIsInstalling(false);
    }
  };

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

  return (
    <div className="w-full h-screen gradient-bg-blobs flex flex-col relative overflow-hidden font-mono">
      {/* App-level drag-and-drop import overlay */}
      <ModelImportDropZone onPathsDropped={handlePathsDropped} enabled={true} />

      {/* Import dialog */}
      {showImportDialog && importPaths.length > 0 && (
        <ModelImportDialog
          importPaths={importPaths}
          onClose={handleImportDialogClose}
          onImportComplete={handleImportComplete}
        />
      )}

      <Header
        systemResources={systemResources}
        appResources={status?.app_resources?.comfyui}
        launcherUpdateAvailable={launcherUpdateAvailable}
        launcherLatestVersion={launcherUpdateState?.latestVersion ?? null}
        isCheckingLauncherUpdates={isCheckingLauncherUpdates}
        onCheckLauncherUpdates={() => {
          void checkLauncherUpdates();
        }}
        onDownloadLauncherUpdate={() => {
          void openLauncherUpdate();
        }}
        onMinimize={minimizeWindow}
        onClose={closeWindow}
        networkAvailable={networkAvailable}
        modelLibraryLoaded={modelLibraryLoaded}
        installationProgress={installationProgress}
        activeModelDownload={activeDownload}
        activeModelDownloadCount={activeDownloadCount}
      />

      <div className="flex flex-1 relative z-10 overflow-hidden">
        {__FEATURE_MULTI_APP__ && (
          <AppSidebar
            apps={apps}
            selectedAppId={selectedAppId}
            onSelectApp={setSelectedAppId}
            onLaunchApp={handleLaunchApp}
            onStopApp={handleStopApp}
            onOpenLog={handleOpenLog}
            onDeleteApp={handleDeleteApp}
            onReorderApps={handleReorderApps}
            onAddApp={handleAddApp}
          />
        )}

        <div className="flex-1 flex flex-col overflow-hidden">
          <AppPanelRenderer
            selectedAppId={selectedAppId}
            comfyUI={{
              appDisplayName,
              versions: appVersions,
              showVersionManager: panelState.showVersionManager,
              onShowVersionManager: handleShowVersionManager,
              activeShortcutState,
              diskSpacePercent,
              isCheckingDeps,
              depsInstalled,
              isInstallingDeps: isInstalling,
              comfyUIRunning,
              onInstallDeps: handleInstallDeps,
              displayStatus,
              isSetupComplete,
            }}
            ollama={{
              appDisplayName,
              connectionUrl: selectedApp?.connectionUrl,
              versions: appVersions,
              showVersionManager: panelState.showVersionManager,
              onShowVersionManager: handleShowVersionManager,
              activeShortcutState,
              diskSpacePercent,
              modelManagerProps,
              isOllamaRunning: ollamaRunning,
              modelGroups,
            }}
            torch={{
              appDisplayName,
              connectionUrl: selectedApp?.connectionUrl,
              versions: appVersions,
              showVersionManager: panelState.showVersionManager,
              onShowVersionManager: handleShowVersionManager,
              activeShortcutState,
              diskSpacePercent,
              modelManagerProps,
              isTorchRunning: torchRunning,
              modelGroups,
            }}
            fallback={{
              appDisplayName,
              modelManagerProps,
            }}
          />
        </div>
      </div>
    </div>
  );
}
