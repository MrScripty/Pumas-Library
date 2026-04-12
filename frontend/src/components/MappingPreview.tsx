/**
 * Mapping Preview Component (Phase 1C)
 *
 * Displays a preview of model mapping operations that will be performed,
 * showing links to create, conflicts, and warnings before applying.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  FolderSymlink,
} from 'lucide-react';
import { getLogger } from '../utils/logger';
import { MappingPreviewDetails } from './MappingPreviewDetails';
import type { MappingPreviewResponse } from './MappingPreviewTypes';

const logger = getLogger('MappingPreview');

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
  const [error, setError] = useState<string | null>(null);
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
    if (!isAPIAvailable() || !versionTag) {
      logger.warn('Preview API not available or no version tag');
      return;
    }

    setIsLoading(true);
    setError(null);
    try {
      // Fetch both preview and cross-filesystem warning in parallel
      const [previewResult, crossFsResult] = await Promise.all([
        api.preview_model_mapping(versionTag),
        api.get_cross_filesystem_warning?.(versionTag),
      ]);

      if (previewResult.success) {
        setPreview(previewResult as MappingPreviewResponse);
        setError(null);
        onPreviewLoaded?.(previewResult as MappingPreviewResponse);
      } else {
        logger.error('Failed to fetch preview', { error: previewResult.error });
        setError(previewResult.error || 'Preview returned an error');
      }

      if (crossFsResult?.success && crossFsResult.cross_filesystem) {
        setCrossFsWarning(crossFsResult as typeof crossFsWarning);
      } else {
        setCrossFsWarning(null);
      }
    } catch (err) {
      logger.error('Error fetching preview', { error: err });
      setError(err instanceof Error ? err.message : 'Failed to load mapping preview');
    } finally {
      setIsLoading(false);
    }
  }, [versionTag, onPreviewLoaded]);

  const applyMapping = useCallback(async () => {
    if (!isAPIAvailable() || !versionTag) {
      logger.warn('Apply API not available or no version tag');
      return;
    }

    setIsApplying(true);
    setApplyResult(null);
    try {
      const result = await api.apply_model_mapping(versionTag);
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
    if (error) {
      return (
        <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--accent-error)/0.5)] p-4">
          <div className="flex items-start gap-3">
            <XCircle className="w-5 h-5 text-[hsl(var(--accent-error))] flex-shrink-0 mt-0.5" />
            <div className="flex-1">
              <div className="text-sm font-medium text-[hsl(var(--accent-error))]">
                Failed to load mapping preview
              </div>
              <div className="text-xs text-[hsl(var(--launcher-text-secondary))] mt-1">
                {error}
              </div>
              <button
                onClick={() => void fetchPreview()}
                className="mt-3 flex items-center gap-2 px-3 py-1.5 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors"
              >
                <RefreshCw className="w-3 h-3" />
                Retry
              </button>
            </div>
          </div>
        </div>
      );
    }
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
            {preview && (
              <MappingPreviewDetails
                applyResult={applyResult}
                brokenCount={brokenCount}
                conflictCount={conflictCount}
                crossFsWarning={crossFsWarning}
                expandedSection={expandedSection}
                hasIssues={hasIssues}
                isApplying={isApplying}
                isLoading={isLoading}
                preview={preview}
                showApplyButton={showApplyButton}
                skipCount={skipCount}
                status={status}
                toCreateCount={toCreateCount}
                onApplyMapping={() => void applyMapping()}
                onFetchPreview={() => void fetchPreview()}
                onToggleSection={toggleSection}
              />
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
