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
 * Extract file paths from drag event data.
 * Supports both File API and text/uri-list (Linux file managers).
 */
function extractFilePaths(e: DragEvent): string[] {
  const paths: string[] = [];

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

  // Fallback to File API (may have limited path access)
  if (paths.length === 0 && e.dataTransfer?.files) {
    for (const file of Array.from(e.dataTransfer.files)) {
      // In PyWebView, we may get the path through the File object
      // This is a best-effort approach
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

      // Check if dragging files
      if (e.dataTransfer?.types.includes('Files') || e.dataTransfer?.types.includes('text/uri-list')) {
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

  // Attach window-level event listeners
  useEffect(() => {
    if (!enabled) return;

    window.addEventListener('dragenter', handleDragEnter);
    window.addEventListener('dragleave', handleDragLeave);
    window.addEventListener('dragover', handleDragOver);
    window.addEventListener('drop', handleDrop);

    return () => {
      window.removeEventListener('dragenter', handleDragEnter);
      window.removeEventListener('dragleave', handleDragLeave);
      window.removeEventListener('dragover', handleDragOver);
      window.removeEventListener('drop', handleDrop);
    };
  }, [enabled, handleDragEnter, handleDragLeave, handleDragOver, handleDrop]);

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
