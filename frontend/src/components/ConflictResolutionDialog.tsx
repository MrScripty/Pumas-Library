/**
 * Conflict Resolution Dialog Component (Phase 3B)
 *
 * Interactive dialog for resolving mapping conflicts.
 * Users can choose to Overwrite, Rename, or Skip for each conflict.
 */

import React, { useEffect, useId, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  X,
  RefreshCw,
  CheckCircle,
} from 'lucide-react';
import type { MappingAction } from '../types/api';
import {
  useConflictResolutions,
  type ConflictResolutions,
} from '../hooks/useConflictResolutions';
import {
  ConflictResolutionItem,
  conflictActionOptions,
} from './ConflictResolutionItem';

interface ConflictResolutionDialogProps {
  /** Whether dialog is open */
  isOpen: boolean;
  /** Conflicts to resolve */
  conflicts: MappingAction[];
  /** Callback when dialog is closed without applying */
  onClose: () => void;
  /** Callback when resolutions are applied */
  onApply: (resolutions: ConflictResolutions) => Promise<void>;
  /** Version tag for display */
  versionTag?: string;
}

export const ConflictResolutionDialog: React.FC<ConflictResolutionDialogProps> = ({
  isOpen,
  conflicts,
  onClose,
  onApply,
  versionTag,
}) => {
  const titleId = useId();
  const cancelButtonRef = useRef<HTMLButtonElement>(null);
  const {
    effectiveResolutions,
    expandedConflict,
    handleApply,
    handleApplyToAll,
    handleResolutionChange,
    isApplying,
    resolutionCounts,
    toggleExpanded,
  } = useConflictResolutions({ conflicts, onApply });

  useEffect(() => {
    if (!isOpen) {
      return undefined;
    }

    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const focusTimer = window.setTimeout(() => cancelButtonRef.current?.focus(), 0);

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isApplying) {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener('keydown', handleKeyDown);
      previousFocus?.focus();
    };
  }, [isApplying, isOpen, onClose]);

  if (!isOpen) {
    return null;
  }

  return (
    <AnimatePresence>
      <>
        <motion.button
          type="button"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm"
          aria-label="Close conflict resolution dialog"
          onClick={onClose}
          disabled={isApplying}
        />
        <div className="fixed inset-0 z-50 flex items-center justify-center pointer-events-none">
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            initial={{ scale: 0.95, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            exit={{ scale: 0.95, opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="pointer-events-auto bg-[hsl(var(--launcher-bg-primary))] rounded-lg border border-[hsl(var(--launcher-border))] shadow-xl w-full max-w-2xl max-h-[80vh] flex flex-col"
          >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
            <div className="flex items-center gap-3">
              <AlertTriangle className="w-5 h-5 text-[hsl(var(--accent-warning))]" />
              <div>
                <h2 id={titleId} className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
                  Resolve Conflicts
                </h2>
                <p className="text-xs text-[hsl(var(--launcher-text-secondary))]">
                  {conflicts.length} conflict{conflicts.length !== 1 ? 's' : ''} found
                  {versionTag && ` for ${versionTag}`}
                </p>
              </div>
            </div>
            <button
              onClick={onClose}
              disabled={isApplying}
              className="p-1 hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
            >
              <X className="w-5 h-5 text-[hsl(var(--launcher-text-secondary))]" />
            </button>
          </div>

          {/* Apply to All Row */}
          <div className="px-6 py-3 bg-[hsl(var(--launcher-bg-secondary)/0.5)] border-b border-[hsl(var(--launcher-border)/0.5)] flex items-center justify-between">
            <span className="text-sm text-[hsl(var(--launcher-text-secondary))]">
              Apply to all conflicts:
            </span>
            <div className="flex gap-2">
              {conflictActionOptions.map((option) => (
                <button
                  key={option.value}
                  onClick={() => handleApplyToAll(option.value)}
                  disabled={isApplying}
                  className="px-3 py-1.5 text-xs rounded border border-[hsl(var(--launcher-border))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors disabled:opacity-50 flex items-center gap-1.5"
                >
                  <option.icon className={`w-3 h-3 ${option.color}`} />
                  {option.label}
                </button>
              ))}
            </div>
          </div>

          {/* Conflict List */}
          <div className="flex-1 overflow-y-auto px-6 py-4 space-y-2">
            {conflicts.map((conflict) => {
              const currentResolution = effectiveResolutions[conflict.model_id] || 'skip';
              const isExpanded = expandedConflict === conflict.model_id;

              return (
                <ConflictResolutionItem
                  key={conflict.model_id}
                  conflict={conflict}
                  currentResolution={currentResolution}
                  isApplying={isApplying}
                  isExpanded={isExpanded}
                  onResolutionChange={handleResolutionChange}
                  onToggleExpanded={toggleExpanded}
                />
              );
            })}
          </div>

          {/* Footer */}
          <div className="px-6 py-4 border-t border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary)/0.3)]">
            {/* Summary */}
            <div className="flex items-center justify-between mb-3">
              <div className="flex gap-4 text-xs">
                <span className="text-[hsl(var(--launcher-text-tertiary))]">
                  <span className="font-medium text-[hsl(var(--launcher-text-secondary))]">
                    {resolutionCounts.skip}
                  </span>{' '}
                  skip
                </span>
                <span className="text-[hsl(var(--launcher-text-tertiary))]">
                  <span className="font-medium text-[hsl(var(--accent-warning))]">
                    {resolutionCounts.overwrite}
                  </span>{' '}
                  overwrite
                </span>
                <span className="text-[hsl(var(--launcher-text-tertiary))]">
                  <span className="font-medium text-[hsl(var(--accent-primary))]">
                    {resolutionCounts.rename}
                  </span>{' '}
                  rename
                </span>
              </div>
            </div>

            {/* Action Buttons */}
            <div className="flex gap-3 justify-end">
              <button
                ref={cancelButtonRef}
                onClick={onClose}
                disabled={isApplying}
                className="px-4 py-2 text-sm rounded border border-[hsl(var(--launcher-border))] hover:bg-[hsl(var(--launcher-bg-tertiary))] transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={handleApply}
                disabled={isApplying}
                className="px-4 py-2 text-sm rounded bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary-hover))] text-white transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {isApplying ? (
                  <>
                    <RefreshCw className="w-4 h-4 animate-spin" />
                    Applying...
                  </>
                ) : (
                  <>
                    <CheckCircle className="w-4 h-4" />
                    Apply Resolutions
                  </>
                )}
              </button>
            </div>
          </div>
          </motion.div>
        </div>
      </>
    </AnimatePresence>
  );
};
