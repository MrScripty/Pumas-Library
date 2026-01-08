/**
 * Installation State Hook
 *
 * Manages UI state for installation dialog (filters, view mode, hover states).
 * Extracted from InstallDialog.tsx
 */

import { useState, useEffect } from 'react';
import type { InstallationProgress } from './useVersions';

interface UseInstallationStateOptions {
  isOpen: boolean;
  installingVersion: string | null;
  progress: InstallationProgress | null;
}

interface UseInstallationStateResult {
  showPreReleases: boolean;
  setShowPreReleases: (value: boolean) => void;
  showInstalled: boolean;
  setShowInstalled: (value: boolean) => void;
  viewMode: 'list' | 'details';
  setViewMode: (value: 'list' | 'details') => void;
  showCompletedItems: boolean;
  setShowCompletedItems: (value: boolean) => void;
  hoveredTag: string | null;
  setHoveredTag: (value: string | null) => void;
  cancelHoverTag: string | null;
  setCancelHoverTag: (value: string | null) => void;
}

export function useInstallationState({
  isOpen,
  installingVersion,
  progress,
}: UseInstallationStateOptions): UseInstallationStateResult {
  const [showPreReleases, setShowPreReleases] = useState(true);
  const [showInstalled, setShowInstalled] = useState(true);
  const [viewMode, setViewMode] = useState<'list' | 'details'>('list');
  const [showCompletedItems, setShowCompletedItems] = useState(false);
  const [hoveredTag, setHoveredTag] = useState<string | null>(null);
  const [cancelHoverTag, setCancelHoverTag] = useState<string | null>(null);

  // Reset to list view when dialog opens
  useEffect(() => {
    if (isOpen) {
      setViewMode('list');
    }
  }, [isOpen]);

  // Auto-switch to list view if no progress data
  useEffect(() => {
    if (viewMode === 'details' && (!installingVersion || !progress)) {
      setViewMode('list');
    }
  }, [viewMode, installingVersion, progress]);

  return {
    showPreReleases,
    setShowPreReleases,
    showInstalled,
    setShowInstalled,
    viewMode,
    setViewMode,
    showCompletedItems,
    setShowCompletedItems,
    hoveredTag,
    setHoveredTag,
    cancelHoverTag,
    setCancelHoverTag,
  };
}
