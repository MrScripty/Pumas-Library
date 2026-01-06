import React from 'react';
import { WifiOff, RefreshCw, Clock, Database, Download, Package } from 'lucide-react';
import { formatSpeed } from '../utils/formatters';

interface InstallationProgress {
  tag: string;
  started_at: string;
  stage: 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress: number;
  overall_progress: number;
  current_item: string | null;
  download_speed: number | null;
  eta_seconds: number | null;
  total_size: number | null;
  downloaded_bytes: number;
  dependency_count: number | null;
  completed_dependencies: number;
  completed_items: Array<{
    name: string;
    type: string;
    size: number | null;
    completed_at: string;
  }>;
  error: string | null;
  completed_at?: string;
  success?: boolean;
  log_path?: string | null;
}

interface StatusFooterProps {
  cacheStatus: {
    has_cache: boolean;
    is_valid: boolean;
    is_fetching: boolean;
    age_seconds?: number;
    last_fetched?: string;
    releases_count?: number;
  };
  installationProgress?: InstallationProgress | null;
}

export const StatusFooter: React.FC<StatusFooterProps> = ({ cacheStatus, installationProgress }) => {
  // Debug logging to trace cache status issues
  React.useEffect(() => {
    console.log('[StatusFooter] Cache status updated:', cacheStatus);
  }, [cacheStatus]);

  const getStatusInfo = () => {
    // INSTALLATION IN PROGRESS STATE - Priority 1
    if (installationProgress && !installationProgress.completed_at) {
      // During download stage with speed available
      if (installationProgress.stage === 'download' && installationProgress.download_speed !== null) {
        return {
          icon: Download,
          text: `Downloading at ${formatSpeed(installationProgress.download_speed)} · ${installationProgress.overall_progress}% complete`,
          color: 'text-accent-info',
          bgColor: 'bg-[hsl(var(--accent-info)/0.1)]',
          spinning: false
        };
      }

      // During dependencies stage
      if (installationProgress.stage === 'dependencies') {
        const packageInfo = installationProgress.dependency_count !== null
          ? `${installationProgress.completed_dependencies}/${installationProgress.dependency_count} packages`
          : 'Installing packages';

        const speedInfo = installationProgress.download_speed !== null
          ? ` · ${formatSpeed(installationProgress.download_speed)}`
          : '';

        return {
          icon: Package,
          text: `Installing · ${packageInfo}${speedInfo}`,
          color: 'text-accent-info',
          bgColor: 'bg-[hsl(var(--accent-info)/0.1)]',
          spinning: false
        };
      }

      // Other installation stages (extract, venv, setup)
      const stageNames = {
        extract: 'Extracting',
        venv: 'Creating environment',
        setup: 'Finalizing setup'
      };

      const stageName = stageNames[installationProgress.stage as keyof typeof stageNames] || 'Installing';

      return {
        icon: Download,
        text: `${stageName} · ${installationProgress.overall_progress}% complete`,
        color: 'text-accent-info',
        bgColor: 'bg-[hsl(var(--accent-info)/0.1)]',
        spinning: false
      };
    }

    // FETCHING STATE
    if (cacheStatus.is_fetching) {
      return {
        icon: RefreshCw,
        text: 'Fetching releases...',
        color: 'text-accent-info',
        bgColor: 'bg-[hsl(var(--accent-info)/0.1)]',
        spinning: true
      };
    }

    // NO CACHE STATE
    if (!cacheStatus.has_cache) {
      return {
        icon: WifiOff,
        text: 'No cache available - offline mode',
        color: 'text-accent-warning',
        bgColor: 'bg-[hsl(var(--accent-warning)/0.1)]',
        spinning: false
      };
    }

    // VALID CACHE STATE
    if (cacheStatus.is_valid) {
      const ageMinutes = cacheStatus.age_seconds
        ? Math.floor(cacheStatus.age_seconds / 60)
        : 0;

      return {
        icon: Database,
        text: `Cached data (${ageMinutes}m old) · ${cacheStatus.releases_count || 0} releases`,
        color: 'text-accent-success',
        bgColor: 'bg-[hsl(var(--accent-success)/0.1)]',
        spinning: false
      };
    }

    // STALE CACHE STATE
    const ageHours = cacheStatus.age_seconds
      ? Math.floor(cacheStatus.age_seconds / 3600)
      : 0;

    return {
      icon: Clock,
      text: `Stale cache (${ageHours}h old) · offline mode`,
      color: 'text-accent-warning',
      bgColor: 'bg-[hsl(var(--accent-warning)/0.1)]',
      spinning: false
    };
  };

  const status = getStatusInfo();
  const Icon = status.icon;

  return (
    <div className={`
      fixed bottom-0 left-0 right-0
      ${status.bgColor} border-t border-[hsl(var(--border-default))]/50
      px-4 py-2 flex items-center gap-2
      text-xs font-medium ${status.color}
      z-50
    `}>
      <Icon
        className={`w-3.5 h-3.5 ${status.spinning ? 'animate-spin' : ''}`}
      />
      <span>{status.text}</span>
    </div>
  );
};
