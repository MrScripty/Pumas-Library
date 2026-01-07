import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ArrowDownToLine, Loader2, ArrowLeft, RefreshCw, Box } from 'lucide-react';
import { VersionSelector } from './components/VersionSelector';
import { InstallDialog } from './components/InstallDialog';
import { Header } from './components/Header';
import { AppSidebar } from './components/AppSidebar';
import { ModelManager } from './components/ModelManager';
import { useVersions } from './hooks/useVersions';
import { DEFAULT_APPS } from './config/apps';
import type { AppConfig, ModelCategory, ModelInfo, SystemResources } from './types/apps';

// TypeScript definitions for PyWebView API
declare global {
  interface Window {
    pywebview?: {
      api: {
        // Original API methods
        get_status: () => Promise<any>;
        get_disk_space: () => Promise<{ success: boolean; total: number; used: number; free: number; percent: number; error?: string }>;
        install_deps: () => Promise<{ success: boolean }>;
        toggle_menu: (tag?: string) => Promise<{ success: boolean }>;
        toggle_desktop: (tag?: string) => Promise<{ success: boolean }>;
        close_window: () => Promise<{ success: boolean }>;
        launch_comfyui: () => Promise<{ success: boolean; error?: string; log_path?: string; ready?: boolean }>;
        open_path: (path: string) => Promise<{ success: boolean; error?: string }>;
        stop_comfyui: () => Promise<{ success: boolean }>;
        get_version_shortcuts: (tag: string) => Promise<{ success: boolean; state: { menu: boolean; desktop: boolean; tag: string }; error?: string }>;
        get_all_shortcut_states: () => Promise<{ success: boolean; states: { active: string | null; states: Record<string, { menu: boolean; desktop: boolean; tag?: string }> }; error?: string }>;
        set_version_shortcuts: (tag: string, enabled: boolean) => Promise<{ success: boolean; state: { menu: boolean; desktop: boolean; tag: string }; error?: string }>;
        toggle_version_menu: (tag: string) => Promise<{ success: boolean; state: any; error?: string }>;
        toggle_version_desktop: (tag: string) => Promise<{ success: boolean; state: any; error?: string }>;

        // Version Management API (Phase 5)
        get_available_versions: (force_refresh?: boolean) => Promise<{ success: boolean; versions: any[]; error?: string }>;
        get_installed_versions: () => Promise<{ success: boolean; versions: string[]; error?: string }>;
        validate_installations: () => Promise<{ success: boolean; result: { had_invalid: boolean; removed: string[]; valid: string[] }; error?: string }>;
        get_installation_progress: () => Promise<any>;
        install_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        cancel_installation: () => Promise<{ success: boolean; error?: string }>;
        remove_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        switch_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        get_active_version: () => Promise<{ success: boolean; version: string; error?: string }>;
        check_version_dependencies: (tag: string) => Promise<{ success: boolean; dependencies: any; error?: string }>;
        install_version_dependencies: (tag: string) => Promise<{ success: boolean; error?: string }>;
        get_version_status: () => Promise<{ success: boolean; status: any; error?: string }>;
        get_version_info: (tag: string) => Promise<{ success: boolean; info: any; error?: string }>;
        launch_version: (tag: string, extra_args?: string[]) => Promise<{ success: boolean; error?: string; log_path?: string; ready?: boolean }>;
        get_default_version: () => Promise<{ success: boolean; version: string; error?: string }>;
        set_default_version: (tag?: string | null) => Promise<{ success: boolean; error?: string }>;

        // Size Calculation API (Phase 6.2.5c)
        calculate_release_size: (tag: string, force_refresh?: boolean) => Promise<any>;
        calculate_all_release_sizes: () => Promise<any>;

        // Utility
        open_url: (url: string) => Promise<{ success: boolean; error?: string }>;

        // Resource Management API (Phase 5)
        get_models: () => Promise<{ success: boolean; models: any; error?: string }>;
        get_custom_nodes: (version_tag: string) => Promise<{ success: boolean; nodes: string[]; error?: string }>;
        install_custom_node: (git_url: string, version_tag: string, node_name?: string) => Promise<{ success: boolean; error?: string }>;
        update_custom_node: (node_name: string, version_tag: string) => Promise<{ success: boolean; error?: string }>;
        remove_custom_node: (node_name: string, version_tag: string) => Promise<{ success: boolean; error?: string }>;
        scan_shared_storage: () => Promise<{ success: boolean; result: any; error?: string }>;
        download_model_from_hf: (
          repo_id: string,
          family: string,
          official_name: string,
          model_type?: string | null,
          subtype?: string | null,
          quant?: string | null
        ) => Promise<{ success: boolean; model_path?: string; error?: string }>;
        start_model_download_from_hf: (
          repo_id: string,
          family: string,
          official_name: string,
          model_type?: string | null,
          subtype?: string | null,
          quant?: string | null
        ) => Promise<{ success: boolean; download_id?: string; total_bytes?: number; error?: string }>;
        get_model_download_status: (download_id: string) => Promise<{
          success: boolean;
          download_id?: string;
          repo_id?: string;
          status?: string;
          progress?: number;
          downloaded_bytes?: number;
          total_bytes?: number;
          error?: string;
        }>;
        cancel_model_download: (download_id: string) => Promise<{ success: boolean; error?: string }>;
        search_hf_models: (query: string, kind?: string | null, limit?: number) => Promise<{
          success: boolean;
          models: any[];
          error?: string;
        }>;

        // Launcher Update API
        get_launcher_version: () => Promise<{ success: boolean; version: string; branch: string; isGitRepo: boolean; error?: string }>;
        check_launcher_updates: (force_refresh?: boolean) => Promise<{ success: boolean; hasUpdate: boolean; currentCommit: string; latestCommit: string; commitsBehind: number; commits: any[]; error?: string }>;
        apply_launcher_update: () => Promise<{ success: boolean; message: string; newCommit?: string; error?: string }>;
        restart_launcher: () => Promise<{ success: boolean; message: string; error?: string }>;

        // Cache Status API
        get_github_cache_status: () => Promise<{
          success: boolean;
          status: {
            has_cache: boolean;
            is_valid: boolean;
            is_fetching: boolean;
            age_seconds?: number;
            last_fetched?: string;
            releases_count?: number;
          };
          error?: string;
        }>;
        has_background_fetch_completed: () => Promise<{
          success: boolean;
          completed: boolean;
          error?: string;
        }>;
        reset_background_fetch_flag: () => Promise<{
          success: boolean;
          error?: string;
        }>;

        // System Resource API
        get_system_resources: () => Promise<{
          success: boolean;
          resources: SystemResources;
          error?: string;
        }>;
      };
    };
  }
}

export default function App() {
  // --- Multi-App State ---
  const [apps, setApps] = useState<AppConfig[]>(DEFAULT_APPS);
  const [selectedAppId, setSelectedAppId] = useState<string | null>('comfyui');
  const [systemResources, setSystemResources] = useState<SystemResources | undefined>();

  // --- State ---
  const [version, setVersion] = useState("Loading...");
  const [depsInstalled, setDepsInstalled] = useState<boolean | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);

  // App States
  const [isPatched, setIsPatched] = useState(false);
  const [menuShortcut, setMenuShortcut] = useState(false);
  const [desktopShortcut, setDesktopShortcut] = useState(false);
  const [shortcutVersion, setShortcutVersion] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState("Checking system status...");
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchLogPath, setLaunchLogPath] = useState<string | null>(null);
  const [launchErrorFlash, setLaunchErrorFlash] = useState(false);

  // ComfyUI running state
  const [comfyUIRunning, setComfyUIRunning] = useState(false);
  const [showVersionManager, setShowVersionManager] = useState(false);
  const [isRefreshingVersions, setIsRefreshingVersions] = useState(false);
  const [launcherVersion, setLauncherVersion] = useState<string | null>(null);
  const isPolling = useRef(false);
  const modelCountRef = useRef<number | null>(null);
  const isModelCountPolling = useRef(false);

  // Launcher update state
  const [launcherUpdateAvailable, setLauncherUpdateAvailable] = useState(false);
  const [isUpdatingLauncher, setIsUpdatingLauncher] = useState(false);
  const [updateCheckDone, setUpdateCheckDone] = useState(false);
  const [isCheckingLauncherUpdate, setIsCheckingLauncherUpdate] = useState(false);
  const [lastLauncherUpdateCheckAt, setLastLauncherUpdateCheckAt] = useState<number | null>(null);

  // Disk space tracking
  const [diskSpacePercent, setDiskSpacePercent] = useState(0);

  // Model Manager State
  const [modelGroups, setModelGroups] = useState<ModelCategory[]>([]);
  const [starredModels, setStarredModels] = useState<Set<string>>(new Set());
  const [linkedModels, setLinkedModels] = useState<Set<string>>(new Set());

  // Version data (shared between selector and manager view)
  // Compute if there's a new version available (latest in availableVersions not in installedVersions)
  const computeHasNewVersion = React.useCallback((available: any[], installed: string[]) => {
    if (!available || available.length === 0 || !installed) {
      return false;
    }
    // Get the latest version (first in the list, as they're sorted newest first)
    const latestAvailable = available[0]?.tag_name;
    if (!latestAvailable) {
      return false;
    }
    // Check if this latest version is not installed
    return !installed.includes(latestAvailable);
  }, []);

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

  // Release info (computed from available vs installed versions)
  const hasUpdate = React.useMemo(() => {
    const result = computeHasNewVersion(availableVersions, installedVersions);
    console.log('[App] Computed hasUpdate:', result, 'availableVersions:', availableVersions.length, 'installedVersions:', installedVersions.length);
    return result;
  }, [availableVersions, installedVersions, computeHasNewVersion]);

  const latestVersion = React.useMemo(() => {
    const result = availableVersions && availableVersions.length > 0 ? availableVersions[0]?.tag_name : null;
    console.log('[App] Computed latestVersion:', result);
    return result;
  }, [availableVersions]);

  // --- API Helpers ---
  const fetchSystemResources = async () => {
    try {
      if (window.pywebview?.api?.get_system_resources) {
        const result = await window.pywebview.api.get_system_resources();
        if (result.success) {
          setSystemResources(result.resources);
          return result.resources;
        }
      }
    } catch (e) {
      console.error("Failed to fetch system resources:", e);
    }
    return null;
  };

  const fetchDiskSpace = async () => {
    try {
      if (window.pywebview?.api?.get_disk_space) {
        const diskData = await window.pywebview.api.get_disk_space();
        if (diskData.success) {
          setDiskSpacePercent(diskData.percent || 0);
        }
      }
    } catch (e) {
      console.error("Failed to fetch disk space:", e);
    }
  };

  const fetchStatus = async (isInitialLoad = false) => {
    const startTime = Date.now();

    if (isInitialLoad) {
      setIsCheckingDeps(true);
    }

    try {
      let data;
      if (window.pywebview) {
        data = await window.pywebview.api.get_status();
      } else {
        setStatusMessage("Running in development mode - PyWebView API not available");
        setIsLoading(false);
        setIsCheckingDeps(false);
        setDepsInstalled(false);
        setVersion("Dev Mode");
        setShortcutVersion(null);
        return;
      }

      setVersion(data.version);
      setDepsInstalled(data.deps_ready);
      setIsPatched(data.patched);
      setMenuShortcut(data.menu_shortcut);
      setDesktopShortcut(data.desktop_shortcut);
      setShortcutVersion(data.shortcut_version || null);
      setStatusMessage(data.message);
      setComfyUIRunning(data.comfyui_running || false);
      setLaunchError(data.last_launch_error || null);
      setLaunchLogPath(data.last_launch_log || null);

      // Note: hasUpdate and latestVersion are now computed from availableVersions vs installedVersions

      await fetchDiskSpace();
      const freshSystemResources = await fetchSystemResources();

      // Update app status based on ComfyUI state
      setApps(prevApps => prevApps.map(app => {
        if (app.id === 'comfyui') {
          const resources = data.app_resources?.comfyui;

          // Convert GPU memory (GB) to percentage of total GPU memory
          let gpuUsagePercent: number | undefined = undefined;
          const gpuTotal = freshSystemResources?.gpu?.memory_total || systemResources?.gpu?.memory_total;
          if (resources?.gpu_memory && gpuTotal && gpuTotal > 0) {
            // GPU memory from backend is in GB, memory_total from systemResources is total GPU capacity in GB
            gpuUsagePercent = Math.round((resources.gpu_memory / gpuTotal) * 100);
          }

          // Convert RAM memory (GB) to percentage of total RAM
          let ramUsagePercent: number | undefined = undefined;
          const ramTotal = freshSystemResources?.ram?.total || systemResources?.ram?.total;
          if (resources?.ram_memory && ramTotal && ramTotal > 0) {
            // RAM memory from backend is in GB, total from systemResources is total RAM capacity in GB
            ramUsagePercent = Math.round((resources.ram_memory / ramTotal) * 100);
          }

          // Update only resources and running state
          // iconState is managed by the useEffect that watches installedVersions
          const updates: Partial<AppConfig> = {
            status: data.comfyui_running ? 'running' : (data.deps_ready ? 'idle' : 'idle'),
            ramUsage: ramUsagePercent,
            gpuUsage: gpuUsagePercent,
          };

          // ONLY update iconState for running/error states (these are immediate)
          // Let useEffect handle offline/uninstalled transition based on installedVersions
          if (data.comfyui_running) {
            updates.iconState = 'running';
          } else if (data.last_launch_error) {
            updates.iconState = 'error';
          }
          // DON'T set offline/uninstalled here - useEffect handles it

          return {
            ...app,
            ...updates,
          };
        }
        return app;
      }));

      if (isInitialLoad) {
        const elapsedTime = Date.now() - startTime;
        const remainingTime = Math.max(0, 800 - elapsedTime);
        setTimeout(() => {
          setIsLoading(false);
          setIsCheckingDeps(false);
        }, remainingTime);
      } else {
        setIsLoading(false);
      }
    } catch (e) {
      console.error("API Error:", e);
      const errorMsg = e instanceof Error ? e.message : String(e);
      setStatusMessage(`Backend error: ${errorMsg}`);
      setIsLoading(false);
      setIsCheckingDeps(false);
      setDepsInstalled(false);
      setVersion("Error");
      setShortcutVersion(null);
    }
  };

  const callApi = async (apiMethod: () => Promise<any>, loadingMsg: string) => {
    setStatusMessage(loadingMsg);
    try {
      if (!window.pywebview) {
        setStatusMessage("PyWebView API not available (dev mode)");
        return;
      }

      const result = await apiMethod();

      if (result && !result.success) {
        setStatusMessage("Operation failed.");
      }

      await fetchStatus();
    } catch (e) {
      setStatusMessage("Operation failed.");
      console.error("API Error:", e);
    }
  };

  // Fetch models from backend
  const fetchModels = async () => {
    try {
      if (window.pywebview?.api?.get_models) {
        const result = await window.pywebview.api.get_models();
        if (result.success && result.models) {
          // Transform backend models to frontend ModelCategory structure
          const categorizedModels: ModelCategory[] = [];
          const categoryMap = new Map<string, ModelInfo[]>();

          // Group models by category
          const modelEntries = Object.entries(result.models);
          modelEntries.forEach(([path, modelData]: [string, any]) => {
            const category = modelData.modelType || 'uncategorized';
            const fileName = path.split('/').pop() || path;
            const displayName = modelData.officialName || modelData.cleanedName || fileName;

            const modelInfo: ModelInfo = {
              id: path,
              name: displayName,
              category: category,
              path: path,
              size: modelData.size,
              date: modelData.addedDate,
            };

            if (!categoryMap.has(category)) {
              categoryMap.set(category, []);
            }
            categoryMap.get(category)!.push(modelInfo);
          });

          // Convert map to array format
          categoryMap.forEach((models, category) => {
            categorizedModels.push({ category, models });
          });

          setModelGroups(categorizedModels);
          modelCountRef.current = modelEntries.length;
        }
      }
    } catch (e) {
      console.error("Failed to fetch models:", e);
    }
  };

  // --- Effects ---
  useEffect(() => {
    const waitForPyWebView = () => {
      if (window.pywebview && window.pywebview.api && typeof window.pywebview.api.get_status === 'function') {
        console.log('PyWebView API ready with methods, initializing...');
        fetchStatus(true).catch(err => {
          console.error("Initial fetchStatus failed:", err);
          setStatusMessage("Failed to connect to backend");
          setIsLoading(false);
          setIsCheckingDeps(false);
          setDepsInstalled(false);
          setVersion("Error");
        });

        checkLauncherVersion(true);
        fetchModels();
      } else {
        console.log('Waiting for PyWebView API methods...');
        setTimeout(waitForPyWebView, 100);
      }
    };

    waitForPyWebView();
  }, []);

  useEffect(() => {
    const pollModelLibrary = async () => {
      if (isModelCountPolling.current || !window.pywebview?.api?.scan_shared_storage) {
        return;
      }

      isModelCountPolling.current = true;
      try {
        const result = await window.pywebview.api.scan_shared_storage();
        if (result.success) {
          const modelsFound = result.result?.modelsFound;
          if (typeof modelsFound === 'number') {
            if (modelCountRef.current === null) {
              modelCountRef.current = modelsFound;
            } else if (modelsFound !== modelCountRef.current) {
              modelCountRef.current = modelsFound;
              await fetchModels();
            }
          }
        }
      } catch (err) {
        console.error('Failed to poll model library count:', err);
      } finally {
        isModelCountPolling.current = false;
      }
    };

    const interval = setInterval(() => {
      void pollModelLibrary();
    }, 10000);

    return () => clearInterval(interval);
  }, []);

  const checkLauncherVersion = async (forceRefresh = false) => {
    try {
      if (!window.pywebview?.api) return;

      const versionResult = await window.pywebview.api.get_launcher_version();
      if (versionResult.success) {
        setLauncherVersion(versionResult.version);
      }

      const updateResult = await window.pywebview.api.check_launcher_updates(forceRefresh);
      if (updateResult.success) {
        setLauncherUpdateAvailable(updateResult.hasUpdate);
        setLastLauncherUpdateCheckAt(Date.now());
      }
      setUpdateCheckDone(true);
      return updateResult;
    } catch (err) {
      console.error('Failed to check launcher version:', err);
      setUpdateCheckDone(true);
      return { success: false, hasUpdate: false };
    } finally {
      setIsCheckingLauncherUpdate(false);
    }
  };

  const handleLauncherUpdate = async () => {
    if (!window.pywebview?.api || isUpdatingLauncher) return;

    const confirmed = confirm(
      'This will update the launcher to the latest version from GitHub.\n\n' +
      'The app will:\n' +
      '1. Pull latest changes from git\n' +
      '2. Update dependencies\n' +
      '3. Rebuild the frontend\n' +
      '4. Restart automatically\n\n' +
      'Continue?'
    );

    if (!confirmed) return;

    const now = Date.now();
    const isStale = !lastLauncherUpdateCheckAt || now - lastLauncherUpdateCheckAt > 5 * 60 * 1000;
    if (isStale) {
      setIsCheckingLauncherUpdate(true);
      const refreshResult = await checkLauncherVersion(true);
      if (!refreshResult?.success) {
        setStatusMessage('Update check failed. Please try again.');
        setIsUpdatingLauncher(false);
        return;
      }
      if (!refreshResult?.hasUpdate) {
        setStatusMessage('Already up to date.');
        setIsUpdatingLauncher(false);
        return;
      }
    }

    setIsUpdatingLauncher(true);
    setStatusMessage('Updating launcher...');

    try {
      const result = await window.pywebview.api.apply_launcher_update();

      if (result.success) {
        setStatusMessage('Update complete! Restarting...');

        setTimeout(async () => {
          await window.pywebview.api.restart_launcher();
        }, 2000);
      } else {
        setStatusMessage(`Update failed: ${result.error || 'Unknown error'}`);
        alert(`Update failed: ${result.error || 'Unknown error'}`);
        setIsUpdatingLauncher(false);
      }
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setStatusMessage(`Update error: ${errorMsg}`);
      alert(`Update error: ${errorMsg}`);
      setIsUpdatingLauncher(false);
    }
  };

  useEffect(() => {
    const pollStatus = async () => {
      if (isPolling.current || !window.pywebview?.api?.get_status) {
        return;
      }

      isPolling.current = true;
      try {
        await fetchStatus(false);
      } catch (err) {
        console.error('Status polling error:', err);
      } finally {
        isPolling.current = false;
      }
    };

    const interval = setInterval(() => {
      void pollStatus();
    }, 500);  // Poll every 0.5 seconds for smooth updates

    return () => clearInterval(interval);
  }, []);


  useEffect(() => {
    if (!launchError) {
      setLaunchErrorFlash(false);
      return;
    }
    const interval = setInterval(() => setLaunchErrorFlash((prev) => !prev), 650);
    return () => clearInterval(interval);
  }, [launchError]);

  useEffect(() => {
    if (!activeVersion || !window.pywebview?.api) {
      return;
    }
    fetchStatus(false);
  }, [activeVersion]);

  // Manage iconState based on installedVersions, comfyUIRunning, and launchError
  // This is the SINGLE source of truth for offline/uninstalled states
  useEffect(() => {
    setApps(prevApps => {
      return prevApps.map(app => {
        if (app.id === 'comfyui') {
          // Determine correct iconState
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
      });
    });
  }, [installedVersions, comfyUIRunning, launchError]);

  // --- Handlers ---
  const handleInstallDeps = async () => {
    if (!window.pywebview) return;

    setIsInstalling(true);
    setStatusMessage("Installing dependencies... Check terminal for password prompt.");
    await callApi(
      () => window.pywebview!.api.install_deps(),
      "Installing dependencies..."
    );
    setIsInstalling(false);
  };

  const openLogPath = async (path: string | null | undefined) => {
    if (!path || !window.pywebview?.api?.open_path) return;
    try {
      await window.pywebview.api.open_path(path);
    } catch (err) {
      console.error("Failed to open log path", err);
    }
  };

  const openModelsRoot = async () => {
    if (!window.pywebview?.api?.open_path) return;
    try {
      await window.pywebview.api.open_path('shared-resources/models');
    } catch (err) {
      console.error("Failed to open models folder", err);
    }
  };

  const closeWindow = () => {
    if (window.pywebview) {
      window.pywebview.api.close_window();
    } else {
      window.close();
    }
  };

  const handleLaunchComfyUI = async () => {
    if (!window.pywebview) return;

    const action = comfyUIRunning ? 'stop' : 'launch';
    setStatusMessage(comfyUIRunning ? "Stopping ComfyUI..." : "Launching ComfyUI...");

    try {
      const result = comfyUIRunning
        ? await window.pywebview.api.stop_comfyui()
        : await window.pywebview.api.launch_comfyui();

      if (result.success) {
        setStatusMessage(comfyUIRunning ? "ComfyUI stopped successfully" : "ComfyUI launched successfully");
        setLaunchError(null);
        setLaunchLogPath(result.log_path || null);
      } else {
        const errMsg = result.error || `Failed to ${action} ComfyUI`;
        setStatusMessage(errMsg);
        setLaunchError(errMsg);
        setLaunchLogPath(result.log_path || null);
      }
    } catch (e) {
      const errMsg = `Error trying to ${action} ComfyUI`;
      setStatusMessage(errMsg);
      setLaunchError(errMsg);
      console.error(`${action === 'launch' ? 'Launch' : 'Stop'} Error:`, e);
    } finally {
      await fetchStatus(false);
      setTimeout(() => fetchStatus(false), 1200);
    }
  };

  // Icon indicator handlers
  const handleLaunchApp = async (appId: string) => {
    if (appId === 'comfyui' && !comfyUIRunning) {
      await handleLaunchComfyUI();
    }
    // Future: Add handlers for other apps here
  };

  const handleStopApp = async (appId: string) => {
    if (appId === 'comfyui' && comfyUIRunning) {
      await handleLaunchComfyUI();
    }
    // Future: Add handlers for other apps here
  };

  const handleOpenLog = async (appId: string) => {
    if (appId === 'comfyui' && launchLogPath) {
      await openLogPath(launchLogPath);
    }
    // Future: Add handlers for other apps here
  };

  // App management handlers
  const handleDeleteApp = (appId: string) => {
    // Prevent deleting ComfyUI (first app)
    if (appId === 'comfyui') {
      console.warn('Cannot delete ComfyUI app');
      return;
    }

    setApps(prevApps => prevApps.filter(app => app.id !== appId));

    // Deselect if deleting selected app
    if (selectedAppId === appId) {
      setSelectedAppId(null);
    }
  };

  const handleReorderApps = (reorderedApps: AppConfig[]) => {
    setApps(reorderedApps);
  };

  const handleAddApp = (insertAtIndex: number) => {
    // Create a new app with default configuration
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

  // Model Manager Handlers
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

  const handleScanModels = async () => {
    try {
      if (window.pywebview?.api?.scan_shared_storage) {
        const result = await window.pywebview.api.scan_shared_storage();
        if (result.success) {
          // Refresh models after scan
          await fetchModels();
          setStatusMessage("Model scan completed");
        } else {
          setStatusMessage(`Scan failed: ${result.error || 'Unknown error'}`);
        }
      }
    } catch (e) {
      console.error("Failed to scan models:", e);
      setStatusMessage("Failed to scan models");
    }
  };

  const handleAddModels = async () => {
    // TODO: Implement folder picker dialog
    // For now, just trigger a scan
    await handleScanModels();
  };

  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;
  const defaultReadyText = statusMessage?.trim().toLowerCase() === 'system ready. configure options below';
  const displayStatus = statusMessage === "Setup complete – everything is ready" || defaultReadyText ? "" : statusMessage;

  return (
    <div className="w-full h-screen gradient-bg-blobs flex flex-col relative overflow-hidden font-mono">
      {/* Header */}
      <Header
        systemResources={systemResources}
        launcherUpdateAvailable={launcherUpdateAvailable}
        onClose={closeWindow}
        cacheStatus={cacheStatus}
        installationProgress={installationProgress}
      />

      {/* Main Layout */}
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
            /* ComfyUI Content - Original Layout */
            <div className="flex-1 p-6 flex flex-col items-center overflow-auto">
              {isCheckingDeps || depsInstalled === null ? (
                <div className="w-full flex items-center justify-center gap-2 text-[hsl(var(--text-secondary))]">
                  <Loader2 className="animate-spin" size={18} />
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
                      onRefreshProgress={fetchInstallationProgress}
                      displayMode="page"
                    />
                  </div>
                </div>
              ) : (
                <>
                  {/* VERSION SELECTOR */}
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
                      onMakeDefault={setDefaultVersion}
                      installingVersion={installingTag}
                      activeShortcutState={{ menu: menuShortcut, desktop: desktopShortcut }}
                      diskSpacePercent={diskSpacePercent}
                      hasNewVersion={hasUpdate}
                      latestVersion={latestVersion}
                    />
                  </div>

                  {/* DEPENDENCY SECTION */}
                  <div className="w-full mb-6 min-h-[50px] flex items-center justify-center">
                    <AnimatePresence mode="wait">
                      {depsInstalled === false ? (
                        <motion.button
                          key="install-btn"
                          layout
                          initial={{ opacity: 0, scale: 0.9 }}
                          animate={{ opacity: 1, scale: 1 }}
                          exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
                          onClick={handleInstallDeps}
                          disabled={isInstalling || comfyUIRunning}
                          className="w-full h-12 bg-[hsl(var(--surface-interactive))] hover:bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--text-secondary))] hover:text-[hsl(var(--text-primary))] font-bold text-sm flex items-center justify-center gap-3 transition-colors active:scale-[0.98] rounded-sm disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                          {isInstalling ? (
                            <>
                              <Loader2 className="animate-spin" size={18} />
                              <span>Installing (Check Terminal)...</span>
                            </>
                          ) : comfyUIRunning ? (
                            <>
                              <ArrowDownToLine size={18} />
                              <span>Stop ComfyUI to Install</span>
                            </>
                          ) : (
                            <>
                              <ArrowDownToLine size={18} />
                              <span>Install Missing Dependencies</span>
                            </>
                          )}
                        </motion.button>
                      ) : null}
                    </AnimatePresence>
                  </div>

                  {/* CONTROL PANEL */}
                  <motion.div
                    className="w-full flex flex-col items-center gap-6"
                    animate={{
                      opacity: depsInstalled ? 1 : 0.3,
                      filter: depsInstalled ? "blur(0px)" : "blur(1px)",
                      pointerEvents: depsInstalled ? "auto" : "none"
                    }}
                    transition={{ duration: 0.4 }}
                  >
                    {displayStatus && (
                      <div className="h-6 text-center w-full px-2">
                        <span
                          className={`text-sm italic font-medium transition-colors duration-300 block truncate ${
                            comfyUIRunning ? 'text-[hsl(var(--accent-success))]' : (isSetupComplete ? 'text-[hsl(var(--accent-success))]' : 'text-[hsl(var(--text-tertiary))]')
                          }`}
                        >
                          {displayStatus}
                        </span>
                      </div>
                    )}
                  </motion.div>
                </>
              )}
            </div>
          ) : (
            /* Other Apps - Show Model Manager */
            <div className="flex-1 flex flex-col gap-4 p-8 px-0 mx-2 py-1 overflow-hidden">
              {selectedAppId && (
                <div className="text-center py-4">
                  <p className="text-[hsl(var(--launcher-text-secondary))] text-sm">
                    {`${apps.find(a => a.id === selectedAppId)?.displayName} - Coming Soon`}
                  </p>
                </div>
              )}

              {/* Model Manager for non-ComfyUI apps */}
              <ModelManager
                modelGroups={modelGroups}
                starredModels={starredModels}
                linkedModels={linkedModels}
                onToggleStar={handleToggleStar}
                onToggleLink={handleToggleLink}
                selectedAppId={selectedAppId}
                onAddModels={handleAddModels}
                onOpenModelsRoot={openModelsRoot}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
