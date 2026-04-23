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
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Link2,
} from 'lucide-react';
import type {
  LinkHealthResponse,
  HealthStatus,
} from '../types/api';
import { getLogger } from '../utils/logger';
import { LinkHealthDetails } from './LinkHealthDetails';

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
  const config = statusConfig[status];
  const StatusIcon = config.icon;
  const hasBrokenLinks = (health?.broken_links.length || 0) > 0;
  const hasOrphanedLinks = (health?.orphaned_links.length || 0) > 0;
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
            {health && (
              <LinkHealthDetails
                activeVersion={activeVersion}
                hasBrokenLinks={hasBrokenLinks}
                hasIssues={hasIssues}
                hasOrphanedLinks={hasOrphanedLinks}
                health={health}
                isCleaning={isCleaning}
                isLoading={isLoading}
                isRemovingOrphans={isRemovingOrphans}
                lastAction={lastAction}
                onCleanBrokenLinks={() => void handleCleanBrokenLinks()}
                onRefresh={() => void fetchHealth()}
                onRemoveOrphans={() => void handleRemoveOrphans()}
              />
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};
