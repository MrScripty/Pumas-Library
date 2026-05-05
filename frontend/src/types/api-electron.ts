import type { DesktopBridgeAPI } from './api-bridge';
import type { ModelLibraryUpdateNotification } from './api-package-facts';
import type { RuntimeProfileUpdateFeed } from './api-runtime-profiles';

// ============================================================================
// Electron-specific API extensions
// ============================================================================

export interface ElectronWindowAPI {
  /** Minimize the window (Electron only) */
  minimizeWindow(): Promise<void>;
  /** Maximize/restore the window (Electron only) */
  maximizeWindow(): Promise<void>;
  /** Get the current theme (Electron only) */
  getTheme(): Promise<'dark' | 'light'>;
  /** Resolve a sandboxed dropped file to a filesystem path. */
  getPathForFile(file: File): string;
  /** Subscribe to backend-owned model-library update notifications. */
  onModelLibraryUpdate(
    callback: (notification: ModelLibraryUpdateNotification) => void
  ): () => void;
  /** Subscribe to backend-owned runtime profile update notifications. */
  onRuntimeProfileUpdate(
    callback: (notification: RuntimeProfileUpdateFeed) => void
  ): () => void;
}

export type ElectronAPI = DesktopBridgeAPI & ElectronWindowAPI;

// ============================================================================
// Global Window Extension
// ============================================================================

declare global {
  interface Window {
    /** Canonical Electron desktop bridge. */
    electronAPI?: ElectronAPI;
  }
}
