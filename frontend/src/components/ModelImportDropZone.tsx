/**
 * Model Import Drop Zone Component
 *
 * Provides a window-level drag-and-drop overlay for importing model paths.
 * Handles both File API and text/uri-list for cross-platform support.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Upload } from 'lucide-react';
import { getElectronAPI } from '../api/adapter';

interface ModelImportDropZoneProps {
  /** Callback when import paths are dropped */
  onPathsDropped: (paths: string[]) => void;
  /** Whether the drop zone is enabled */
  enabled?: boolean;
}

/**
 * Convert a file:// URI to a filesystem path.
 * Handles GTK/WebKit-style file-manager URI payloads.
 */
function fileUriToPath(uri: string): string {
  // Handle file:// protocol
  if (uri.startsWith('file://')) {
    // Decode URL-encoded characters
    let path = decodeURIComponent(uri.slice(7));
    // Remove leading slash on Windows paths (file:///C:/...)
    if (/^\/[A-Z]:/.test(path)) {
      path = path.slice(1);
    }
    return path;
  }
  return uri;
}

function extractElectronFilePaths(dataTransfer: DataTransfer): string[] {
  const electronAPI = getElectronAPI();
  if (!electronAPI || dataTransfer.files.length === 0) {
    return [];
  }

  const paths: string[] = [];
  for (const file of Array.from(dataTransfer.files)) {
    try {
      const path = electronAPI.getPathForFile(file);
      if (path) {
        paths.push(path);
      }
    } catch {
      // Fall through to other methods if getPathForFile fails.
    }
  }
  return paths;
}

function extractUriListPaths(dataTransfer: DataTransfer): string[] {
  const uriList = dataTransfer.getData('text/uri-list');
  if (!uriList) {
    return [];
  }

  return uriList
    .split('\n')
    .filter((line) => line && !line.startsWith('#'))
    .map((uri) => fileUriToPath(uri.trim()))
    .filter(Boolean);
}

function extractPlainTextPaths(dataTransfer: DataTransfer): string[] {
  const plainText = dataTransfer.getData('text/plain');
  if (!plainText) {
    return [];
  }

  return plainText
    .split('\n')
    .map((line) => fileUriToPath(line.trim()))
    .filter(Boolean);
}

function extractFallbackFilePaths(dataTransfer: DataTransfer): string[] {
  return Array.from(dataTransfer.files)
    .map((file) => (file as File & { path?: string }).path || file.name)
    .filter(Boolean);
}

/**
 * Extract filesystem paths from drag event data.
 * Supports File API, text/uri-list, and text/plain for cross-platform support.
 * In Electron with sandbox, uses webUtils.getPathForFile() via preload.
 */
function extractDroppedPaths(e: DragEvent): string[] {
  const dataTransfer = e.dataTransfer;
  if (!dataTransfer) {
    return [];
  }

  const electronPaths = extractElectronFilePaths(dataTransfer);
  if (electronPaths.length > 0) {
    return electronPaths;
  }

  const uriListPaths = extractUriListPaths(dataTransfer);
  if (uriListPaths.length > 0) {
    return uriListPaths;
  }

  const plainTextPaths = extractPlainTextPaths(dataTransfer);
  if (plainTextPaths.length > 0) {
    return plainTextPaths;
  }

  return extractFallbackFilePaths(dataTransfer);
}

export const ModelImportDropZone: React.FC<ModelImportDropZoneProps> = ({
  onPathsDropped,
  enabled = true,
}) => {
  const [isDragging, setIsDragging] = useState(false);
  const dragCounterRef = useRef(0);

  const handleDragEnter = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;
      e.preventDefault();
      e.stopPropagation();

      dragCounterRef.current++;

      // GTK/WebKit-backed drag sources may use non-File mime types here.
      // Accept any of: Files, text/uri-list, text/plain, or application/x-moz-file
      const types = e.dataTransfer?.types || [];
      const hasFileType =
        types.includes('Files') ||
        types.includes('text/uri-list') ||
        types.includes('text/plain') ||
        types.includes('application/x-moz-file');

      if (hasFileType) {
        setIsDragging(true);
      }
    },
    [enabled]
  );

  const handleDragLeave = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;
      e.preventDefault();
      e.stopPropagation();

      dragCounterRef.current--;

      // Only hide when all drag events have left
      if (dragCounterRef.current === 0) {
        setIsDragging(false);
      }
    },
    [enabled]
  );

  const handleDragOver = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;
      // CRITICAL: Must prevent default to allow drop
      e.preventDefault();
      e.stopPropagation();

      // Set the drop effect
      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = 'copy';
      }
    },
    [enabled]
  );

  const handleDrop = useCallback(
    (e: DragEvent) => {
      if (!enabled) return;
      // CRITICAL: Must prevent default to handle drop
      e.preventDefault();
      e.stopPropagation();

      // Reset drag state
      dragCounterRef.current = 0;
      setIsDragging(false);

      // Extract paths from the drop event
      const allPaths = extractDroppedPaths(e);
      if (allPaths.length === 0) {
        return;
      }

      onPathsDropped(allPaths);
    },
    [enabled, onPathsDropped]
  );

  // Handler for native drop events (sent from backend for platform compatibility)
  // This fires as a parallel path for platforms with native drag-drop handling
  const handleNativeDrop = useCallback(
    (e: CustomEvent<{ paths: string[] }>) => {
      if (!enabled) return;

      // Reset drag state
      dragCounterRef.current = 0;
      setIsDragging(false);

      onPathsDropped(e.detail.paths);
    },
    [enabled, onPathsDropped]
  );

  // Attach window-level event listeners
  useEffect(() => {
    if (!enabled) return;

    // Standard web drag-drop listeners (work on Windows/macOS)
    window.addEventListener('dragenter', handleDragEnter);
    window.addEventListener('dragleave', handleDragLeave);
    window.addEventListener('dragover', handleDragOver);
    window.addEventListener('drop', handleDrop);

    // Native drop listener (for platform-specific file drop handling)
    window.addEventListener('native-file-drop', handleNativeDrop as EventListener);

    return () => {
      window.removeEventListener('dragenter', handleDragEnter);
      window.removeEventListener('dragleave', handleDragLeave);
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('drop', handleDrop);
      window.removeEventListener('native-file-drop', handleNativeDrop as EventListener);
    };
  }, [enabled, handleDragEnter, handleDragLeave, handleDragOver, handleDrop, handleNativeDrop]);

  if (!isDragging) {
    return null;
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center drop-zone-backdrop bg-[hsl(var(--launcher-bg-primary)/0.8)]"
      onDragEnter={(e) => handleDragEnter(e.nativeEvent)}
      onDragLeave={(e) => handleDragLeave(e.nativeEvent)}
      onDragOver={(e) => handleDragOver(e.nativeEvent)}
      onDrop={(e) => handleDrop(e.nativeEvent)}
    >
      <div className="flex flex-col items-center justify-center p-12 rounded-xl border-2 border-dashed animate-pulse-border bg-[hsl(var(--launcher-bg-secondary)/0.9)]">
        <Upload className="w-16 h-16 mb-4 text-[hsl(var(--launcher-accent-primary))]" />
        <h2 className="text-xl font-semibold text-[hsl(var(--launcher-text-primary))] mb-2">
          Drop models or folders to import
        </h2>
        <p className="text-sm text-[hsl(var(--launcher-text-muted))] text-center max-w-xs">
          Pumas will classify files, bundle roots, and model folders before import.
        </p>
      </div>
    </div>
  );
};
