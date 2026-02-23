/**
 * Link Health Status Component (Phase 1B)
 *
 * Displays the health status of model symlinks with actions
 * to clean broken links and remove orphaned files.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  Trash2,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Link2,
  AlertCircle,
} from 'lucide-react';
import type {
  LinkHealthResponse,
  BrokenLinkInfo,
  HealthStatus,
} from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('LinkHealthStatus');

interface LinkHealthStatusProps {
  /** Current active version tag for orphan detection */
  activeVersion?: string | null;
  /** Whether to auto-refresh on mount */
  autoRefresh?: boolean;
}

const statusConfig: Record<
  HealthStatus,
  { icon: typeof CheckCircle; color: string; label: string }
> = {
  healthy: {
    icon: CheckCircle,
    color: 'text-[hsl(var(--accent-success))]',
    label: 'All links healthy',
  },
  warnings: {
    icon: AlertTriangle,
    color: 'text-[hsl(var(--accent-warning))]',
    label: 'Warnings detected',
  },
  errors: {
    icon: XCircle,
    color: 'text-[hsl(var(--accent-error))]',
    label: 'Errors detected',
  },
};

export const LinkHealthStatus: React.FC<LinkHealthStatusProps> = ({
  activeVersion,
  autoRefresh = true,
}) => {
  const [health, setHealth] = useState<LinkHealthResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isCleaning, setIsCleaning] = useState(false);
  const [isRemovingOrphans, setIsRemovingOrphans] = useState(false);
  const [lastAction, setLastAction] = useState<string | null>(null);

  const fetchHealth = useCallback(async () => {
    if (!isAPIAvailable()) {
      logger.warn('Link health API not available');
      return;
    }

    setIsLoading(true);
    try {
      const result = await api.get_link_health(activeVersion);
      if (result.success) {
        setHealth(result);
      } else {
        logger.error('Failed to fetch link health', { error: result.error });
      }
    } catch (error) {
      logger.error('Error fetching link health', { error });
    } finally {
      setIsLoading(false);
    }
  }, [activeVersion]);

  useEffect(() => {
    if (autoRefresh) {
      void fetchHealth();
    }
  }, [autoRefresh, fetchHealth]);

  const handleCleanBrokenLinks = async () => {
    if (!isAPIAvailable()) return;

    setIsCleaning(true);
    setLastAction(null);
    try {
      const result = await api.clean_broken_links();
      if (result.success) {
        setLastAction(`Cleaned ${result.cleaned} broken link${result.cleaned !== 1 ? 's' : ''}`);
        await fetchHealth();
      } else {
        logger.error('Failed to clean broken links', { error: result.error });
        setLastAction('Failed to clean broken links');
      }
    } catch (error) {
      logger.error('Error cleaning broken links', { error });
      setLastAction('Error cleaning broken links');
    } finally {
      setIsCleaning(false);
    }
  };

  const handleRemoveOrphans = async () => {
    if (!isAPIAvailable() || !activeVersion) return;

    setIsRemovingOrphans(true);
    setLastAction(null);
    try {
      const result = await api.remove_orphaned_links(activeVersion);
      if (result.success) {
        setLastAction(`Removed ${result.removed} orphaned link${result.removed !== 1 ? 's' : ''}`);
        await fetchHealth();
      } else {
        logger.error('Failed to remove orphaned links', { error: result.error });
        setLastAction('Failed to remove orphaned links');
      }
    } catch (error) {
      logger.error('Error removing orphaned links', { error });
      setLastAction('Error removing orphaned links');
    } finally {
      setIsRemovingOrphans(false);
    }
  };

  if (!health && !isLoading) {
    return null;
  }

  const status = health?.status || 'healthy';
  const config = statusConfig[status] ?? statusConfig['healthy'];
  const StatusIcon = config.icon;
  const hasBrokenLinks = (health?.broken_links?.length || 0) > 0;
  const hasOrphanedLinks = (health?.orphaned_links?.length || 0) > 0;
  const hasIssues = hasBrokenLinks || hasOrphanedLinks;

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
      >
        <div className="flex items-center gap-3">
          <Link2 className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            Link Health
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
          {health && (
            <span className="text-xs text-[hsl(var(--launcher-text-tertiary))]">
              {health.total_links} link{health.total_links !== 1 ? 's' : ''}
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
              {/* Stats */}
              {health && (
                <div className="grid grid-cols-3 gap-2 text-center">
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">
                      {health.healthy_links}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Healthy</div>
                  </div>
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className={`text-lg font-semibold ${hasBrokenLinks ? 'text-[hsl(var(--accent-error))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
                      {health.broken_links?.length || 0}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Broken</div>
                  </div>
                  <div className="p-2 bg-[hsl(var(--launcher-bg-secondary)/0.5)] rounded">
                    <div className={`text-lg font-semibold ${hasOrphanedLinks ? 'text-[hsl(var(--accent-warning))]' : 'text-[hsl(var(--launcher-text-primary))]'}`}>
                      {health.orphaned_links?.length || 0}
                    </div>
                    <div className="text-xs text-[hsl(var(--launcher-text-secondary))]">Orphaned</div>
                  </div>
                </div>
              )}

              {/* Broken Links List */}
              {hasBrokenLinks && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-[hsl(var(--accent-error))] flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    Broken Links
                  </div>
                  <div className="max-h-32 overflow-y-auto space-y-1">
                    {health?.broken_links?.map((link: BrokenLinkInfo) => (
                      <div
                        key={link.link_id}
                        className="text-xs p-2 bg-[hsl(var(--accent-error)/0.1)] rounded border border-[hsl(var(--accent-error)/0.2)]"
                      >
                        <div className="font-mono truncate text-[hsl(var(--launcher-text-primary))]">
                          {link.target_path}
                        </div>
                        <div className="text-[hsl(var(--launcher-text-tertiary))]">
                          {link.reason}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Orphaned Links List */}
              {hasOrphanedLinks && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-[hsl(var(--accent-warning))] flex items-center gap-1">
                    <AlertTriangle className="w-3 h-3" />
                    Orphaned Links
                  </div>
                  <div className="max-h-32 overflow-y-auto space-y-1">
                    {health?.orphaned_links?.map((path: string, index: number) => (
                      <div
                        key={index}
                        className="text-xs p-2 bg-[hsl(var(--accent-warning)/0.1)] rounded border border-[hsl(var(--accent-warning)/0.2)] font-mono truncate"
                      >
                        {path}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Action Buttons */}
              <div className="flex gap-2 pt-2">
                <button
                  onClick={() => void fetchHealth()}
                  disabled={isLoading}
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--launcher-bg-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] rounded transition-colors disabled:opacity-50"
                >
                  <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
                  Refresh
                </button>
                {hasBrokenLinks && (
                  <button
                    onClick={() => void handleCleanBrokenLinks()}
                    disabled={isCleaning}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-error)/0.2)] hover:bg-[hsl(var(--accent-error)/0.3)] text-[hsl(var(--accent-error))] rounded transition-colors disabled:opacity-50"
                  >
                    <Trash2 className={`w-3 h-3 ${isCleaning ? 'animate-spin' : ''}`} />
                    Clean Broken
                  </button>
                )}
                {hasOrphanedLinks && activeVersion && (
                  <button
                    onClick={() => void handleRemoveOrphans()}
                    disabled={isRemovingOrphans}
                    className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs bg-[hsl(var(--accent-warning)/0.2)] hover:bg-[hsl(var(--accent-warning)/0.3)] text-[hsl(var(--accent-warning))] rounded transition-colors disabled:opacity-50"
                  >
                    <Trash2 className={`w-3 h-3 ${isRemovingOrphans ? 'animate-spin' : ''}`} />
                    Remove Orphans
                  </button>
                )}
              </div>

              {/* Last Action Message */}
              {lastAction && (
                <div className="text-xs text-center text-[hsl(var(--launcher-text-secondary))] py-1">
                  {lastAction}
                </div>
              )}

              {/* No Issues Message */}
              {!hasIssues && health && (
                <div className="text-xs text-center text-[hsl(var(--accent-success))] py-2 flex items-center justify-center gap-2">
                  <CheckCircle className="w-4 h-4" />
                  All symlinks are healthy
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
