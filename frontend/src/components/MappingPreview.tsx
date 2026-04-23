/**
 * Mapping Preview Component (Phase 1C)
 *
 * Displays a preview of model mapping operations that will be performed,
 * showing links to create, conflicts, and warnings before applying.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import { getLogger } from '../utils/logger';
import { MappingPreviewDetails } from './MappingPreviewDetails';
import { MappingPreviewHeader } from './MappingPreviewHeader';
import {
  getMappingPreviewCounts,
  getMappingPreviewStatus,
  hasMappingPreviewIssues,
} from './MappingPreviewState';
import { MappingPreviewUnavailableState } from './MappingPreviewUnavailableState';
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
        api.get_cross_filesystem_warning(versionTag),
      ]);

      if (previewResult.success) {
        setPreview(previewResult as MappingPreviewResponse);
        setError(null);
        onPreviewLoaded?.(previewResult as MappingPreviewResponse);
      } else {
        logger.error('Failed to fetch preview', { error: previewResult.error });
        setError(previewResult.error || 'Preview returned an error');
      }

      if (crossFsResult.success && crossFsResult.cross_filesystem) {
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
    return <MappingPreviewUnavailableState error={error} onRetry={() => void fetchPreview()} />;
  }

  const counts = getMappingPreviewCounts(preview);
  const hasIssues = hasMappingPreviewIssues(counts);
  const status = getMappingPreviewStatus(preview, counts);

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      <MappingPreviewHeader
        counts={counts}
        isExpanded={isExpanded}
        isLoading={isLoading}
        preview={preview}
        status={status}
        onToggleExpanded={() => setIsExpanded(!isExpanded)}
      />

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
                brokenCount={counts.brokenCount}
                conflictCount={counts.conflictCount}
                crossFsWarning={crossFsWarning}
                expandedSection={expandedSection}
                hasIssues={hasIssues}
                isApplying={isApplying}
                isLoading={isLoading}
                preview={preview}
                showApplyButton={showApplyButton}
                skipCount={counts.skipCount}
                status={status}
                toCreateCount={counts.toCreateCount}
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
