import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Terminal, ArrowDownToLine, Monitor, Menu, Loader2, ArrowLeft, RefreshCw, Play, Square } from 'lucide-react';
import { SpringyToggle } from './components/SpringyToggle';
import { VersionSelector } from './components/VersionSelector';
import { InstallDialog } from './components/InstallDialog';
import { useVersions } from './hooks/useVersions';

// TypeScript definitions for PyWebView API
declare global {
  interface Window {
    pywebview?: {
      api: {
        // Original API methods
        get_status: () => Promise<any>;
        install_deps: () => Promise<{ success: boolean }>;
        toggle_menu: () => Promise<{ success: boolean }>;
        toggle_desktop: () => Promise<{ success: boolean }>;
        close_window: () => Promise<{ success: boolean }>;
        launch_comfyui: () => Promise<{ success: boolean }>;
        stop_comfyui: () => Promise<{ success: boolean }>;

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
        launch_version: (tag: string, extra_args?: string[]) => Promise<{ success: boolean; error?: string }>;

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
      };
    };
  }
}

export default function App() {
  // --- State ---
  const [version, setVersion] = useState("Loading...");
  const [depsInstalled, setDepsInstalled] = useState<boolean | null>(null); // null means not checked yet
  const [isInstalling, setIsInstalling] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isCheckingDeps, setIsCheckingDeps] = useState(true);

  // App States
  const [isPatched, setIsPatched] = useState(false);
  const [menuShortcut, setMenuShortcut] = useState(false);
  const [desktopShortcut, setDesktopShortcut] = useState(false);
  const [statusMessage, setStatusMessage] = useState("Checking system status...");

  // Release info
  const [hasUpdate, setHasUpdate] = useState(false);
  const [latestVersion, setLatestVersion] = useState<string | null>(null);

  // ComfyUI running state
  const [comfyUIRunning, setComfyUIRunning] = useState(false);
  const [showVersionManager, setShowVersionManager] = useState(false);
  const [isRefreshingVersions, setIsRefreshingVersions] = useState(false);
  const [isLaunchHover, setIsLaunchHover] = useState(false);
  const [launcherVersion, setLauncherVersion] = useState<string | null>(null);
  const [spinnerFrame, setSpinnerFrame] = useState(0);

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
  } = useVersions();

  // --- API Helpers ---
  const fetchStatus = async (isInitialLoad = false) => {
    const startTime = Date.now();

    // Only show checking animation on initial load
    if (isInitialLoad) {
      setIsCheckingDeps(true);
    }

    try {
      // Use PyWebView API if available, otherwise fall back to fetch (dev mode)
      let data;
      if (window.pywebview) {
        data = await window.pywebview.api.get_status();
      } else {
        // Development mode fallback
        setStatusMessage("Running in development mode - PyWebView API not available");
        setIsLoading(false);
        setIsCheckingDeps(false);
        setDepsInstalled(false);
        setVersion("Dev Mode");
        return;
      }

      setVersion(data.version);
      setDepsInstalled(data.deps_ready);
      setIsPatched(data.patched);
      setMenuShortcut(data.menu_shortcut);
      setDesktopShortcut(data.desktop_shortcut);
      setStatusMessage(data.message);
      setComfyUIRunning(data.comfyui_running || false);

      // Handle release info
      if (data.release_info) {
        setHasUpdate(Boolean(data.release_info.has_update));
        setLatestVersion(data.release_info.latest_version);
      }

      // Ensure loading indicator shows for at least 800ms for better UX on initial load
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

      // Capture launcher version as short string (fallback to existing version state)
      const shortVersion = data?.launcher_version || data?.version || null;
      setLauncherVersion(shortVersion);
    } catch (e) {
      console.error("API Error:", e);
      const errorMsg = e instanceof Error ? e.message : String(e);
      setStatusMessage(`Backend error: ${errorMsg}`);
      setIsLoading(false);
      setIsCheckingDeps(false);
      setDepsInstalled(false);
      setVersion("Error");
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

      // Refresh state after action
      await fetchStatus();
    } catch (e) {
      setStatusMessage("Operation failed.");
      console.error("API Error:", e);
    }
  };

  // --- Effects ---

  // Initial load effect - runs once on mount
  useEffect(() => {
    // Wait for PyWebView API to be ready with actual methods
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
      } else {
        console.log('Waiting for PyWebView API methods...');
        setTimeout(waitForPyWebView, 100);
      }
    };

    waitForPyWebView();
  }, []); // Empty dependency array - runs only once on mount

  // Polling effect - only polls when ComfyUI is running
  useEffect(() => {
    if (!comfyUIRunning) {
      return; // Don't set up interval if ComfyUI is not running
    }

    // Poll every 5 seconds while ComfyUI is running
    const interval = setInterval(() => {
      if (window.pywebview && window.pywebview.api) {
        fetchStatus(false);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [comfyUIRunning]); // Re-run when comfyUIRunning changes

  // Spinner frame updater for running state
  useEffect(() => {
    if (comfyUIRunning) {
      setSpinnerFrame(0);
    }

    if (!comfyUIRunning || isLaunchHover) {
      return;
    }

    const interval = setInterval(() => {
      setSpinnerFrame((prev) => (prev + 1) % spinnerFrames.length);
    }, 180);

    return () => clearInterval(interval);
  }, [comfyUIRunning, isLaunchHover]);

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

  const toggleMenu = () => {
    if (!window.pywebview) return;

    callApi(
      () => window.pywebview!.api.toggle_menu(),
      menuShortcut ? "Removing menu shortcut..." : "Creating menu shortcut..."
    );
  };

  const toggleDesktop = () => {
    if (!window.pywebview) return;

    callApi(
      () => window.pywebview!.api.toggle_desktop(),
      desktopShortcut ? "Removing desktop shortcut..." : "Creating desktop shortcut..."
    );
  };

  const closeWindow = () => {
    if (window.pywebview) {
      window.pywebview.api.close_window();
    } else {
      // Development mode fallback
      window.close();
    }
  };

  const handleLaunchComfyUI = async () => {
    if (!window.pywebview) return;

    if (comfyUIRunning) {
      // Stop ComfyUI
      setStatusMessage("Stopping ComfyUI...");
      try {
        const result = await window.pywebview.api.stop_comfyui();
        if (result.success) {
          setStatusMessage("ComfyUI stopped successfully");
          setComfyUIRunning(false);
        } else {
          setStatusMessage("Failed to stop ComfyUI");
        }
      } catch (e) {
        setStatusMessage("Error stopping ComfyUI");
        console.error("Stop Error:", e);
      }
    } else {
      // Launch ComfyUI
      setStatusMessage("Launching ComfyUI...");
      try {
        const result = await window.pywebview.api.launch_comfyui();
        if (result.success) {
          setStatusMessage("ComfyUI launched successfully");
          setComfyUIRunning(true);
        } else {
          setStatusMessage("Failed to launch ComfyUI");
        }
      } catch (e) {
        setStatusMessage("Error launching ComfyUI");
        console.error("Launch Error:", e);
      }
    }

    // Refresh status after a moment
    setTimeout(() => fetchStatus(false), 1000);
  };

  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;
  const defaultReadyText = statusMessage?.trim().toLowerCase() === 'system ready. configure options below';
  const displayStatus = statusMessage === "Setup complete – everything is ready" || defaultReadyText ? "" : statusMessage;
  const activeVersionLabel = activeVersion || 'No version';
  const canLaunch = depsInstalled === true && installedVersions.length > 0;
  const launchSubText = canLaunch ? activeVersionLabel : 'No version installed';
  const idleIconGlow = !comfyUIRunning && canLaunch ? { filter: 'drop-shadow(0 0 6px #55ff55)' } : undefined;
  const spinnerFrames = ['/', '-', '\\', '|'];

  return (
    <div className="w-full h-full bg-[#1e1e1e] shadow-2xl overflow-auto flex flex-col relative font-sans selection:bg-gray-700">

      {/* Title Bar */}
      <div className="sticky top-0 z-20 h-14 bg-[#252525] flex items-center justify-between px-6 select-none border-b border-[#333] shadow-sm">
        <div className="flex items-center gap-4 h-full">
          <div className="flex flex-col justify-center h-full">
            <h1 className="text-white text-base font-semibold leading-tight">ComfyUI Setup</h1>
            <span className="text-[#aaaaaa] text-[11px] flex items-center gap-2">
              Launcher: {launcherVersion || 'dev'}
              {isLoading && <Loader2 size={12} className="animate-spin" />}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <motion.button
            onMouseEnter={() => setIsLaunchHover(true)}
            onMouseLeave={() => setIsLaunchHover(false)}
            onClick={handleLaunchComfyUI}
            disabled={!canLaunch}
            className={`flex items-center justify-center gap-3 w-[128px] h-[48px] rounded border text-xs font-semibold transition-colors ${
              !canLaunch
                ? 'bg-[#333]/50 border-[#444] text-[#666] cursor-not-allowed'
                : comfyUIRunning
                  ? 'bg-[#55ff55]/10 hover:bg-[#55ff55]/20 border-[#55ff55] text-[#dfffd3]'
                  : 'bg-[#2c2c2c] hover:bg-[#333] border-[#555] text-[#e6e6e6]'
            }`}
            whileHover={{ scale: canLaunch ? 1.04 : 1 }}
            whileTap={{ scale: canLaunch ? 0.98 : 1 }}
          >
            <div className="w-5 flex items-center justify-center">
              {comfyUIRunning ? (
                isLaunchHover ? (
                  <Square
                    size={18}
                    className="flex-shrink-0 text-[#ff6666]"
                    fill="currentColor"
                    stroke="currentColor"
                    strokeWidth={1}
                  />
                ) : (
                  <span className="flex-shrink-0 w-4 text-center font-mono text-[15px]">
                    {spinnerFrames[spinnerFrame]}
                  </span>
                )
              ) : (
                <Play size={20} className="flex-shrink-0 text-[#55ff55]" style={idleIconGlow} />
              )}
            </div>
            <div className="flex flex-col items-start leading-tight w-[80px]">
              <span className="text-[13px] leading-tight font-semibold">
                {comfyUIRunning ? (isLaunchHover ? 'Stop' : 'Running') : 'Launch'}
              </span>
              <span className="text-[10px] mt-0.5 truncate w-full">
                {launchSubText}
              </span>
            </div>
          </motion.button>
          <div className="h-14 w-14 flex items-center justify-center">
            <div
              onClick={closeWindow}
              className="cursor-pointer group p-2 rounded hover:bg-[#333] transition-colors"
            >
              <X className="text-[#cccccc] group-hover:text-[#ff4444] transition-colors" size={22} />
            </div>
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 p-6 flex flex-col items-center">
        {isCheckingDeps || depsInstalled === null ? (
          <div className="w-full flex items-center justify-center gap-2 text-gray-400">
            <Loader2 className="animate-spin" size={18} />
            <span className="text-sm">Checking Dependencies...</span>
          </div>
        ) : showVersionManager ? (
          <div className="w-full flex-1 flex flex-col gap-4">
            <div className="w-full flex items-center justify-between">
              <button
                onClick={() => setShowVersionManager(false)}
                className="flex items-center gap-2 px-3 py-2 rounded border border-[#333] bg-[#2a2a2a] hover:bg-[#333] text-white text-sm transition-colors"
              >
                <ArrowLeft size={14} />
                <span>Back to setup</span>
              </button>
              <div className="flex items-center gap-3 text-xs text-gray-400">
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
                  className="p-2 rounded hover:bg-[#333] transition-colors disabled:opacity-50"
                  whileHover={{ scale: isRefreshingVersions || isVersionLoading ? 1 : 1.05 }}
                  whileTap={{ scale: isRefreshingVersions || isVersionLoading ? 1 : 0.96 }}
                  title="Refresh versions"
                >
                  <RefreshCw size={14} className={isRefreshingVersions ? 'animate-spin text-gray-500' : 'text-gray-300'} />
                </motion.button>
              </div>
            </div>
            <div className="w-full flex-1 min-h-0">
              <InstallDialog
                isOpen={showVersionManager}
                onClose={() => setShowVersionManager(false)}
                availableVersions={availableVersions}
                installedVersions={installedVersions}
                isLoading={isVersionLoading}
                onInstallVersion={installVersion}
                onRemoveVersion={removeVersion}
                onRefreshAll={refreshAll}
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
              />
            </div>

            {/* DEPENDENCY SECTION */}
            <div className="w-full mb-6 min-h-[50px] flex items-center justify-center">
              <AnimatePresence mode="wait">
                {depsInstalled === false ? (
                  /* MISSING STATE: Big Wide Button */
                  <motion.button
                    key="install-btn"
                    layout
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    exit={{ opacity: 0, scale: 0.5, transition: { duration: 0.2 } }}
                    onClick={handleInstallDeps}
                    disabled={isInstalling || comfyUIRunning}
                    className="w-full h-12 bg-[#333333] hover:bg-[#444444] text-[#aaaaaa] hover:text-white font-bold text-sm flex items-center justify-center gap-3 transition-colors active:scale-[0.98] rounded-sm disabled:opacity-50 disabled:cursor-not-allowed"
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

            {/* Status Footer Text */}
              {displayStatus && (
                <div className="h-6 text-center w-full px-2">
                  <span
                    className={`text-sm italic font-medium transition-colors duration-300 block truncate ${
                      comfyUIRunning ? 'text-[#55ff55]' : (isSetupComplete ? 'text-[#55ff55]' : 'text-[#666666]')
                    }`}
                  >
                    {displayStatus}
                  </span>
                </div>
              )}

            {/* Toggles */}
            <div className="flex flex-col gap-4">
              <div className="flex items-center gap-3">
                {isLoading ? (
                  <Loader2 size={14} className="text-gray-400 animate-spin" />
                ) : (
                  <Menu size={16} className="text-[#555]" />
                )}
                <SpringyToggle
                  isOn={menuShortcut}
                  onToggle={toggleMenu}
                  disabled={isLoading}
                  labelOff="No Shortcut"
                  labelOn="Menu Shortcut"
                />
              </div>

              <div className="flex items-center gap-3">
                {isLoading ? (
                  <Loader2 size={14} className="text-gray-400 animate-spin" />
                ) : (
                  <Monitor size={16} className="text-[#555]" />
                )}
                <SpringyToggle
                  isOn={desktopShortcut}
                  onToggle={toggleDesktop}
                  disabled={isLoading}
                  labelOff="No Shortcut"
                  labelOn="Desktop Shortcut"
                />
              </div>
            </div>

          </motion.div>
          </>
        )}
      </div>
    </div>
  );
}
