import { useState, useEffect, useCallback } from 'react';

export interface VersionRelease {
  tag_name: string;
  name: string;
  published_at: string;
  prerelease: boolean;
  body?: string;
  total_size?: number | null;
  archive_size?: number | null;
  dependencies_size?: number | null;
}

export interface VersionStatus {
  installedCount: number;
  activeVersion: string | null;
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

export function useVersions() {
  const [installedVersions, setInstalledVersions] = useState<string[]>([]);
  const [activeVersion, setActiveVersion] = useState<string | null>(null);
  const [availableVersions, setAvailableVersions] = useState<VersionRelease[]>([]);
  const [versionStatus, setVersionStatus] = useState<VersionStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
      } else {
        console.error('Failed to fetch available versions:', result.error);
        setError(result.error || 'Failed to fetch available versions');
      }
    } catch (e) {
      console.error('Exception fetching available versions:', e);
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  // Fetch comprehensive version status
  const fetchVersionStatus = useCallback(async () => {
    if (!window.pywebview?.api?.get_version_status) {
      return;
    }

    try {
      const result = await window.pywebview.api.get_version_status();
      if (result.success) {
        setVersionStatus(result.status || null);
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
      const result = await window.pywebview.api.install_version(tag);
      if (result.success) {
        // Refresh all version data after successful installation
        await Promise.all([
          fetchInstalledVersions(),
          fetchVersionStatus(),
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
  }, [fetchInstalledVersions, fetchVersionStatus]);

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
        fetchAvailableVersions(forceRefresh),
        fetchVersionStatus(),
      ]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsLoading(false);
    }
  }, [fetchInstalledVersions, fetchActiveVersion, fetchAvailableVersions, fetchVersionStatus]);

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

      console.log('Calling refreshAll...');
      refreshAll(false);
    };

    // Poll for PyWebView API to be ready with actual methods (same approach as App.tsx)
    const waitForPyWebView = () => {
      if (window.pywebview?.api && typeof window.pywebview.api.get_available_versions === 'function') {
        console.log('PyWebView API ready with methods, loading version data...');
        loadData();
      } else {
        console.log('Waiting for PyWebView API methods...');
        setTimeout(waitForPyWebView, 100);
      }
    };

    waitForPyWebView();
  }, [refreshAll]);

  return {
    // State
    installedVersions,
    activeVersion,
    availableVersions,
    versionStatus,
    isLoading,
    error,

    // Actions
    switchVersion,
    installVersion,
    removeVersion,
    getVersionInfo,
    refreshAll,
  };
}
