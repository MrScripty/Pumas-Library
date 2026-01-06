import { useState, useEffect, useCallback, useRef } from 'react';

export interface VersionRelease {
  tag_name: string;
  name: string;
  published_at: string;
  prerelease: boolean;
  body?: string;
  html_url?: string;
  total_size?: number | null;
  archive_size?: number | null;
  dependencies_size?: number | null;
  installing?: boolean;
}

export interface VersionStatus {
  installedCount: number;
  activeVersion: string | null;
  defaultVersion?: string | null;
  versions: {
    [tag: string]: {
      isActive: boolean;
      dependencies: {
        installed: string[];
        missing: string[];
      };
    };
  };
}

export interface VersionInfo {
  path: string;
  installedDate: string;
  pythonVersion: string;
  releaseTag: string;
}

export interface InstallationProgress {
  tag: string;
  started_at: string;
  stage: 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress: number;
  overall_progress: number;
  current_item: string | null;
  download_speed: number | null;
  eta_seconds: number | null;
  total_size: number | null;
  downloaded_bytes: number;
  dependency_count: number | null;
  completed_dependencies: number;
  completed_items: Array<{
    name: string;
    type: string;
    size: number | null;
    completed_at: string;
  }>;
  error: string | null;
  completed_at?: string;
  success?: boolean;
  log_path?: string | null;
}

export type InstallNetworkStatus = 'idle' | 'downloading' | 'stalled' | 'failed';

export interface CacheStatus {
  has_cache: boolean;
  is_valid: boolean;
  is_fetching: boolean;
  age_seconds?: number;
  last_fetched?: string;
  releases_count?: number;
}

export function useVersions() {
  const [installedVersions, setInstalledVersions] = useState<string[]>([]);
  const [activeVersion, setActiveVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<VersionRelease[]>([]);
  const [versionStatus, setVersionStatus] = useState<VersionStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [installingTag, setInstallingTag] = useState<string | null>(null);
  const [installationProgress, setInstallationProgress] = useState<InstallationProgress | null>(null);
  const [defaultVersion, setDefaultVersionState] = useState<string | null>(null);
  const installPollRef = useRef<NodeJS.Timeout | null>(null);
  const followupRefreshRef = useRef<NodeJS.Timeout | null>(null);
  const [installNetworkStatus, setInstallNetworkStatus] = useState<InstallNetworkStatus>('idle');
  const lastDownloadRef = useRef<{ bytes: number; speed: number; ts: number }>({ bytes: 0, speed: 0, ts: 0 });
  const lastDownloadTagRef = useRef<string | null>(null);
  const topSpeedRef = useRef<number>(0);
  const lowSinceRef = useRef<number | null>(null);
  const downloadSamplesRef = useRef<{ ts: number; bytes: number }[]>([]);
  const lastStageRef = useRef<InstallationProgress['stage'] | null>(null);
  const initializedRef = useRef(false);
  const refreshAllRef = useRef<(forceRefresh?: boolean) => Promise<void>>(() => Promise.resolve());
  const fetchInstallationProgressRef = useRef<() => Promise<InstallationProgress | null>>(() => Promise.resolve(null));
  const fetchAvailableVersionsRef = useRef<(forceRefresh?: boolean) => Promise<void>>(() => Promise.resolve());
  const [cacheStatus, setCacheStatus] = useState<CacheStatus>({
    has_cache: false,
    is_valid: false,
    is_fetching: false
  });

  // Fetch installed versions
  const fetchInstalledVersions = useCallback(async () => {
    if (!window.pywebview?.api?.get_installed_versions) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_installed_versions();
      if (result.success) {
        setInstalledVersions(result.versions || []);
      } else {
        setError(result.error || 'Failed to fetch installed versions');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Fetch active version
  const fetchActiveVersion = useCallback(async () => {
    if (!window.pywebview?.api?.get_active_version) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_active_version();
      if (result.success) {
        setActiveVersion(result.version || null);
      } else {
        setError(result.error || 'Failed to fetch active version');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const fetchDefaultVersion = useCallback(async () => {
    if (!window.pywebview?.api?.get_default_version) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_default_version();
      if (result.success) {
        setDefaultVersionState(result.version || null);
      }
    } catch (e) {
      // non-fatal
    }
  }, []);

  // Fetch current installation progress (persists across UI reopen)
  const fetchInstallationProgress = useCallback(async () => {
    if (!window.pywebview?.api?.get_installation_progress) {
      return null;
    }

    try {
      const progress = await window.pywebview.api.get_installation_progress();

        if (progress && !progress.completed_at) {
          setInstallingTag(progress.tag || null);
          const now = Date.now();
          const downloadedBytes = progress.downloaded_bytes || 0;
          const speed = progress.download_speed || 0;

          // Reset download tracker when tag changes
          if (progress.tag !== lastDownloadTagRef.current) {
            lastDownloadRef.current = { bytes: downloadedBytes, speed: speed, ts: now };
            lastDownloadTagRef.current = progress.tag || null;
            topSpeedRef.current = speed || 0;
            lowSinceRef.current = null;
            downloadSamplesRef.current = [];
            lastStageRef.current = progress.stage || null;
          } else if (progress.stage !== lastStageRef.current) {
            downloadSamplesRef.current = [];
            lastStageRef.current = progress.stage || null;
          }

          downloadSamplesRef.current.push({ ts: now, bytes: downloadedBytes });
          downloadSamplesRef.current = downloadSamplesRef.current.filter(
            (sample) => now - sample.ts <= 5000
          );
          const sampleStart = downloadSamplesRef.current[0];
          const sampleEnd = downloadSamplesRef.current[downloadSamplesRef.current.length - 1];
          let averageSpeed = 0;
          if (sampleStart && sampleEnd && sampleEnd.ts > sampleStart.ts) {
            const deltaBytes = sampleEnd.bytes - sampleStart.bytes;
            const deltaTime = (sampleEnd.ts - sampleStart.ts) / 1000;
            if (deltaTime > 0 && deltaBytes >= 0) {
              averageSpeed = deltaBytes / deltaTime;
            }
          }

          const release = availableVersions.find((release) => release.tag_name === progress.tag);
          const archiveEstimate = release?.archive_size ?? null;
          const dependencyEstimate = release?.dependencies_size ??
            (release?.total_size && release?.archive_size
              ? Math.max(release.total_size - release.archive_size, 0)
              : null);

          let expectedTotal: number | null = null;
          if (progress.stage === 'download') {
            expectedTotal = progress.total_size ?? archiveEstimate ?? release?.total_size ?? null;
          } else if (progress.stage === 'dependencies') {
            expectedTotal = dependencyEstimate ?? release?.total_size ?? null;
          }

          let etaSeconds: number | null = null;
          const etaSpeed = averageSpeed > 0 ? averageSpeed : speed;
          if ((progress.stage === 'download' || progress.stage === 'dependencies') && expectedTotal && etaSpeed > 0) {
            const remaining = Math.max(expectedTotal - downloadedBytes, 0);
            etaSeconds = Math.ceil(remaining / etaSpeed);
          }

          const adjustedProgress = {
            ...progress,
            download_speed: progress.download_speed ?? (averageSpeed > 0 ? averageSpeed : null),
            eta_seconds: etaSeconds,
            total_size: expectedTotal ?? progress.total_size ?? null,
          };

          setInstallationProgress(adjustedProgress);

          // Compute network status
          let status: InstallNetworkStatus = 'downloading';

          if (progress.error) {
            status = 'failed';
          } else if (progress.stage === 'download') {
            const deltaTime = now - lastDownloadRef.current.ts;
            const deltaBytes = downloadedBytes - lastDownloadRef.current.bytes;
            const instantaneous = deltaTime > 0 ? deltaBytes / (deltaTime / 1000) : speed;

            const currentSpeed = speed || instantaneous;
            // Track top speed (never reduced by slow periods to avoid drift)
            if (currentSpeed > topSpeedRef.current * 0.9) {
              topSpeedRef.current = currentSpeed;
            } else if (topSpeedRef.current === 0) {
              topSpeedRef.current = currentSpeed;
            }

            const threshold = topSpeedRef.current * 0.5;
            const belowThreshold = topSpeedRef.current > 0 && currentSpeed > 0 && currentSpeed < threshold;

            if (belowThreshold) {
              if (lowSinceRef.current === null) {
                lowSinceRef.current = now;
              }
              const lowDuration = now - lowSinceRef.current;
              if (lowDuration >= 5000) {
                status = 'stalled';
              }
            } else {
              lowSinceRef.current = null;
              status = 'downloading';
            }

            lastDownloadRef.current = {
              bytes: downloadedBytes,
              speed: currentSpeed,
              ts: now,
            };
          } else {
            status = 'downloading';
          }

          setInstallNetworkStatus(status);
      } else {
        if (!progress && installingTag) {
          return progress || null;
        }
        setInstallingTag(null);
        setInstallationProgress(progress || null);
        setInstallNetworkStatus('idle');
        lastDownloadRef.current = { bytes: 0, speed: 0, ts: 0 };
        lastDownloadTagRef.current = null;
        topSpeedRef.current = 0;
        lowSinceRef.current = null;
        downloadSamplesRef.current = [];
        lastStageRef.current = null;
      }

      return progress || null;
    } catch (e) {
      console.error('Failed to fetch installation progress', e);
      setInstallNetworkStatus('failed');
      return null;
    }
  }, [availableVersions, installingTag]);

  // Fetch available versions from GitHub
  const fetchAvailableVersions = useCallback(async (forceRefresh: boolean = false) => {
    if (!window.pywebview?.api?.get_available_versions) {
      console.error('get_available_versions not available');
      return;
    }

    try {
      console.log('Fetching available versions, forceRefresh:', forceRefresh);
      const result = await window.pywebview.api.get_available_versions(forceRefresh);
      console.log('Available versions result:', result);
      if (result.success) {
        setAvailableVersions(result.versions || []);
        console.log('Set available versions:', result.versions?.length);

        // If backend flags an installing release, update local state
        const installingRelease = (result.versions || []).find((r: VersionRelease) => r.installing);
        if (installingRelease?.tag_name) {
          setInstallingTag(installingRelease.tag_name);
        }

        // Schedule a follow-up fetch to pick up size data after background calc
        if (forceRefresh) {
          if (followupRefreshRef.current) {
            clearTimeout(followupRefreshRef.current);
          }
          followupRefreshRef.current = setTimeout(() => {
            void fetchAvailableVersionsRef.current(false);
          }, 1500) as any;
        }
      } else {
        console.error('Failed to fetch available versions:', result.error);
        setError(result.error || 'Failed to fetch available versions');
      }
    } catch (e) {
      console.error('Exception fetching available versions:', e);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Update ref to latest function
  fetchAvailableVersionsRef.current = fetchAvailableVersions;

  // Fetch comprehensive version status
  const fetchVersionStatus = useCallback(async () => {
    if (!window.pywebview?.api?.get_version_status) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_version_status();
      if (result.success) {
        setVersionStatus(result.status || null);
        if (result.status?.defaultVersion !== undefined) {
          setDefaultVersionState(result.status.defaultVersion || null);
        }
      } else {
        setError(result.error || 'Failed to fetch version status');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Switch to a different version
  const switchVersion = useCallback(async (tag: string) => {
    if (!window.pywebview?.api?.switch_version) {
      throw new Error('API not available');
    }

    try {
      const result = await window.pywebview.api.switch_version(tag);
      if (result.success) {
        // Refresh all version data after successful switch
        await Promise.all([
          fetchActiveVersion(),
          fetchVersionStatus(),
        ]);
        return true;
      } else {
        throw new Error(result.error || 'Failed to switch version');
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, [fetchActiveVersion, fetchVersionStatus]);

  // Install a version
  const installVersion = useCallback(async (tag: string) => {
    if (!window.pywebview?.api?.install_version) {
      throw new Error('API not available');
    }

    try {
      setInstallingTag(tag);
      setInstallationProgress(null);
      const result = await window.pywebview.api.install_version(tag);
      if (result.success) {
        // Refresh all version data after successful installation
        await Promise.all([
          fetchInstalledVersions(),
          fetchVersionStatus(),
          fetchInstallationProgress(),
        ]);
        return true;
      } else {
        throw new Error(result.error || 'Failed to install version');
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, [fetchInstalledVersions, fetchInstallationProgress, fetchVersionStatus]);

  // Remove a version
  const removeVersion = useCallback(async (tag: string) => {
    if (!window.pywebview?.api?.remove_version) {
      throw new Error('API not available');
    }

    try {
      const result = await window.pywebview.api.remove_version(tag);
      if (result.success) {
        // Refresh all version data after successful removal
        await Promise.all([
          fetchInstalledVersions(),
          fetchActiveVersion(),
          fetchVersionStatus(),
        ]);
        return true;
      } else {
        throw new Error(result.error || 'Failed to remove version');
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, [fetchInstalledVersions, fetchActiveVersion, fetchVersionStatus]);

  // Open arbitrary path in the system file manager
  const openPath = useCallback(async (path: string) => {
    if (!window.pywebview?.api?.open_path) {
      throw new Error('API not available');
    }

    try {
      const result = await window.pywebview.api.open_path(path);
      if (!result.success) {
        const message = result.error || 'Failed to open path';
        setError(message);
        throw new Error(message);
      }
      return true;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, []);

  // Open the active installation directory
  const openActiveInstall = useCallback(async () => {
    if (!window.pywebview?.api?.open_active_install) {
      throw new Error('API not available');
    }

    try {
      const result = await window.pywebview.api.open_active_install();
      if (!result.success) {
        const message = result.error || 'Failed to open active installation';
        setError(message);
        throw new Error(message);
      }
      return true;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, []);

  // Get version info
  const getVersionInfo = useCallback(async (tag: string): Promise<VersionInfo | null> => {
    if (!window.pywebview?.api?.get_version_info) {
      throw new Error('API not available');
    }

    try {
      const result = await window.pywebview.api.get_version_info(tag);
      if (result.success) {
        return result.info || null;
      } else {
        throw new Error(result.error || 'Failed to get version info');
      }
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      setError(errorMsg);
      throw e;
    }
  }, []);

  // Refresh all version data
  const refreshAll = useCallback(async (forceRefresh: boolean = false) => {
    setIsLoading(true);
    setError(null);

    try {
      await Promise.all([
        fetchInstalledVersions(),
        fetchActiveVersion(),
        fetchDefaultVersion(),
        fetchAvailableVersions(forceRefresh),
        fetchVersionStatus(),
        fetchInstallationProgress(),
      ]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsLoading(false);
    }
  }, [fetchInstalledVersions, fetchActiveVersion, fetchDefaultVersion, fetchAvailableVersions, fetchVersionStatus, fetchInstallationProgress]);

  useEffect(() => {
    refreshAllRef.current = refreshAll;
  }, [refreshAll]);

  useEffect(() => {
    fetchInstallationProgressRef.current = fetchInstallationProgress;
  }, [fetchInstallationProgress]);

  // Refresh available versions only (non-blocking UI)
  const refreshAvailableVersions = useCallback(async (forceRefresh: boolean = false) => {
    setError(null);
    try {
      await fetchAvailableVersions(forceRefresh);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [fetchAvailableVersions]);

  const setDefaultVersion = useCallback(async (tag: string | null) => {
    if (!window.pywebview?.api?.set_default_version) {
      throw new Error('API not available');
    }
    try {
      const result = await window.pywebview.api.set_default_version(tag);
      if (result.success) {
        setDefaultVersionState(tag);
        await fetchVersionStatus();
        return true;
      }
      throw new Error(result.error || 'Failed to set default version');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
      throw e;
    }
  }, [fetchVersionStatus]);

  // Poll installation progress while an install is active
  useEffect(() => {
    if (installPollRef.current) {
      clearInterval(installPollRef.current);
      installPollRef.current = null;
    }

    if (!installingTag) {
      return;
    }

    const interval = setInterval(() => {
      void fetchInstallationProgress();
    }, 1000);

    installPollRef.current = interval as any;

    return () => {
      clearInterval(interval);
    };
  }, [installingTag, fetchInstallationProgress]);

  // Initial load - wait for PyWebView to be ready
  useEffect(() => {
    const loadData = async () => {
      console.log('useVersions loadData - pywebview available:', !!window.pywebview, 'api available:', !!window.pywebview?.api);
      if (!window.pywebview?.api) {
        console.error('PyWebView API not available!');
        setIsLoading(false);
        return;
      }

      // Validate installations first to detect and clean up any incomplete installations
      console.log('Validating installations...');
      try {
        if (window.pywebview.api.validate_installations) {
          const validationResult = await window.pywebview.api.validate_installations();
          if (validationResult.success && validationResult.result.had_invalid) {
            console.log('Found and cleaned up invalid installations:', validationResult.result.removed);
            console.log('Valid installations:', validationResult.result.valid);
          }
        }
      } catch (e) {
        console.error('Failed to validate installations:', e);
      }

      // Capture any in-progress installation before loading lists
      await fetchInstallationProgressRef.current();

      console.log('Calling refreshAll...');
      refreshAllRef.current(false);
    };

    // Poll for PyWebView API to be ready with actual methods (same approach as App.tsx)
    const waitForPyWebView = () => {
      if (initializedRef.current) {
        return;
      }
      if (window.pywebview?.api && typeof window.pywebview.api.get_available_versions === 'function') {
        console.log('PyWebView API ready with methods, loading version data...');
        initializedRef.current = true;
        loadData();
      } else {
        console.log('Waiting for PyWebView API methods...');
        setTimeout(waitForPyWebView, 100);
      }
    };

    waitForPyWebView();
  }, []);

  // Poll for background fetch completion and cache status
  useEffect(() => {
    let interval: NodeJS.Timeout | null = null;
    let waitTimeout: NodeJS.Timeout | null = null;

    const checkBackgroundFetch = async () => {
      if (!window.pywebview?.api) return;

      try {
        // Update cache status for footer (do this first for immediate feedback)
        const statusResult = await window.pywebview.api.get_github_cache_status();
        console.log('Cache status result:', statusResult);
        if (statusResult.success) {
          setCacheStatus(statusResult.status);
        } else {
          console.error('Failed to get cache status:', statusResult.error);
        }

        // Check if background fetch completed
        const result = await window.pywebview.api.has_background_fetch_completed();

        if (result.success && result.completed) {
          console.log('✓ Background fetch completed - refreshing UI with new data');

          // Reset the flag
          await window.pywebview.api.reset_background_fetch_flag();

          // Refresh versions (will get fresh data from cache)
          await fetchAvailableVersionsRef.current(false);
        }
      } catch (error) {
        console.error('Error checking background fetch:', error);
      }
    };

    // Wait for PyWebView API to be ready before starting polling
    const waitAndStartPolling = () => {
      if (window.pywebview?.api && typeof window.pywebview.api.get_github_cache_status === 'function') {
        console.log('Starting cache status polling...');
        // Initial check immediately
        void checkBackgroundFetch();

        // Poll every 2 seconds
        interval = setInterval(checkBackgroundFetch, 2000);
      } else {
        console.log('Waiting for PyWebView API to start cache status polling...');
        waitTimeout = setTimeout(waitAndStartPolling, 100);
      }
    };

    waitAndStartPolling();

    return () => {
      if (interval) {
        clearInterval(interval);
      }
      if (waitTimeout) {
        clearTimeout(waitTimeout);
      }
    };
  }, []); // Empty dependency array - run once on mount

  // Cleanup timers on unmount
  useEffect(() => {
    return () => {
      if (followupRefreshRef.current) {
        clearTimeout(followupRefreshRef.current);
      }
      if (installPollRef.current) {
        clearInterval(installPollRef.current);
      }
    };
  }, []);

  return {
    // State
    installedVersions,
    activeVersion,
    availableVersions,
    versionStatus,
    isLoading,
    error,
    installingTag,
    installationProgress,
    defaultVersion,
    setDefaultVersion,
    installNetworkStatus,
    cacheStatus,

    // Actions
    switchVersion,
    installVersion,
    removeVersion,
    getVersionInfo,
    refreshAll,
    refreshAvailableVersions,
    openPath,
    openActiveInstall,
    fetchInstallationProgress,
  };
}
