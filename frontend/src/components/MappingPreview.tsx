/**
 * Mapping Preview Component (Phase 1C)
 *
 * Displays a preview of model mapping operations that will be performed,
 * showing links to create, conflicts, and warnings before applying.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Link,
  Plus,
  SkipForward,
  AlertCircle,
  FolderSymlink,
  HardDrive,
} from 'lucide-react';
import { getLogger } from '../utils/logger';

const logger = getLogger('MappingPreview');

interface MappingAction {
  model_id: string;
  model_name: string;
  source_path: string;
  target_path: string;
  link_type?: string;
  reason: string;
  existing_target?: string;
}

interface MappingPreviewResponse {
  success: boolean;
  error?: string;
  to_create: MappingAction[];
  to_skip_exists: MappingAction[];
  conflicts: MappingAction[];
  broken_to_remove: Array<{
    target_path: string;
    existing_target: string;
    reason: string;
  }>;
  total_actions: number;
  warnings: string[];
  errors: string[];
}

interface MappingPreviewProps {
  /** Version tag to preview mapping for */
  versionTag: string;
  /** Callback when preview is loaded */
  onPreviewLoaded?: (preview: MappingPreviewResponse) => void;
  /** Whether to auto-refresh on mount */
  autoRefresh?: boolean;
  /** Callback when mapping is applied */
  onMappingApplied?: (result: { links_created: number; links_removed: number }) => void;
  /** Whether to show the apply button */
  showApplyButton?: boolean;
}

export const MappingPreview: React.FC<MappingPreviewProps> = ({
  versionTag,
  onPreviewLoaded,
  autoRefresh = true,
  onMappingApplied,
  showApplyButton = true,
}) => {
  const [preview, setPreview] = useState<MappingPreviewResponse | null>(null);
  const [crossFsWarning, setCrossFsWarning] = useState<{
    cross_filesystem: boolean;
    warning?: string;
    recommendation?: string;
  } | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isApplying, setIsApplying] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandedSection, setExpandedSection] = useState<string | null>(null);
  const [applyResult, setApplyResult] = useState<{
    success: boolean;
    links_created: number;
    links_removed: number;
    error?: string;
  } | null>(null);

  const fetchPreview = useCallback(async () => {
    if (!window.pywebview?.api?.preview_model_mapping || !versionTag) {
      logger.warn('Preview API not available or no version tag');
      return;
    }

    setIsLoading(true);
    try {
      // Fetch both preview and cross-filesystem warning in parallel
      const [previewResult, crossFsResult] = await Promise.all([
        window.pywebview.api.preview_model_mapping(versionTag),
        window.pywebview.api.get_cross_filesystem_warning?.(versionTag),
      ]);

      if (previewResult.success) {
        setPreview(previewResult as MappingPreviewResponse);
        onPreviewLoaded?.(previewResult as MappingPreviewResponse);
      } else {
        logger.error('Failed to fetch preview', { error: previewResult.error });
      }

      if (crossFsResult?.success && crossFsResult.cross_filesystem) {
        setCrossFsWarning(crossFsResult as typeof crossFsWarning);
      } else {
        setCrossFsWarning(null);
      }
    } catch (error) {
      logger.error('Error fetching preview', { error });
    } finally {
      setIsLoading(false);
    }
  }, [versionTag, onPreviewLoaded]);

  const applyMapping = useCallback(async () => {
    if (!window.pywebview?.api?.apply_model_mapping || !versionTag) {
      logger.warn('Apply API not available or no version tag');
      return;
    }

    setIsApplying(true);
    setApplyResult(null);
    try {
      const result = await window.pywebview.api.apply_model_mapping(versionTag);
      setApplyResult({
        success: result.success,
        links_created: result.links_created || 0,
        links_removed: result.links_removed || 0,
        error: result.error,
      });

      if (result.success) {
        logger.info('Mapping applied successfully', {
          links_created: result.links_created,
          links_removed: result.links_removed,
        });
        onMappingApplied?.({
          links_created: result.links_created || 0,
          links_removed: result.links_removed || 0,
        });
        // Refresh preview after applying
        void fetchPreview();
      } else {
        logger.error('Failed to apply mapping', { error: result.error });
      }
    } catch (error) {
      logger.error('Error applying mapping', { error });
      setApplyResult({
        success: false,
        links_created: 0,
        links_removed: 0,
        error: error instanceof Error ? error.message : 'Unknown error',
      });
    } finally {
      setIsApplying(false);
    }
  }, [versionTag, onMappingApplied, fetchPreview]);

  useEffect(() => {
    if (autoRefresh && versionTag) {
      void fetchPreview();
    }
  }, [autoRefresh, versionTag, fetchPreview]);

  const toggleSection = (section: string) => {
    setExpandedSection(expandedSection === section ? null : section);
  };

  if (!preview && !isLoading) {
    return null;
  }

  const toCreateCount = preview?.to_create?.length || 0;
  const conflictCount = preview?.conflicts?.length || 0;
  const skipCount = preview?.to_skip_exists?.length || 0;
  const brokenCount = preview?.broken_to_remove?.length || 0;
  const warningCount = preview?.warnings?.length || 0;
  const hasIssues = conflictCount > 0 || warningCount > 0;

  // Determine overall status
  let status: 'ready' | 'warnings' | 'errors' = 'ready';
  if (preview?.errors?.length) {
    status = 'errors';
  } else if (hasIssues) {
    status = 'warnings';
  }

  const statusConfig = {
    ready: {
      icon: CheckCircle,
      color: 'text-[hsl(var(--accent-success))]',
      label: `${toCreateCount} links ready`,
    },
    warnings: {
      icon: AlertTriangle,
      color: 'text-[hsl(var(--accent-warning))]',
      label: `${conflictCount} conflict${conflictCount !== 1 ? 's' : ''}`,
    },
    errors: {
      icon: XCircle,
      color: 'text-[hsl(var(--accent-error))]',
      label: 'Configuration error',
    },
  };

  const config = statusConfig[status];
  const StatusIcon = config.icon;

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
      >
        <div className="flex items-center gap-3">
          <FolderSymlink className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            Mapping Preview
          </span>
          {isLoading ? (
            <RefreshCw className="w-4 h-4 animate-spin text-[hsl(var(--launcher-text-secondary))]" />
          ) : (
            <div className="flex items-center gap-2">
              <StatusIcon className={`w-4 h-4 ${config.color}`} />
              <span className={`text-xs ${config.color}`}>{config.label}</span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-2">
          {preview && (
            <span className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
              {preview.total_actions} action{preview.total_actions !== 1 ? 's' : ''}
            </span>
          )}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          ) : (
            <ChevronDown className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          )}
        </div>
      </button>

      {/* Expanded Content */}
      <AnimatePresence>
        {isExpanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-4 space-y-3">
              {/* Cross-Filesystem Warning */}
              {crossFsWarning?.cross_filesystem && (
                <div className="p-3 bg-[hsl(var(--accent-warning)/0.1)] rounded-lg border border-[hsl(var(--accent-warning)/0.3)]">
                  <div className="flex items-start gap-2">
                    <HardDrive className="w-4 h-4 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
                    <div>
                      <div className="text-sm font-medium text-[hsl(var(--accent-warning))]">
                        Cross-Filesystem Warning
                      </div>
                      <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
                        {crossFsWarning.warning}
                      </div>
                      {crossFsWarning.recommendation && (
                        <div className="text-xs text-[hsl(var(--launcher-text-tertiary))] mt-1">
                          Tip: {crossFsWarning.recommendation}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )}

              {/* Stats Grid */}
              {preview && (
                <div className="grid grid-cols-4 gap-2 text-center">
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className="text-lg font-semibold text-[hsl(var(--accent-success))]">
                      {toCreateCount}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">To Create</div>
                  </div>
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className="text-lg font-semibold text-[hsl(var(--launcher-text-tertiary))]">
                      {skipCount}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Existing</div>
                  </div>
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className={`text-lg font-semibold ${conflictCount > 0 ? 'text-[hsl(var(--accent-warning))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
                      {conflictCount}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Conflicts</div>
                  </div>
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className={`text-lg font-semibold ${brokenCount > 0 ? 'text-[hsl(var(--accent-error))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
                      {brokenCount}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Broken</div>
                  </div>
                </div>
              )}

              {/* Warnings */}
              {preview?.warnings && preview.warnings.length > 0 && (
                <div className="space-y-1">
                  {preview.warnings.map((warning, index) => (
                    <div
                      key={index}
                      className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)] flex items-start gap-2"
                    >
                      <AlertTriangle className="w-3 h-3 text-[hsl(var(--accent-warning))] flex-shrink-0 mt-0.5" />
                      <span className="text-[hsl(var(--launcher-text-secondary))]">{warning}</span>
                    </div>
                  ))}
                </div>
              )}

              {/* Links to Create Section */}
              {toCreateCount > 0 && (
                <div className="border border-[hsl(var(--launcher-border)/0.3)] rounded">
                  <button
                    onClick={() => toggleSection('create')}
                    className="w-full px-3 py-2 flex items-center justify-between text-left hover:bg-[hsl(var(--launcher-bg-secondary)/0.3)] transition-colors"
                  >
                    <div className="flex items-center gap-2">
                      <Plus className="w-3 h-3 text-[hsl(var(--accent-success))]" />
                      <span className="text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
                        Links to Create ({toCreateCount})
                      </span>
                    </div>
                    {expandedSection === 'create' ? (
                      <ChevronUp className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    ) : (
                      <ChevronDown className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    )}
                  </button>
                  <AnimatePresence>
                    {expandedSection === 'create' && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                      >
                        <div className="max-h-48 overflow-y-auto px-3 pb-2 space-y-1">
                          {preview?.to_create?.map((action, index) => (
                            <div
                              key={index}
                              className="text-xs p-2 bg-[hsl(var(--accent-success)/0.05)] rounded"
                            >
                              <div className="font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                                {action.model_name || action.model_id}
                              </div>
                              <div className="flex items-center gap-1 text-[hsl(var(--launcher-text-tertiary))] mt-1">
                                <Link className="w-3 h-3" />
                                <span className="truncate font-mono">{action.target_path.split('/').slice(-2).join('/')}</span>
                              </div>
                            </div>
                          ))}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              )}

              {/* Skipped (Existing) Section */}
              {skipCount > 0 && (
                <div className="border border-[hsl(var(--launcher-border)/0.3)] rounded">
                  <button
                    onClick={() => toggleSection('skip')}
                    className="w-full px-3 py-2 flex items-center justify-between text-left hover:bg-[hsl(var(--launcher-bg-secondary)/0.3)] transition-colors"
                  >
                    <div className="flex items-center gap-2">
                      <SkipForward className="w-3 h-3 text-[hsl(var(--launcher-text-tertiary))]" />
                      <span className="text-xs font-medium text-[hsl(var(--launcher-text-primary))]">
                        Already Linked ({skipCount})
                      </span>
                    </div>
                    {expandedSection === 'skip' ? (
                      <ChevronUp className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    ) : (
                      <ChevronDown className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    )}
                  </button>
                  <AnimatePresence>
                    {expandedSection === 'skip' && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                      >
                        <div className="max-h-32 overflow-y-auto px-3 pb-2 space-y-1">
                          {preview?.to_skip_exists?.map((action, index) => (
                            <div
                              key={index}
                              className="text-xs p-2 bg-[hsl(var(--launcher-bg-secondary)/0.3)] rounded font-mono truncate text-[hsl(var(--launcher-text-tertiary))]"
                            >
                              {action.target_path.split('/').pop()}
                            </div>
                          ))}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              )}

              {/* Conflicts Section */}
              {conflictCount > 0 && (
                <div className="border border-[hsl(var(--accent-warning)/0.3)] rounded">
                  <button
                    onClick={() => toggleSection('conflicts')}
                    className="w-full px-3 py-2 flex items-center justify-between text-left hover:bg-[hsl(var(--accent-warning)/0.1)] transition-colors"
                  >
                    <div className="flex items-center gap-2">
                      <AlertCircle className="w-3 h-3 text-[hsl(var(--accent-warning))]" />
                      <span className="text-xs font-medium text-[hsl(var(--accent-warning))]">
                        Conflicts ({conflictCount})
                      </span>
                    </div>
                    {expandedSection === 'conflicts' ? (
                      <ChevronUp className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    ) : (
                      <ChevronDown className="w-3 h-3 text-[hsl(var(--launcher-text-secondary))]" />
                    )}
                  </button>
                  <AnimatePresence>
                    {expandedSection === 'conflicts' && (
                      <motion.div
                        initial={{ height: 0, opacity: 0 }}
                        animate={{ height: 'auto', opacity: 1 }}
                        exit={{ height: 0, opacity: 0 }}
                        className="overflow-hidden"
                      >
                        <div className="max-h-48 overflow-y-auto px-3 pb-2 space-y-1">
                          {preview?.conflicts?.map((conflict, index) => (
                            <div
                              key={index}
                              className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)]"
                            >
                              <div className="font-medium text-[hsl(var(--launcher-text-primary))] truncate">
                                {conflict.model_name || conflict.model_id}
                              </div>
                              <div className="text-[hsl(var(--accent-warning))] mt-1">
                                {conflict.reason}
                              </div>
                              {conflict.existing_target && (
                                <div className="font-mono text-[hsl(var(--launcher-text-tertiary))] mt-1 truncate">
                                  → {conflict.existing_target}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              )}

              {/* Apply Result Message */}
              {applyResult && (
                <div
                  className={`p-3 rounded-lg border ${
                    applyResult.success
                      ? 'bg-[hsl(var(--accent-success)/0.1)] border-[hsl(var(--accent-success)/0.3)]'
                      : 'bg-[hsl(var(--accent-error)/0.1)] border-[hsl(var(--accent-error)/0.3)]'
                  }`}
                >
                  <div className="flex items-start gap-2">
                    {applyResult.success ? (
                      <CheckCircle className="w-4 h-4 text-[hsl(var(--accent-success))] flex-shrink-0 mt-0.5" />
                    ) : (
                      <XCircle className="w-4 h-4 text-[hsl(var(--accent-error))] flex-shrink-0 mt-0.5" />
                    )}
                    <div>
                      <div
                        className={`text-sm font-medium ${
                          applyResult.success
                            ? 'text-[hsl(var(--accent-success))]'
                            : 'text-[hsl(var(--accent-error))]'
                        }`}
                      >
                        {applyResult.success ? 'Mapping Applied' : 'Mapping Failed'}
                      </div>
                      <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
                        {applyResult.success ? (
                          <>
                            Created {applyResult.links_created} link
                            {applyResult.links_created !== 1 ? 's' : ''}
                            {applyResult.links_removed > 0 && (
                              <>, removed {applyResult.links_removed} broken</>
                            )}
                          </>
                        ) : (
                          applyResult.error || 'Unknown error occurred'
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* Action Buttons */}
              <div className="flex gap-2 pt-2">
                <button
                  onClick={() => void fetchPreview()}
                  disabled={isLoading || isApplying}
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
                >
                  <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
                  Refresh
                </button>
                {showApplyButton && toCreateCount > 0 && (
                  <button
                    onClick={() => void applyMapping()}
                    disabled={isLoading || isApplying || status === 'errors'}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-primary))] hover:bg-[hsl(var(--accent-primary-hover))] text-white rounded transition-colors disabled:opacity-50"
                  >
                    {isApplying ? (
                      <>
                        <RefreshCw className="w-3 h-3 animate-spin" />
                        Applying...
                      </>
                    ) : (
                      <>
                        <Link className="w-3 h-3" />
                        Apply Mapping
                      </>
                    )}
                  </button>
                )}
              </div>

              {/* Success Message */}
              {!hasIssues && preview && toCreateCount === 0 && skipCount > 0 && !applyResult && (
                <div className="text-xs text-center text-[hsl(var(--accent-success))] py-2 flex items-center justify-center gap-2">
                  <CheckCircle className="w-4 h-4" />
                  All models already linked
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
