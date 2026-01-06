import React, { useState, useEffect, useRef } from 'react';
import { X, ArrowUp, ChevronDown, Download, FolderOpen, Anchor } from 'lucide-react';
import { StatusFooter } from './components/StatusFooter';
import { AppSidebar } from './components/AppSidebar';
import { ResourceMonitor, DiskMonitor } from './components/ResourceMonitor';
import { ModelManager } from './components/ModelManager';
import { useVersions } from './hooks/useVersions';
import { DEFAULT_APPS } from './config/apps';
import type { AppConfig, ModelCategory, SystemResources } from './types/apps';

// TypeScript definitions for PyWebView API
declare global {
  interface Window {
    pywebview?: {
      api: {
        // Original API methods
        get_status: () => Promise<{
          success: boolean;
          version: string;
          deps_ready: boolean;
          patched: boolean;
          menu_shortcut: boolean;
          desktop_shortcut: boolean;
          shortcut_version: string | null;
          message: string;
          comfyui_running: boolean;
          last_launch_error: string | null;
          last_launch_log: string | null;
          release_info?: { has_update: boolean; latest_version: string };
          error?: string;
        }>;
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
        toggle_version_menu: (tag: string) => Promise<{ success: boolean; state: { menu: boolean; desktop: boolean; tag: string }; error?: string }>;
        toggle_version_desktop: (tag: string) => Promise<{ success: boolean; state: { menu: boolean; desktop: boolean; tag: string }; error?: string }>;

        // Version Management API (Phase 5)
        get_available_versions: (force_refresh?: boolean) => Promise<{ success: boolean; versions: Array<{ tag_name: string; published_at: string; html_url: string }>; error?: string }>;
        get_installed_versions: () => Promise<{ success: boolean; versions: string[]; error?: string }>;
        validate_installations: () => Promise<{ success: boolean; result: { had_invalid: boolean; removed: string[]; valid: string[] }; error?: string }>;
        get_installation_progress: () => Promise<{
          success: boolean;
          installing: boolean;
          tag: string | null;
          phase: string | null;
          percent: number;
          message: string;
          error?: string;
        }>;
        install_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        cancel_installation: () => Promise<{ success: boolean; error?: string }>;
        remove_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        switch_version: (tag: string) => Promise<{ success: boolean; error?: string }>;
        get_active_version: () => Promise<{ success: boolean; version: string; error?: string }>;
        check_version_dependencies: (tag: string) => Promise<{ success: boolean; dependencies: { satisfied: boolean; missing: string[] }; error?: string }>;
        install_version_dependencies: (tag: string) => Promise<{ success: boolean; error?: string }>;
        get_version_status: () => Promise<{ success: boolean; status: { active: string | null; installed: string[] }; error?: string }>;
        get_version_info: (tag: string) => Promise<{ success: boolean; info: { tag: string; path: string; exists: boolean }; error?: string }>;
        launch_version: (tag: string, extra_args?: string[]) => Promise<{ success: boolean; error?: string; log_path?: string; ready?: boolean }>;
        get_default_version: () => Promise<{ success: boolean; version: string; error?: string }>;
        set_default_version: (tag?: string | null) => Promise<{ success: boolean; error?: string }>;

        // Size Calculation API (Phase 6.2.5c)
        calculate_release_size: (tag: string, force_refresh?: boolean) => Promise<{
          success: boolean;
          size_mb: number;
          cached: boolean;
          error?: string;
        }>;
        calculate_all_release_sizes: () => Promise<{
          success: boolean;
          sizes: Record<string, number>;
          error?: string;
        }>;

        // Utility
        open_url: (url: string) => Promise<{ success: boolean; error?: string }>;

        // Resource Management API (Phase 5)
        get_models: () => Promise<{ success: boolean; models: Record<string, unknown>; error?: string }>;
        get_custom_nodes: (version_tag: string) => Promise<{ success: boolean; nodes: string[]; error?: string }>;
        install_custom_node: (git_url: string, version_tag: string, node_name?: string) => Promise<{ success: boolean; error?: string }>;
        update_custom_node: (node_name: string, version_tag: string) => Promise<{ success: boolean; error?: string }>;
        remove_custom_node: (node_name: string, version_tag: string) => Promise<{ success: boolean; error?: string }>;
        scan_shared_storage: () => Promise<{ success: boolean; result: Record<string, unknown>; error?: string }>;

        // Launcher Update API
        get_launcher_version: () => Promise<{ success: boolean; version: string; branch: string; isGitRepo: boolean; error?: string }>;
        check_launcher_updates: (force_refresh?: boolean) => Promise<{ success: boolean; hasUpdate: boolean; currentCommit: string; latestCommit: string; commitsBehind: number; commits: Array<{ sha: string; message: string; author: string; date: string }>; error?: string }>;
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

  // --- Legacy State (maintained for ComfyUI compatibility) ---
  const [depsInstalled, setDepsInstalled] = useState<boolean | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);
  const [isPatched, setIsPatched] = useState(false);
  const [menuShortcut, setMenuShortcut] = useState(false);
  const [desktopShortcut, setDesktopShortcut] = useState(false);
  const [statusMessage, setStatusMessage] = useState("Checking system status...");
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [launchLogPath, setLaunchLogPath] = useState<string | null>(null);
  const [comfyUIRunning, setComfyUIRunning] = useState(false);
  const [launcherVersion, setLauncherVersion] = useState<string | null>(null);
  const isPolling = useRef(false);
  const [launcherUpdateAvailable, setLauncherUpdateAvailable] = useState(false);
  const [isUpdatingLauncher, setIsUpdatingLauncher] = useState(false);
  const [updateCheckDone, setUpdateCheckDone] = useState(false);
  const [isCheckingLauncherUpdate, setIsCheckingLauncherUpdate] = useState(false);
  const [lastLauncherUpdateCheckAt, setLastLauncherUpdateCheckAt] = useState<number | null>(null);
  const [diskSpacePercent, setDiskSpacePercent] = useState(0);

  // Model Manager State
  const [modelGroups, setModelGroups] = useState<ModelCategory[]>([]);
  const [starredModels, setStarredModels] = useState<Set<string>>(new Set());
  const [linkedModels, setLinkedModels] = useState<Set<string>>(new Set());
  const [versionDropdownOpen, setVersionDropdownOpen] = useState(false);

  // Version data (shared between selector and manager view)
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

  // --- API Helpers ---
  const fetchSystemResources = async () => {
    try {
      if (window.pywebview?.api?.get_system_resources) {
        const result = await window.pywebview.api.get_system_resources();
        if (result.success) {
          setSystemResources(result.resources);
        }
      }
    } catch (e) {
      console.error("Failed to fetch system resources:", e);
    }
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
        return;
      }

      setDepsInstalled(data.deps_ready);
      setIsPatched(data.patched);
      setMenuShortcut(data.menu_shortcut);
      setDesktopShortcut(data.desktop_shortcut);
      setStatusMessage(data.message);
      setComfyUIRunning(data.comfyui_running || false);
      setLaunchError(data.last_launch_error || null);
      setLaunchLogPath(data.last_launch_log || null);

      await fetchDiskSpace();
      await fetchSystemResources();

      // Update app status based on ComfyUI state
      setApps(prevApps => prevApps.map(app => {
        if (app.id === 'comfyui') {
          return {
            ...app,
            status: data.comfyui_running ? 'running' : (data.deps_ready ? 'idle' : 'idle'),
            iconState: data.comfyui_running ? 'running' : (installedVersions.length > 0 ? 'offline' : 'uninstalled'),
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
    }
  };

  const callApi = async (apiMethod: () => Promise<{ success: boolean }>, loadingMsg: string) => {
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
      } else {
        console.log('Waiting for PyWebView API methods...');
        setTimeout(waitForPyWebView, 100);
      }
    };

    waitForPyWebView();
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
    }, 4000);

    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (!activeVersion || !window.pywebview?.api) {
      return;
    }
    fetchStatus(false);
  }, [activeVersion]);

  // --- Handlers ---
  const handleInstallDeps = async () => {
    if (!window.pywebview) return;

    setStatusMessage("Installing dependencies... Check terminal for password prompt.");
    await callApi(
      () => window.pywebview!.api.install_deps(),
      "Installing dependencies..."
    );
  };

  const openLogPath = async (path: string | null | undefined) => {
    if (!path || !window.pywebview?.api?.open_path) return;
    try {
      await window.pywebview.api.open_path(path);
    } catch (err) {
      console.error("Failed to open log path", err);
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

  const activeVersionLabel = activeVersion || 'No version';

  return (
    <div className="w-full h-screen bg-[hsl(var(--launcher-bg-primary))] flex flex-col relative overflow-hidden font-mono">
      {/* Header */}
      <div className="border-b border-[hsl(var(--launcher-border))] px-8 py-4 flex justify-between bg-[hsl(var(--launcher-bg-secondary)/0.5)] relative z-10 items-start gap-4">
        <ResourceMonitor resources={systemResources} />

        <div className="flex-1 flex justify-center">
          <DiskMonitor diskFree={systemResources?.disk.free || diskSpacePercent} />
        </div>

        <div className="flex items-center gap-4">
          <div>
            <div className="flex items-center gap-1">
              <span className="text-xs text-[hsl(var(--launcher-text-secondary))]">{launcherVersion || 'dev'}</span>
              <ArrowUp className="w-3 h-3 text-[hsl(var(--launcher-text-muted))]" />
            </div>
          </div>

          <button
            className="p-2 rounded hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
            title="Close application"
            onClick={closeWindow}
          >
            <X className="w-5 h-5 text-[hsl(var(--launcher-text-secondary))] hover:text-[hsl(var(--launcher-text-primary))]" />
          </button>
        </div>
      </div>

      {/* Main Layout */}
      <div className="flex flex-1 relative z-10">
        <AppSidebar
          apps={apps}
          selectedAppId={selectedAppId}
          onSelectApp={setSelectedAppId}
        />

        <div className="flex-1 flex flex-col bg-[hsl(var(--launcher-bg-primary))]">
          <div className="flex-1 flex flex-col gap-4 p-8 px-0 mx-2 py-1">
            {/* Version Selector */}
            <div className="relative">
              <button
                onClick={() => setVersionDropdownOpen(!versionDropdownOpen)}
                className="w-full px-4 py-2.5 bg-[hsl(var(--launcher-bg-tertiary)/0.4)] border border-[hsl(var(--launcher-border))] rounded-full hover:border-[hsl(var(--launcher-border)/0.8)] transition-colors text-left text-white flex items-center justify-between group"
              >
                <div className="flex items-center gap-2 flex-1">
                  <Anchor
                    onClick={(e) => {
                      e.stopPropagation();
                      if (activeVersion) {
                        setDefaultVersion(activeVersion);
                      }
                    }}
                    className={`w-4 h-4 transition-colors cursor-pointer ${
                      defaultVersion === activeVersion
                        ? 'text-[hsl(var(--launcher-accent-primary))]'
                        : 'text-[hsl(var(--launcher-accent-primary)/0.7)]'
                    }`}
                  />
                  <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                    {activeVersionLabel}
                  </span>
                </div>
                <div className="flex items-center gap-1">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      // TODO: Open version manager when implemented
                    }}
                    className="p-1 rounded hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                    title="Install new version"
                  >
                    <Download className="w-4 h-4 text-[hsl(var(--launcher-accent-primary)/0.7)] hover:text-[hsl(var(--launcher-accent-primary))]" />
                  </button>

                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      openActiveInstall();
                    }}
                    className="p-1 rounded hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors"
                    title="Open folder"
                  >
                    <FolderOpen className="w-4 h-4 text-[hsl(var(--launcher-accent-primary)/0.7)] hover:text-[hsl(var(--launcher-accent-primary))]" />
                  </button>

                  <ChevronDown
                    className={`w-4 h-4 text-[hsl(var(--launcher-text-secondary))] transition-transform ${
                      versionDropdownOpen ? 'rotate-180' : ''
                    }`}
                  />
                </div>
              </button>
            </div>

            {/* Model Manager */}
            <ModelManager
              modelGroups={modelGroups}
              starredModels={starredModels}
              linkedModels={linkedModels}
              onToggleStar={handleToggleStar}
              onToggleLink={handleToggleLink}
              selectedAppId={selectedAppId}
            />
          </div>
        </div>
      </div>

      {/* Status Footer */}
      <StatusFooter
        cacheStatus={cacheStatus}
        installationProgress={installationProgress}
      />
    </div>
  );
}
