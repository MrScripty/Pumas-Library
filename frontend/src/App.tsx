import React, { useState, useEffect, useCallback } from 'react';
import { motion } from 'framer-motion';
import { ArrowLeft, RefreshCw, Box } from 'lucide-react';
import { VersionSelector } from './components/VersionSelector';
import { InstallDialog } from './components/InstallDialog';
import { Header } from './components/Header';
import { AppSidebar } from './components/AppSidebar';
import { ModelManager } from './components/ModelManager';
import { ModelImportDropZone } from './components/ModelImportDropZone';
import { ModelImportDialog } from './components/ModelImportDialog';
import { DependencySection } from './components/DependencySection';
import { StatusDisplay } from './components/StatusDisplay';
import { useVersions } from './hooks/useVersions';
import { useStatus } from './hooks/useStatus';
import { useDiskSpace } from './hooks/useDiskSpace';
import { useComfyUIProcess } from './hooks/useComfyUIProcess';
import { useModels } from './hooks/useModels';
import { pywebview } from './api/pywebview';
import { DEFAULT_APPS } from './config/apps';
import type { AppConfig } from './types/apps';
import { getLogger } from './utils/logger';
import { APIError, ProcessError } from './errors';

const logger = getLogger('App');

// PyWebViewAPI is imported from api/pywebview.ts

export default function App() {
  // --- Multi-App State ---
  const [apps, setApps] = useState<AppConfig[]>(DEFAULT_APPS);
  const [selectedAppId, setSelectedAppId] = useState<string | null>('comfyui');

  // --- UI State ---
  const [isInstalling, setIsInstalling] = useState(false);
  const [showVersionManager, setShowVersionManager] = useState(false);
  const [isRefreshingVersions, setIsRefreshingVersions] = useState(false);
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

  const {
    installedVersions,
    activeVersion,
    availableVersions,
    isLoading: isVersionLoading,
    switchVersion,
    installVersion,
    removeVersion,
    refreshAll,
    openActiveInstall,
    installingTag,
    installationProgress,
    fetchInstallationProgress,
    installNetworkStatus,
    defaultVersion,
    setDefaultVersion,
    cacheStatus,
  } = useVersions();

  // --- Computed Values ---
  const computeHasNewVersion = useCallback((available: any[], installed: string[]) => {
    if (!available || available.length === 0 || !installed) {
      return false;
    }
    const latestAvailable = available[0]?.tag_name;
    if (!latestAvailable) {
      return false;
    }
    return !installed.includes(latestAvailable);
  }, []);

  const hasUpdate = React.useMemo(() => {
    return computeHasNewVersion(availableVersions, installedVersions);
  }, [availableVersions, installedVersions, computeHasNewVersion]);

  const latestVersion = React.useMemo(() => {
    return availableVersions && availableVersions.length > 0 ? availableVersions[0]?.tag_name : null;
  }, [availableVersions]);

  const comfyUIRunning = status?.comfyui_running || false;
  const depsInstalled = status?.deps_ready ?? null;
  const isPatched = status?.patched ?? false;
  const menuShortcut = status?.menu_shortcut ?? false;
  const desktopShortcut = status?.desktop_shortcut ?? false;

  // --- API Helpers ---
  const checkLauncherVersion = async (forceRefresh = false) => {
    try {
      if (!window.pywebview?.api) return;

      await window.pywebview.api.get_launcher_version();

      const updateResult = await window.pywebview.api.check_launcher_updates(forceRefresh);
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
      if (pywebview.isAvailable()) {
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

  // Manage iconState based on installedVersions
  useEffect(() => {
    setApps(prevApps => prevApps.map(app => {
      if (app.id === 'comfyui') {
        let newState: 'running' | 'offline' | 'uninstalled' | 'error';

        if (comfyUIRunning) {
          newState = 'running';
        } else if (launchError) {
          newState = 'error';
        } else if (installedVersions.length > 0) {
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
  }, [installedVersions, comfyUIRunning, launchError]);

  // Launch error flash effect is handled by AppIndicator component

  // Refetch status when active version changes
  useEffect(() => {
    if (activeVersion && pywebview.isAvailable()) {
      void refetchStatus(false);
    }
  }, [activeVersion]);

  // --- Handlers ---
  const handleRefreshProgress = async () => {
    await fetchInstallationProgress();
  };

  const handleMakeDefault = async (tag: string | null) => {
    await setDefaultVersion(tag);
    return true;
  };

  const handleInstallDeps = async () => {
    if (!pywebview.isAvailable()) return;

    setIsInstalling(true);
    try {
      await pywebview.installDeps();
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

  const openModelsRoot = async () => {
    if (!pywebview.isAvailable()) return;
    try {
      await pywebview.openPath('shared-resources/models');
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
    if (pywebview.isAvailable()) {
      void pywebview.closeWindow();
    } else {
      window.close();
    }
  };

  // Computed display values
  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;
  const statusMessage = status?.message || '';
  const defaultReadyText = statusMessage?.trim().toLowerCase() === 'system ready. configure options below';
  const displayStatus = statusMessage === 'Setup complete – everything is ready' || defaultReadyText ? '' : statusMessage;

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
          {selectedAppId === 'comfyui' ? (
            <div className="flex-1 p-6 flex flex-col items-center overflow-auto">
              {isCheckingDeps || depsInstalled === null ? (
                <div className="w-full flex items-center justify-center gap-2 text-[hsl(var(--text-secondary))]">
                  <span className="text-sm">Checking Dependencies...</span>
                </div>
              ) : showVersionManager ? (
                <div className="w-full flex-1 flex flex-col gap-4 min-h-0">
                  <div className="w-full flex items-center justify-between flex-shrink-0">
                    <button
                      onClick={() => setShowVersionManager(false)}
                      className="flex items-center gap-2 px-3 py-2 rounded border border-[hsl(var(--border-control))] bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-primary))] text-sm transition-colors"
                    >
                      <ArrowLeft size={14} />
                      <span>Back to setup</span>
                    </button>
                    <div className="flex items-center gap-3 text-xs text-[hsl(var(--text-secondary))]">
                      <span>{installedVersions.length} installed</span>
                      <motion.button
                        onClick={async () => {
                          if (isRefreshingVersions) return;
                          setIsRefreshingVersions(true);
                          try {
                            await refreshAll(true);
                          } finally {
                            setIsRefreshingVersions(false);
                          }
                        }}
                        disabled={isRefreshingVersions || isVersionLoading}
                        className="p-2 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors disabled:opacity-50"
                        whileHover={{ scale: isRefreshingVersions || isVersionLoading ? 1 : 1.05 }}
                        whileTap={{ scale: isRefreshingVersions || isVersionLoading ? 1 : 0.96 }}
                        title="Refresh versions"
                      >
                        <RefreshCw size={14} className={isRefreshingVersions ? 'animate-spin text-[hsl(var(--text-tertiary))]' : 'text-[hsl(var(--text-secondary))]'} />
                      </motion.button>
                    </div>
                  </div>
                  <div className="w-full flex-1 min-h-0 overflow-hidden">
                    <InstallDialog
                      isOpen={showVersionManager}
                      onClose={() => setShowVersionManager(false)}
                      availableVersions={availableVersions}
                      installedVersions={installedVersions}
                      isLoading={isVersionLoading}
                      onInstallVersion={installVersion}
                      onRemoveVersion={removeVersion}
                      onRefreshAll={refreshAll}
                      installingTag={installingTag}
                      installationProgress={installationProgress}
                      installNetworkStatus={installNetworkStatus}
                      onRefreshProgress={handleRefreshProgress}
                      displayMode="page"
                    />
                  </div>
                </div>
              ) : (
                <>
                  <div className="w-full mb-4">
                    <VersionSelector
                      installedVersions={installedVersions}
                      activeVersion={activeVersion}
                      isLoading={isVersionLoading}
                      switchVersion={switchVersion}
                      openActiveInstall={openActiveInstall}
                      onOpenVersionManager={() => setShowVersionManager(true)}
                      installNetworkStatus={installNetworkStatus}
                      defaultVersion={defaultVersion}
                      onMakeDefault={handleMakeDefault}
                      installingVersion={installingTag}
                      activeShortcutState={{ menu: menuShortcut, desktop: desktopShortcut }}
                      diskSpacePercent={diskSpacePercent}
                      hasNewVersion={hasUpdate}
                      latestVersion={latestVersion}
                    />
                  </div>

                  <DependencySection
                    depsInstalled={depsInstalled}
                    isInstalling={isInstalling}
                    comfyUIRunning={comfyUIRunning}
                    onInstall={handleInstallDeps}
                  />

                  <motion.div
                    className="w-full flex flex-col items-center gap-6"
                    animate={{
                      opacity: depsInstalled ? 1 : 0.3,
                      filter: depsInstalled ? 'blur(0px)' : 'blur(1px)',
                      pointerEvents: depsInstalled ? 'auto' : 'none'
                    }}
                    transition={{ duration: 0.4 }}
                  >
                    {displayStatus && (
                      <StatusDisplay
                        message={displayStatus}
                        isRunning={comfyUIRunning}
                        isSetupComplete={isSetupComplete}
                      />
                    )}
                  </motion.div>
                </>
              )}
            </div>
          ) : (
            <div className="flex-1 flex flex-col gap-4 p-8 px-0 mx-2 py-1 overflow-hidden">
              {selectedAppId && (
                <div className="text-center py-4">
                  <p className="text-[hsl(var(--launcher-text-secondary))] text-sm">
                    {`${apps.find(a => a.id === selectedAppId)?.displayName} - Coming Soon`}
                  </p>
                </div>
              )}

              <ModelManager
                modelGroups={modelGroups}
                starredModels={starredModels}
                linkedModels={linkedModels}
                onToggleStar={handleToggleStar}
                onToggleLink={handleToggleLink}
                selectedAppId={selectedAppId}
                onAddModels={scanModels}
                onOpenModelsRoot={openModelsRoot}
                onModelsImported={fetchModels}
                activeVersion={activeVersion}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
