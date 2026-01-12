/**
 * API Adapter
 *
 * Provides a unified API interface that works with both PyWebView and Electron.
 * Automatically detects the runtime environment and uses the appropriate backend.
 */

import type { PyWebViewAPI } from '../types/pywebview';

/**
 * Runtime environment detection
 */
export type RuntimeEnvironment = 'electron' | 'pywebview' | 'browser';

/**
 * Detect the current runtime environment
 */
export function detectEnvironment(): RuntimeEnvironment {
  if (typeof window === 'undefined') {
    return 'browser';
  }

  // Check for Electron API first (newer, preferred)
  if ('electronAPI' in window) {
    return 'electron';
  }

  // Check for PyWebView API
  if (window.pywebview?.api) {
    return 'pywebview';
  }

  // Fallback to browser (development mode without backend)
  return 'browser';
}

/**
 * Get the API instance for the current runtime
 */
function getAPIInstance(): PyWebViewAPI | null {
  const env = detectEnvironment();

  switch (env) {
    case 'electron':
      // In Electron, the API is exposed as window.electronAPI
      // but also as window.pywebview.api for backwards compatibility
      return (window as unknown as { electronAPI: PyWebViewAPI }).electronAPI;

    case 'pywebview':
      return window.pywebview?.api ?? null;

    case 'browser':
      // In browser mode (no backend), return null
      // Components should handle this gracefully
      return null;
  }
}

/**
 * Check if the API is available
 */
export function isAPIAvailable(): boolean {
  return getAPIInstance() !== null;
}

/**
 * Get the current runtime environment name (for debugging)
 */
export function getEnvironmentName(): string {
  return detectEnvironment();
}

/**
 * The unified API instance
 *
 * Usage:
 * ```typescript
 * import { api } from './api/adapter';
 *
 * // Use the API - works in both PyWebView and Electron
 * const status = await api.get_status();
 * ```
 */
export const api: PyWebViewAPI = new Proxy({} as PyWebViewAPI, {
  get(_target, prop: string) {
    const instance = getAPIInstance();

    if (!instance) {
      // Return a function that throws an error
      return async () => {
        throw new Error(
          `API not available: ${prop}. ` +
            `Current environment: ${detectEnvironment()}. ` +
            'Make sure you are running in Electron or PyWebView.'
        );
      };
    }

    const value = instance[prop as keyof PyWebViewAPI];

    // If it's a function, bind it to the instance
    if (typeof value === 'function') {
      return value.bind(instance);
    }

    return value;
  },
});

/**
 * Safe API call wrapper with error handling
 *
 * Usage:
 * ```typescript
 * const result = await safeAPICall(
 *   () => api.get_status(),
 *   { success: false, error: 'API unavailable' }
 * );
 * ```
 */
export async function safeAPICall<T>(
  call: () => Promise<T>,
  fallback: T
): Promise<T> {
  if (!isAPIAvailable()) {
    return fallback;
  }

  try {
    return await call();
  } catch (error) {
    console.error('API call failed:', error);
    return fallback;
  }
}

/**
 * Window-specific API extensions (Electron only)
 */
export const windowAPI = {
  /**
   * Minimize the window (Electron only)
   */
  minimize: async (): Promise<void> => {
    const env = detectEnvironment();
    if (env === 'electron') {
      const electronAPI = (window as unknown as { electronAPI: { minimizeWindow: () => Promise<void> } }).electronAPI;
      await electronAPI.minimizeWindow();
    }
  },

  /**
   * Maximize/restore the window (Electron only)
   */
  maximize: async (): Promise<void> => {
    const env = detectEnvironment();
    if (env === 'electron') {
      const electronAPI = (window as unknown as { electronAPI: { maximizeWindow: () => Promise<void> } }).electronAPI;
      await electronAPI.maximizeWindow();
    }
  },

  /**
   * Get the current theme (Electron only)
   */
  getTheme: async (): Promise<'dark' | 'light'> => {
    const env = detectEnvironment();
    if (env === 'electron') {
      const electronAPI = (window as unknown as { electronAPI: { getTheme: () => Promise<'dark' | 'light'> } }).electronAPI;
      return await electronAPI.getTheme();
    }
    // Default to dark theme
    return 'dark';
  },
};

// Export types
export type { PyWebViewAPI } from '../types/pywebview';
