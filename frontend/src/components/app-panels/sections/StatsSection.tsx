/**
 * Stats Section for GenericAppPanel.
 *
 * Displays real-time statistics from the app's API endpoint.
 * Configuration-driven: reads endpoint and mapping from plugin config.
 */

import { useState, useEffect, useCallback } from 'react';
import { Activity, HardDrive, Cpu, Loader2, AlertCircle } from 'lucide-react';
import { api, isAPIAvailable } from '../../../api/adapter';
import { getLogger } from '../../../utils/logger';

const logger = getLogger('StatsSection');

export interface StatsSectionConfig {
  showMemory?: boolean;
  showLoadedModels?: boolean;
  pollingIntervalMs?: number;
}

export interface StatsSectionProps {
  appId: string;
  config?: StatsSectionConfig;
  isRunning: boolean;
}

interface StatsData {
  loadedModels?: Array<{ name: string; size?: number }>;
  memoryUsed?: number;
  memoryTotal?: number;
}

export function StatsSection({
  appId,
  config = {},
  isRunning,
}: StatsSectionProps) {
  const [stats, setStats] = useState<StatsData | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const {
    showMemory = true,
    showLoadedModels = true,
    pollingIntervalMs = 5000,
  } = config;

  const fetchStats = useCallback(async () => {
    if (!isAPIAvailable() || !isRunning) {
      setStats(null);
      return;
    }

    try {
      setIsLoading(true);
      setError(null);

      const result = await api.call_plugin_endpoint(appId, 'stats', {});

      if (result.success && result.data) {
        // Map the response to our stats format
        const data = result.data as Record<string, unknown>;
        const models = data['models'] as Array<{ name: string; size?: number }> | undefined;

        setStats({
          loadedModels: models,
          memoryUsed: data['memoryUsed'] as number | undefined,
          memoryTotal: data['memoryTotal'] as number | undefined,
        });
      } else {
        setError(result.error || 'Failed to fetch stats');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      logger.debug('Stats fetch error', { appId, error: message });
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, [appId, isRunning]);

  useEffect(() => {
    if (!isRunning) {
      setStats(null);
      return;
    }

    void fetchStats();
    const interval = setInterval(() => void fetchStats(), pollingIntervalMs);
    return () => clearInterval(interval);
  }, [fetchStats, isRunning, pollingIntervalMs]);

  if (!isRunning) {
    return null;
  }

  if (isLoading && !stats) {
    return (
      <div className="w-full flex items-center gap-2 text-[hsl(var(--text-secondary))] py-2">
        <Loader2 className="w-4 h-4 animate-spin" />
        <span className="text-sm">Loading stats...</span>
      </div>
    );
  }

  if (error && !stats) {
    return (
      <div className="w-full flex items-center gap-2 text-[hsl(var(--accent-warning))] py-2">
        <AlertCircle className="w-4 h-4" />
        <span className="text-sm">Unable to fetch stats</span>
      </div>
    );
  }

  const formatBytes = (bytes: number): string => {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
    return `${(bytes / 1e3).toFixed(1)} KB`;
  };

  return (
    <div className="w-full space-y-3">
      <div className="text-xs uppercase tracking-wider text-[hsl(var(--launcher-text-muted))] flex items-center gap-2">
        <Activity className="w-3.5 h-3.5" />
        <span>Live Stats</span>
        {isLoading && <Loader2 className="w-3 h-3 animate-spin ml-auto" />}
      </div>

      <div className="grid grid-cols-2 gap-3">
        {showLoadedModels && stats?.loadedModels !== undefined && (
          <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.4)] border border-[hsl(var(--launcher-border)/0.5)]">
            <Cpu className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
            <div className="flex flex-col">
              <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Loaded Models</span>
              <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                {stats.loadedModels.length}
              </span>
            </div>
          </div>
        )}

        {showMemory && stats?.memoryUsed !== undefined && (
          <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-[hsl(var(--launcher-bg-secondary)/0.4)] border border-[hsl(var(--launcher-border)/0.5)]">
            <HardDrive className="w-4 h-4 text-[hsl(var(--launcher-text-secondary))]" />
            <div className="flex flex-col">
              <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Memory</span>
              <span className="text-sm font-medium text-[hsl(var(--launcher-text-primary))]">
                {formatBytes(stats.memoryUsed)}
                {stats.memoryTotal && ` / ${formatBytes(stats.memoryTotal)}`}
              </span>
            </div>
          </div>
        )}
      </div>

      {showLoadedModels && stats?.loadedModels && stats.loadedModels.length > 0 && (
        <div className="space-y-1.5">
          <span className="text-xs text-[hsl(var(--launcher-text-muted))]">Loaded:</span>
          <div className="flex flex-wrap gap-1.5">
            {stats.loadedModels.map((model, i) => (
              <span
                key={`${model.name}-${i}`}
                className="px-2 py-0.5 text-xs rounded bg-[hsl(var(--launcher-accent-primary)/0.15)] text-[hsl(var(--launcher-accent-primary))]"
              >
                {model.name}
                {model.size && ` (${formatBytes(model.size)})`}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
