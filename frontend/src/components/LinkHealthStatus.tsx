/**
 * Link Health Status Component (Phase 1B)
 *
 * Displays the health status of model symlinks with actions
 * to clean broken links and remove orphaned files.
 */

import React, { useState, useCallback, useEffect, useRef } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import {
  AlertTriangle,
  CheckCircle,
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
type InvocationOutcome = 'applied' | 'unavailable' | 'superseded';

interface LinkHealthStatusProps {
  /** Presentation scope; the registered-link health read is registry-wide. */
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
  degraded: {
    icon: AlertTriangle,
    color: 'text-[hsl(var(--accent-warning))]',
    label: 'Issues detected',
  },
};

export const LinkHealthStatus: React.FC<LinkHealthStatusProps> = (props) => (
  // Each version owns its report and actions, including a later return to an earlier version.
  <ScopedLinkHealthStatus key={props.activeVersion ?? ''} {...props} />
);

const ScopedLinkHealthStatus: React.FC<LinkHealthStatusProps> = ({
  activeVersion,
  autoRefresh = true,
}) => {
  const [health, setHealth] = useState<LinkHealthResponse | null>(null);
  const [isUnavailable, setIsUnavailable] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [isCleaning, setIsCleaning] = useState(false);
  const [isRemovingOrphans, setIsRemovingOrphans] = useState(false);
  const [lastAction, setLastAction] = useState<string | null>(null);
  const scope = useRef<object | null>(null);
  const currentRead = useRef<object | null>(null);

  useEffect(() => {
    scope.current = {};
    return () => {
      scope.current = null;
      currentRead.current = null;
    };
  }, []);

  // IPC has no cancellation handle. Observe every completion, but only the current
  // invocation may publish; teardown revokes that authority without detaching work.
  const fetchHealth = useCallback(async (): Promise<InvocationOutcome> => {
    if (!scope.current) return 'superseded';
    const invocation = {};
    currentRead.current = invocation;
    setHealth(null);
    setIsUnavailable(false);
    if (!isAPIAvailable()) {
      logger.warn('Link health API not available');
      setIsUnavailable(true);
      setIsLoading(false);
      return 'unavailable';
    }

    setIsLoading(true);
    try {
      const result = await api.get_link_health(activeVersion);
      if (currentRead.current !== invocation) return 'superseded';
      setHealth(result);
      return 'applied';
    } catch (error) {
      if (currentRead.current !== invocation) return 'superseded';
      logger.error('Error fetching link health', { error });
      setIsUnavailable(true);
      return 'unavailable';
    } finally {
      if (currentRead.current === invocation) setIsLoading(false);
    }
  }, [activeVersion]);

  useEffect(() => {
    if (autoRefresh) {
      void fetchHealth();
    } else {
      setIsLoading(false);
    }
    return () => { currentRead.current = null; };
  }, [autoRefresh, fetchHealth]);

  const handleCleanBrokenLinks = async (): Promise<InvocationOutcome> => {
    const invocationScope = scope.current;
    if (!invocationScope) return 'superseded';
    if (!isAPIAvailable()) return 'unavailable';

    setIsCleaning(true);
    setLastAction(null);
    try {
      const result = await api.clean_broken_links();
      if (scope.current !== invocationScope) return 'superseded';
      if (result.success) {
        setLastAction(`Cleaned ${result.cleaned} broken link${result.cleaned !== 1 ? 's' : ''}`);
        return await fetchHealth();
      } else {
        logger.error('Failed to clean broken links', { error: result.error });
        setLastAction('Failed to clean broken links');
        return 'unavailable';
      }
    } catch (error) {
      if (scope.current !== invocationScope) return 'superseded';
      logger.error('Error cleaning broken links', { error });
      setLastAction('Error cleaning broken links');
      return 'unavailable';
    } finally {
      if (scope.current === invocationScope) setIsCleaning(false);
    }
  };

  const handleRemoveOrphans = async (): Promise<InvocationOutcome> => {
    const invocationScope = scope.current;
    if (!invocationScope) return 'superseded';
    if (!isAPIAvailable() || !activeVersion) return 'unavailable';

    setIsRemovingOrphans(true);
    setLastAction(null);
    try {
      const result = await api.remove_orphaned_links(activeVersion);
      if (scope.current !== invocationScope) return 'superseded';
      if (result.success) {
        setLastAction(`Removed ${result.removed} orphaned link${result.removed !== 1 ? 's' : ''}`);
        return await fetchHealth();
      } else {
        logger.error('Failed to remove orphaned links', { error: result.error });
        setLastAction('Failed to remove orphaned links');
        return 'unavailable';
      }
    } catch (error) {
      if (scope.current !== invocationScope) return 'superseded';
      logger.error('Error removing orphaned links', { error });
      setLastAction('Error removing orphaned links');
      return 'unavailable';
    } finally {
      if (scope.current === invocationScope) setIsRemovingOrphans(false);
    }
  };

  const config = health ? statusConfig[health.status] : null;
  const StatusIcon = config?.icon;
  const hasBrokenLinks = (health?.broken_links.length || 0) > 0;
  const hasOrphanedLinks = (health?.orphaned_links.length || 0) > 0;
  const hasIssues = hasBrokenLinks || hasOrphanedLinks;

  return (
    <div className="bg-[hsl(var(--launcher-bg-tertiary)/0.3)] rounded-lg border border-[hsl(var(--launcher-border)/0.5)]">
      {/* Header */}
      <button
        aria-expanded={isExpanded}
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full px-4 py-3 flex items-center justify-between hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors rounded-lg"
      >
        <div className="flex items-center gap-3">
          <Link2 className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
          <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
            Link Health
          </span>
          {isLoading ? (
            <span role="status" className="text-sm text-[hsl(var(--launcher-text-secondary))]">Checking link health…</span>
          ) : config && StatusIcon ? (
            <div className="flex items-center gap-2">
              <StatusIcon className={`w-4 h-4 ${config.color}`} />
              <span className={`text-xs ${config.color}`}>{config.label}</span>
            </div>
          ) : null}
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
      {!health && !isLoading && (
        <div className="px-4 pb-4 space-y-2">
          <p role={isUnavailable ? 'alert' : 'status'} className="text-sm text-[hsl(var(--launcher-text-secondary))]">
            {isUnavailable ? 'Link health unavailable' : 'Link health not checked'}
          </p>
          <button onClick={() => void fetchHealth()} className="rounded border border-[hsl(var(--launcher-border))] px-3 py-2 text-sm text-[hsl(var(--launcher-text-primary))] hover:bg-[hsl(var(--launcher-bg-tertiary))] focus-visible:outline focus-visible:outline-2">
            {isUnavailable ? 'Retry link health' : 'Check link health'}
          </button>
        </div>
      )}

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
