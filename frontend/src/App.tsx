import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, X, Terminal, ArrowDownToLine, Monitor, Menu, RefreshCw, Loader2 } from 'lucide-react';
import { SpringyToggle } from './components/SpringyToggle';
import { VersionSelector } from './components/VersionSelector';

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
  const displayStatus = statusMessage === "Setup complete – everything is ready" ? "" : statusMessage;

  return (
    <div className="w-full h-full bg-[#1e1e1e] shadow-2xl overflow-hidden flex flex-col relative font-sans selection:bg-gray-700">

      {/* Title Bar */}
      <div className="h-[72px] bg-[#252525] flex items-center justify-between px-6 select-none border-b border-[#333]">
        <div className="flex flex-col justify-center h-full">
          <h1 className="text-white text-lg font-bold mt-2">ComfyUI Setup</h1>
          <span className="text-[#aaaaaa] text-xs mb-2 flex items-center gap-2">
            Version: {version}
            {isLoading && <Loader2 size={12} className="animate-spin" />}
            {hasUpdate && latestVersion && (
              <motion.span
                initial={{ opacity: 0, scale: 0.8 }}
                animate={{ opacity: 1, scale: 1 }}
                className="text-[#55ff55] text-xs font-semibold"
              >
                (New: {latestVersion})
              </motion.span>
            )}
            {!hasUpdate && latestVersion && (
              <span className="text-[#666666] text-xs">
                (Latest: {latestVersion})
              </span>
            )}
          </span>
        </div>
        <div
          onClick={closeWindow}
          className="cursor-pointer group p-2 rounded hover:bg-[#333] transition-colors"
        >
          <X className="text-[#cccccc] group-hover:text-[#ff4444] transition-colors" size={24} />
        </div>
      </div>

        {/* Main Content */}
        <div className="flex-1 p-6 flex flex-col items-center">

        {isCheckingDeps || depsInstalled === null ? (
          <div className="w-full flex items-center justify-center gap-2 text-gray-400">
            <Loader2 className="animate-spin" size={18} />
            <span className="text-sm">Checking Dependencies...</span>
          </div>
        ) : (
          <>
        {/* VERSION SELECTOR */}
        <div className="w-full mb-4">
          <VersionSelector />
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

          {/* Launch/Stop ComfyUI Button */}
          <motion.button
            onClick={handleLaunchComfyUI}
            disabled={depsInstalled !== true}
            className={`w-full mt-4 h-12 border font-bold text-sm flex items-center justify-center gap-3 transition-colors active:scale-[0.98] rounded-sm ${
              depsInstalled !== true
                ? 'bg-[#333]/50 border-[#444] text-[#666] cursor-not-allowed'
                : comfyUIRunning
                  ? 'bg-[#ff4444]/10 hover:bg-[#ff4444]/20 border-[#ff4444] text-[#ff4444] hover:text-white'
                  : 'bg-[#55ff55]/10 hover:bg-[#55ff55]/20 border-[#55ff55] text-[#55ff55] hover:text-white'
            }`}
            whileHover={{ scale: depsInstalled === true ? 1.02 : 1 }}
            whileTap={{ scale: depsInstalled === true ? 0.98 : 1 }}
          >
            <Terminal size={18} />
            <span>{comfyUIRunning ? 'Stop ComfyUI' : 'Launch ComfyUI'}</span>
          </motion.button>

        </motion.div>
        </>        
        )}
      </div>
    </div>
  );
}
