import type { DesktopBridgeAPI } from './api-bridge';

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
