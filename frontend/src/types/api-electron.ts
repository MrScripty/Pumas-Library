import type { DesktopBridgeAPI } from './api-bridge';
import type {
  ModelDownloadUpdateOutcome,
  ModelLibraryUpdateNotification,
  RuntimeProfileUpdateFeed,
  ServingStatusUpdateFeed,
  StatusTelemetryUpdateNotification,
} from '../generated/desktop-contract';

/**
 * Receives why an update could not be delivered: a payload that failed its
 * contract, or a stream failure. Resynchronize from the RPC snapshot.
 */
export type UpdateStreamErrorHandler = (message: string) => void;

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
    callback: (notification: ModelLibraryUpdateNotification) => void,
    onError: UpdateStreamErrorHandler
  ): () => void;
  /** Subscribe to backend-owned model download update notifications. */
  onModelDownloadUpdate(
    callback: (notification: ModelDownloadUpdateOutcome) => void,
    onError: UpdateStreamErrorHandler
  ): () => void;
  /** Subscribe to backend-owned runtime profile update notifications. */
  onRuntimeProfileUpdate(
    callback: (notification: RuntimeProfileUpdateFeed) => void,
    onError: UpdateStreamErrorHandler
  ): () => void;
  /** Subscribe to backend-owned serving status update notifications. */
  onServingStatusUpdate(
    callback: (notification: ServingStatusUpdateFeed) => void,
    onError: UpdateStreamErrorHandler
  ): () => void;
  /** Subscribe to backend-owned status/resource telemetry notifications. */
  onStatusTelemetryUpdate(
    callback: (notification: StatusTelemetryUpdateNotification) => void,
    onError: UpdateStreamErrorHandler
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
