import { useState, useEffect, useCallback, useMemo } from 'react';
import { Box } from 'lucide-react';
import { Header } from './components/Header';
import { AppSidebar } from './components/AppSidebar';
import { ModelImportDropZone } from './components/ModelImportDropZone';
import { ModelImportDialog } from './components/ModelImportDialog';
import { AppPanelRenderer } from './components/app-panels/AppPanelRenderer';
import type { ModelManagerProps } from './components/ModelManager';
import { useVersions } from './hooks/useVersions';
import { useStatus } from './hooks/useStatus';
import { useDiskSpace } from './hooks/useDiskSpace';
import { useComfyUIProcess } from './hooks/useComfyUIProcess';
import { useModels } from './hooks/useModels';
import { useAppPanelState } from './hooks/useAppPanelState';
import { api, isAPIAvailable } from './api/adapter';
import { DEFAULT_APPS } from './config/apps';
import type { AppConfig } from './types/apps';
import { getLogger } from './utils/logger';
import { APIError, ProcessError } from './errors';
import { getAppVersionState } from './utils/appVersionState';

const logger = getLogger('App');


export default function App() {
  // --- Multi-App State ---
  const [apps, setApps] = useState<AppConfig[]>(DEFAULT_APPS);
  const [selectedAppId, setSelectedAppId] = useState<string | null>('comfyui');
  const appIds = useMemo(() => apps.map(app => app.id), [apps]);
  const { getPanelState, setShowVersionManager } = useAppPanelState(appIds);

  // --- UI State ---
  const [isInstalling, setIsInstalling] = useState(false);
  const [launcherUpdateAvailable, setLauncherUpdateAvailable] = useState(false);

  // Model Manager State
  const [starredModels, setStarredModels] = useState<Set<string>>(new Set());
  const [linkedModels, setLinkedModels] = useState<Set<string>>(new Set());

  // Model Import State (for app-level drag-drop)
  const [droppedFiles, setDroppedFiles] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);

  // --- Custom Hooks ---
  const { status, systemResources, isCheckingDeps, refetch: refetchStatus } = useStatus();
  const { diskSpacePercent, fetchDiskSpace } = useDiskSpace();
  const { launchError, launchLogPath, launchComfyUI, stopComfyUI, openLogPath } = useComfyUIProcess();
  const { modelGroups, scanModels, fetchModels } = useModels();

  const comfyVersions = useVersions({ appId: 'comfyui' });
  const ollamaVersions = useVersions({ appId: 'ollama' });
  const activeVersions =
    selectedAppId === 'comfyui'
      ? comfyVersions
      : selectedAppId === 'ollama'
        ? ollamaVersions
        : comfyVersions;
  const appVersions = getAppVersionState(selectedAppId, activeVersions);

  const { installedVersions: comfyInstalledVersions, activeVersion: comfyActiveVersion } =
    comfyVersions;
  const installationProgress = appVersions.installationProgress;
  const cacheStatus = appVersions.cacheStatus;

  const comfyUIRunning = status?.comfyui_running || false;
  const depsInstalled = status?.deps_ready ?? null;
  const isPatched = status?.patched ?? false;
  const menuShortcut = status?.menu_shortcut ?? false;
  const desktopShortcut = status?.desktop_shortcut ?? false;
  const selectedApp = apps.find(app => app.id === selectedAppId) ?? null;
  const appDisplayName = selectedApp?.displayName ?? 'App';
  const panelState = getPanelState(selectedAppId);
  const activeShortcutState =
    selectedAppId === 'comfyui' ? { menu: menuShortcut, desktop: desktopShortcut } : undefined;

  // --- API Helpers ---
  const checkLauncherVersion = async (forceRefresh = false) => {
    try {
      if (!isAPIAvailable()) return;

      await api.get_launcher_version();

      const updateResult = await api.check_launcher_updates(forceRefresh);
      if (updateResult.success) {
        setLauncherUpdateAvailable(updateResult.hasUpdate);
      }
      return updateResult;
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error checking launcher version', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Unexpected error checking launcher version', { error: error.message });
      } else {
        logger.error('Unknown error checking launcher version', { error });
      }
      return { success: false, hasUpdate: false };
    }
  };


  // --- Effects ---
  useEffect(() => {
    let waitTimeout: NodeJS.Timeout | null = null;

    const startPolling = () => {
      checkLauncherVersion(true);
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
    };
  }, []);

  // Update app status based on backend data
  useEffect(() => {
    if (!status) return;

    const freshResources = systemResources;
    setApps(prevApps => prevApps.map(app => {
      if (app.id === 'comfyui') {
        const resources = status.app_resources?.comfyui;

        let gpuUsagePercent: number | undefined = undefined;
        const gpuTotal = freshResources?.gpu?.memory_total;
        if (resources?.gpu_memory && gpuTotal && gpuTotal > 0) {
          gpuUsagePercent = Math.round((resources.gpu_memory / gpuTotal) * 100);
        }

        let ramUsagePercent: number | undefined = undefined;
        const ramTotal = freshResources?.ram?.total;
        if (resources?.ram_memory && ramTotal && ramTotal > 0) {
          ramUsagePercent = Math.round((resources.ram_memory / ramTotal) * 100);
        }

        const updates: Partial<AppConfig> = {
          status: comfyUIRunning ? 'running' : (depsInstalled ? 'idle' : 'idle'),
          ramUsage: ramUsagePercent,
          gpuUsage: gpuUsagePercent,
        };

        if (comfyUIRunning) {
          updates.iconState = 'running';
        } else if (launchError) {
          updates.iconState = 'error';
        }

        return { ...app, ...updates };
      }
      return app;
    }));
  }, [status, systemResources, comfyUIRunning, depsInstalled, launchError]);

  // Manage iconState based on ComfyUI installed versions
  useEffect(() => {
    setApps(prevApps => prevApps.map(app => {
      if (app.id === 'comfyui') {
        let newState: 'running' | 'offline' | 'uninstalled' | 'error';

        if (comfyUIRunning) {
          newState = 'running';
        } else if (launchError) {
          newState = 'error';
        } else if (comfyInstalledVersions.length > 0) {
          newState = 'offline';
        } else {
          newState = 'uninstalled';
        }

        if (newState !== app.iconState) {
          return { ...app, iconState: newState };
        }
      }
      return app;
    }));
  }, [comfyInstalledVersions, comfyUIRunning, launchError]);

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

  const handleLaunchComfyUI = async () => {
    if (comfyUIRunning) {
      await stopComfyUI();
    } else {
      await launchComfyUI();
    }
    await refetchStatus(false);
    setTimeout(() => refetchStatus(false), 1200);
  };

  const handleLaunchApp = async (appId: string) => {
    if (appId === 'comfyui' && !comfyUIRunning) {
      await handleLaunchComfyUI();
    }
  };

  const handleStopApp = async (appId: string) => {
    if (appId === 'comfyui' && comfyUIRunning) {
      await handleLaunchComfyUI();
    }
  };

  const handleOpenLog = async (appId: string) => {
    if (appId === 'comfyui' && launchLogPath) {
      await openLogPath(launchLogPath);
    }
  };

  const handleDeleteApp = (appId: string) => {
    if (appId === 'comfyui') {
      logger.warn('Attempt to delete ComfyUI app prevented', { appId });
      return;
    }
    logger.info('Deleting app', { appId });
    setApps(prevApps => prevApps.filter(app => app.id !== appId));
    if (selectedAppId === appId) {
      setSelectedAppId(null);
    }
  };

  const handleReorderApps = (reorderedApps: AppConfig[]) => {
    setApps(reorderedApps);
  };

  const handleAddApp = (insertAtIndex: number) => {
    const newAppNumber = apps.length + 1;
    const newApp: AppConfig = {
      id: `app-${Date.now()}`,
      name: `new-app-${newAppNumber}`,
      displayName: `New App ${newAppNumber}`,
      icon: Box,
      status: 'idle',
      iconState: 'uninstalled',
      ramUsage: 0,
      gpuUsage: 0,
    };

    setApps(prevApps => {
      const newApps = [...prevApps];
      newApps.splice(insertAtIndex, 0, newApp);
      return newApps;
    });
  };

  const handleToggleStar = (modelId: string) => {
    setStarredModels(prev => {
      const newSet = new Set(prev);
      if (newSet.has(modelId)) {
        newSet.delete(modelId);
      } else {
        newSet.add(modelId);
      }
      return newSet;
    });
  };

  const handleToggleLink = (modelId: string) => {
    setLinkedModels(prev => {
      const newSet = new Set(prev);
      if (newSet.has(modelId)) {
        newSet.delete(modelId);
      } else {
        newSet.add(modelId);
      }
      return newSet;
    });
  };

  // Model import handlers (app-level drag-drop)
  const handleFilesDropped = useCallback((paths: string[]) => {
    logger.info('Files dropped for import', { count: paths.length });
    setDroppedFiles(paths);
    setShowImportDialog(true);
  }, []);

  const handleImportDialogClose = useCallback(() => {
    setShowImportDialog(false);
    setDroppedFiles([]);
  }, []);

  const handleImportComplete = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    fetchModels();
  }, [fetchModels]);

  const handleShowVersionManager = (show: boolean) => {
    if (!selectedAppId) {
      return;
    }
    setShowVersionManager(selectedAppId, show);
  };

  const openModelsRoot = async () => {
    if (!isAPIAvailable()) return;
    try {
      const result = await api.open_path('shared-resources/models');
      if (!result.success) {
        throw new APIError(result.error || 'Failed to open models folder', 'open_path');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening models folder', { error: error.message, endpoint: error.endpoint, path: 'shared-resources/models' });
      } else if (error instanceof Error) {
        logger.error('Unexpected error opening models folder', { error: error.message, path: 'shared-resources/models' });
      } else {
        logger.error('Unknown error opening models folder', { error, path: 'shared-resources/models' });
      }
    }
  };

  const closeWindow = () => {
    if (isAPIAvailable()) {
      void api.close_window();
    } else {
      window.close();
    }
  };

  // Computed display values
  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;
  const statusMessage = status?.message || '';
  const defaultReadyText = statusMessage?.trim().toLowerCase() === 'system ready. configure options below';
  const displayStatus = statusMessage === 'Setup complete – everything is ready' || defaultReadyText ? '' : statusMessage;
  const modelManagerProps: ModelManagerProps = {
    modelGroups,
    starredModels,
    linkedModels,
    onToggleStar: handleToggleStar,
    onToggleLink: handleToggleLink,
    selectedAppId,
    onAddModels: scanModels,
    onOpenModelsRoot: openModelsRoot,
    onModelsImported: fetchModels,
    activeVersion: appVersions.activeVersion,
  };

  return (
    <div className="w-full h-screen gradient-bg-blobs flex flex-col relative overflow-hidden font-mono">
      {/* App-level drag-and-drop import overlay */}
      <ModelImportDropZone onFilesDropped={handleFilesDropped} enabled={true} />

      {/* Import dialog */}
      {showImportDialog && droppedFiles.length > 0 && (
        <ModelImportDialog
          filePaths={droppedFiles}
          onClose={handleImportDialogClose}
          onImportComplete={handleImportComplete}
        />
      )}

      <Header
        systemResources={systemResources}
        appResources={status?.app_resources?.comfyui}
        launcherUpdateAvailable={launcherUpdateAvailable}
        onClose={closeWindow}
        cacheStatus={cacheStatus}
        installationProgress={installationProgress}
      />

      <div className="flex flex-1 relative z-10 overflow-hidden">
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
