/**
 * Conflict Resolution Dialog Component (Phase 3B)
 *
 * Interactive dialog for resolving mapping conflicts.
 * Users can choose to Overwrite, Rename, or Skip for each conflict.
 */

import React, { useState, useCallback, useMemo } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  X,
  RefreshCw,
  FileSymlink,
  SkipForward,
  Replace,
  Edit3,
  CheckCircle,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import type { MappingAction } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('ConflictResolutionDialog');

/**
 * Resolution action types
 */
export type ConflictResolutionAction = 'skip' | 'overwrite' | 'rename';

/**
 * Resolution map for all conflicts
 */
export type ConflictResolutions = Record<string, ConflictResolutionAction>;

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

/**
 * Human-readable conflict reason descriptions
 */
function getConflictDescription(reason: string): string {
  if (reason.includes('different source')) {
    return 'Symlink points to a different model file';
  }
  if (reason.includes('file exists') || reason.includes('Non-symlink')) {
    return 'A regular file exists at this location';
  }
  return reason;
}

/**
 * Action option configuration
 */
const actionOptions: Array<{
  value: ConflictResolutionAction;
  label: string;
  description: string;
  icon: typeof SkipForward;
  color: string;
}> = [
  {
    value: 'skip',
    label: 'Skip',
    description: 'Keep existing file, do not create link',
    icon: SkipForward,
    color: 'text-[hsl(var(--launcher-text-tertiary))]',
  },
  {
    value: 'overwrite',
    label: 'Overwrite',
    description: 'Replace existing with new symlink',
    icon: Replace,
    color: 'text-[hsl(var(--accent-warning))]',
  },
  {
    value: 'rename',
    label: 'Rename Existing',
    description: 'Rename existing to .old, create new link',
    icon: Edit3,
    color: 'text-[hsl(var(--accent-primary))]',
  },
];

export const ConflictResolutionDialog: React.FC<ConflictResolutionDialogProps> = ({
  isOpen,
  conflicts,
  onClose,
  onApply,
  versionTag,
}) => {
  // Resolution state for each conflict (keyed by model_id)
  const [resolutions, setResolutions] = useState<ConflictResolutions>({});
  const [isApplying, setIsApplying] = useState(false);
  const [expandedConflict, setExpandedConflict] = useState<string | null>(null);

  // Initialize resolutions with 'skip' as default
  const effectiveResolutions = useMemo(() => {
    const result: ConflictResolutions = {};
    for (const conflict of conflicts) {
      result[conflict.model_id] = resolutions[conflict.model_id] || 'skip';
    }
    return result;
  }, [conflicts, resolutions]);

  // Count resolutions by type
  const resolutionCounts = useMemo(() => {
    const counts = { skip: 0, overwrite: 0, rename: 0 };
    for (const resolution of Object.values(effectiveResolutions)) {
      counts[resolution]++;
    }
    return counts;
  }, [effectiveResolutions]);

  const handleResolutionChange = useCallback(
    (modelId: string, action: ConflictResolutionAction) => {
      setResolutions((prev) => ({
        ...prev,
        [modelId]: action,
      }));
    },
    []
  );

  const handleApplyToAll = useCallback((action: ConflictResolutionAction) => {
    const newResolutions: ConflictResolutions = {};
    for (const conflict of conflicts) {
      newResolutions[conflict.model_id] = action;
    }
    setResolutions(newResolutions);
  }, [conflicts]);

  const handleApply = useCallback(async () => {
    setIsApplying(true);
    try {
      await onApply(effectiveResolutions);
      logger.info('Applied conflict resolutions', { resolutions: effectiveResolutions });
    } catch (error) {
      logger.error('Failed to apply resolutions', { error });
    } finally {
      setIsApplying(false);
    }
  }, [effectiveResolutions, onApply]);

  const toggleExpanded = useCallback((modelId: string) => {
    setExpandedConflict((prev) => (prev === modelId ? null : modelId));
  }, []);

  if (!isOpen) {
    return null;
  }

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      >
        <motion.div
          initial={{ scale: 0.95, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.95, opacity: 0 }}
          transition={{ duration: 0.2 }}
          className="bg-[hsl(var(--launcher-bg-primary))] rounded-lg border border-[hsl(var(--launcher-border))] shadow-xl w-full max-w-2xl max-h-[80vh] flex flex-col"
          onClick={(e) => e.stopPropagation()}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-[hsl(var(--launcher-border))]">
            <div className="flex items-center gap-3">
              <AlertTriangle className="w-5 h-5 text-[hsl(var(--accent-warning))]" />
              <div>
                <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
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
              {actionOptions.map((option) => (
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
              const currentResolution = effectiveResolutions[conflict.model_id];
              const isExpanded = expandedConflict === conflict.model_id;

              return (
                <div
                  key={conflict.model_id}
                  className="border border-[hsl(var(--launcher-border)/0.5)] rounded-lg overflow-hidden"
                >
                  {/* Conflict Summary Row */}
                  <button
                    onClick={() => toggleExpanded(conflict.model_id)}
                    className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.3)] transition-colors"
                  >
                    <div className="flex items-center gap-3 flex-1 min-w-0">
                      <FileSymlink className="w-4 h-4 text-[hsl(var(--accent-warning))] flex-shrink-0" />
                      <div className="flex-1 min-w-0 text-left">
                        <div className="text-sm font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                          {conflict.model_name || conflict.model_id}
                        </div>
                        <div className="text-xs text-[hsl(var(--accent-warning))]">
                          {getConflictDescription(conflict.reason)}
                        </div>
                      </div>
                    </div>

                    <div className="flex items-center gap-3 flex-shrink-0">
                      {/* Resolution Selector */}
                      <select
                        value={currentResolution}
                        onChange={(e) => {
                          e.stopPropagation();
                          handleResolutionChange(
                            conflict.model_id,
                            e.target.value as ConflictResolutionAction
                          );
                        }}
                        onClick={(e) => e.stopPropagation()}
                        disabled={isApplying}
                        className="px-3 py-1.5 text-xs rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))] text-[hsl(var(--launcher-text-primary))] disabled:opacity-50"
                      >
                        {actionOptions.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>

                      {isExpanded ? (
                        <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
                      ) : (
                        <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
                      )}
                    </div>
                  </button>

                  {/* Expanded Details */}
                  <AnimatePresence>
                    {isExpanded && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        transition={{ duration: 0.15 }}
                        className="overflow-hidden"
                      >
                        <div className="px-4 pb-3 pt-1 space-y-2 bg-[hsl(var(--launcher-bg-secondary)/0.3)]">
                          <div className="text-xs space-y-1">
                            <div className="flex">
                              <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Source:</span>
                              <span className="text-[hsl(var(--launcher-text-secondary))] font-mono truncate flex-1">
                                {conflict.source_path.split('/').slice(-2).join('/')}
                              </span>
                            </div>
                            <div className="flex">
                              <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Target:</span>
                              <span className="text-[hsl(var(--launcher-text-secondary))] font-mono truncate flex-1">
                                {conflict.target_path.split('/').slice(-2).join('/')}
                              </span>
                            </div>
                            {conflict.existing_target && (
                              <div className="flex">
                                <span className="text-[hsl(var(--launcher-text-tertiary))] w-20">Existing:</span>
                                <span className="text-[hsl(var(--accent-warning))] font-mono truncate flex-1">
                                  {conflict.existing_target}
                                </span>
                              </div>
                            )}
                          </div>

                          {/* Action Descriptions */}
                          <div className="pt-2 border-t border-[hsl(var(--launcher-border)/0.3)]">
                            {actionOptions.map((option) => (
                              <label
                                key={option.value}
                                className={`flex items-start gap-2 py-1 cursor-pointer ${
                                  currentResolution === option.value
                                    ? 'opacity-100'
                                    : 'opacity-50 hover:opacity-75'
                                }`}
                              >
                                <input
                                  type="radio"
                                  name={`resolution-${conflict.model_id}`}
                                  value={option.value}
                                  checked={currentResolution === option.value}
                                  onChange={() =>
                                    handleResolutionChange(conflict.model_id, option.value)
                                  }
                                  disabled={isApplying}
                                  className="mt-0.5"
                                />
                                <div className="flex-1">
                                  <div className={`text-xs font-medium ${option.color}`}>
                                    {option.label}
                                  </div>
                                  <div className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
                                    {option.description}
                                  </div>
                                </div>
                              </label>
                            ))}
                          </div>
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
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
      </motion.div>
    </AnimatePresence>
  );
};
