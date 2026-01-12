/**
 * Model Import Drop Zone Component
 *
 * Provides a window-level drag-and-drop overlay for importing model files.
 * Handles both File API and text/uri-list for cross-platform support.
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Upload } from 'lucide-react';

/** Valid model file extensions */
const VALID_EXTENSIONS = ['.safetensors', '.ckpt', '.gguf', '.pt', '.bin', '.pth', '.onnx'];

interface ModelImportDropZoneProps {
  /** Callback when files are dropped */
  onFilesDropped: (paths: string[]) => void;
  /** Whether the drop zone is enabled */
  enabled?: boolean;
}

/**
 * Convert a file:// URI to a filesystem path.
 * Handles PyWebView GTK/WebKit URI format.
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

/**
 * Get the Electron API for file path resolution (if available).
 */
function getElectronAPI(): { getPathForFile: (file: File) => string } | null {
  if (typeof window !== 'undefined' && 'electronAPI' in window) {
    return (window as unknown as { electronAPI: { getPathForFile: (file: File) => string } }).electronAPI;
  }
  return null;
}

/**
 * Extract file paths from drag event data.
 * Supports File API, text/uri-list, and text/plain for cross-platform support.
 * In Electron with sandbox, uses webUtils.getPathForFile() via preload.
 */
function extractFilePaths(e: DragEvent): string[] {
  const paths: string[] = [];
  const electronAPI = getElectronAPI();

  // In Electron, use the secure getPathForFile API (required for sandbox mode)
  if (electronAPI && e.dataTransfer?.files && e.dataTransfer.files.length > 0) {
    for (const file of Array.from(e.dataTransfer.files)) {
      try {
        const path = electronAPI.getPathForFile(file);
        if (path) {
          paths.push(path);
        }
      } catch {
        // Fall through to other methods if getPathForFile fails
      }
    }
    if (paths.length > 0) {
      return paths;
    }
  }

  // Try text/uri-list first (Linux file managers like Nautilus, Dolphin, Thunar)
  const uriList = e.dataTransfer?.getData('text/uri-list');
  if (uriList) {
    const uris = uriList.split('\n').filter((line) => line && !line.startsWith('#'));
    for (const uri of uris) {
      const path = fileUriToPath(uri.trim());
      if (path) {
        paths.push(path);
      }
    }
  }

  // Fallback: Try text/plain (some file managers use this)
  if (paths.length === 0) {
    const plainText = e.dataTransfer?.getData('text/plain');
    if (plainText) {
      const lines = plainText
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line);
      for (const line of lines) {
        const path = fileUriToPath(line);
        if (path) {
          paths.push(path);
        }
      }
    }
  }

  // Last resort fallback to File API (may have limited path access without Electron)
  if (paths.length === 0 && e.dataTransfer?.files) {
    for (const file of Array.from(e.dataTransfer.files)) {
      // Try to get path property (non-sandboxed environments)
      const path = (file as File & { path?: string }).path || file.name;
      if (path) {
        paths.push(path);
      }
    }
  }

  return paths;
}

/**
 * Check if a file has a valid model extension.
 */
function isValidModelFile(path: string): boolean {
  const lowerPath = path.toLowerCase();
  return VALID_EXTENSIONS.some((ext) => lowerPath.endsWith(ext));
}

/**
 * Filter paths to only include valid model files.
 * Returns both valid and invalid counts for feedback.
 */
function filterValidPaths(paths: string[]): { valid: string[]; invalidCount: number } {
  const valid: string[] = [];
  let invalidCount = 0;

  for (const path of paths) {
    if (isValidModelFile(path)) {
      valid.push(path);
    } else {
      invalidCount++;
    }
  }

  return { valid, invalidCount };
}

export const ModelImportDropZone: React.FC<ModelImportDropZoneProps> = ({
  onFilesDropped,
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

      // Check if dragging files - PyWebView GTK/WebKit may use different types
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
      const allPaths = extractFilePaths(e);
      if (allPaths.length === 0) {
        return;
      }

      // Filter to valid model files
      const { valid } = filterValidPaths(allPaths);
      if (valid.length > 0) {
        onFilesDropped(valid);
      }
    },
    [enabled, onFilesDropped]
  );

  // Handler for native drop events (sent from backend for platform compatibility)
  // This fires as a parallel path for platforms with native drag-drop handling
  const handleNativeDrop = useCallback(
    (e: CustomEvent<{ paths: string[] }>) => {
      if (!enabled) return;

      // Reset drag state
      dragCounterRef.current = 0;
      setIsDragging(false);

      const { paths } = e.detail;
      const { valid } = filterValidPaths(paths);
      if (valid.length > 0) {
        onFilesDropped(valid);
      }
    },
    [enabled, onFilesDropped]
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
          Drop models to import
        </h2>
        <p className="text-sm text-[hsl(var(--launcher-text-muted))] text-center max-w-xs">
          Supports .safetensors, .ckpt, .gguf, .pt, .bin, .pth, .onnx
        </p>
      </div>
    </div>
  );
};
