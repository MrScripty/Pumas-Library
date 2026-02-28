import React from 'react';
import { X, Minus, Cpu, Gpu, BicepsFlexed, RefreshCw, WifiOff, Clock, Database, Download, Package, ArrowUp } from 'lucide-react';
import type { SystemResources } from '../types/apps';
import { formatSpeed, formatBytes } from '../utils/formatters';
import { Tooltip, IconButton } from './ui';
import type { ActiveModelDownload } from '../hooks/useActiveModelDownload';

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

interface HeaderProps {
  systemResources?: SystemResources;
  appResources?: {
    gpu_memory?: number;
    ram_memory?: number;
  };
  launcherUpdateAvailable: boolean;
  onMinimize: () => void;
  onClose: () => void;
  networkAvailable?: boolean | null;
  modelLibraryLoaded?: boolean | null;
  installationProgress?: InstallationProgress | null;
  activeModelDownload?: ActiveModelDownload | null;
  activeModelDownloadCount?: number;
}

// Helper to get color based on load percentage
const getLoadColor = (load: number): string => {
  if (load < 30) return 'hsl(var(--accent-info))'; // Blue for low load
  if (load < 70) return 'hsl(var(--accent-warning))'; // Purple/Yellow for medium load
  return 'hsl(var(--accent-error))'; // Red for high load
};


export const Header: React.FC<HeaderProps> = ({
  systemResources,
  appResources,
  launcherUpdateAvailable,
  onMinimize,
  onClose,
  networkAvailable,
  modelLibraryLoaded,
  installationProgress,
  activeModelDownload,
  activeModelDownloadCount = 0,
}) => {
  const cpuUsage = Math.round(systemResources?.cpu?.usage ?? 0);
  const gpuUsage = Math.round(systemResources?.gpu?.usage ?? 0);
  const ramPercent = Math.round(systemResources?.ram?.usage ?? 0);
  const ramTotal = systemResources?.ram?.total ?? 0;
  const ramUsed = (ramTotal * ramPercent) / 100;
  const vramTotal = systemResources?.gpu?.memory_total ?? 0;
  const vramUsedSystem = systemResources?.gpu?.memory ?? 0;
  const vramUsed = Math.max(vramUsedSystem, appResources?.gpu_memory ?? 0);
  const vramPercent = vramTotal > 0 ? Math.min(100, Math.round((vramUsed / vramTotal) * 100)) : 0;

  // Get status info (same logic as StatusFooter)
  const getStatusInfo = () => {
    // INSTALLATION IN PROGRESS STATE - Priority 1
    if (installationProgress && !installationProgress.completed_at) {
      if (installationProgress.stage === 'download' && installationProgress.download_speed !== null) {
        return {
          icon: Download,
          text: `Downloading at ${formatSpeed(installationProgress.download_speed)} · ${installationProgress.overall_progress}% complete`,
          spinning: false
        };
      }

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
          spinning: false
        };
      }

      const stageNames = {
        extract: 'Extracting',
        venv: 'Creating environment',
        setup: 'Finalizing setup'
      };

      const stageName = stageNames[installationProgress.stage as keyof typeof stageNames] || 'Installing';

      return {
        icon: Download,
        text: `${stageName} · ${installationProgress.overall_progress}% complete`,
        spinning: false
      };
    }

    // MODEL DOWNLOAD IN PROGRESS STATE - Priority 2
    if (activeModelDownload) {
      if (activeModelDownload.status === 'downloading' && activeModelDownloadCount > 0) {
        const modelLabel = activeModelDownloadCount === 1 ? 'model' : 'models';
        return {
          icon: Download,
          text: `Downloading ${activeModelDownloadCount} ${modelLabel}`,
          spinning: false
        };
      }

      const progress = Math.max(0, Math.min(100, Math.round(activeModelDownload.progress)));
      const modelName = activeModelDownload.repoId?.split('/').pop() || activeModelDownload.repoId || 'model';

      if (activeModelDownload.status === 'downloading') {
        const speedInfo = activeModelDownload.speed && activeModelDownload.speed > 0
          ? ` at ${formatSpeed(activeModelDownload.speed)}`
          : '';
        const bytesInfo = !speedInfo && activeModelDownload.downloadedBytes !== null && activeModelDownload.totalBytes !== null
          ? ` · ${formatBytes(activeModelDownload.downloadedBytes)} / ${formatBytes(activeModelDownload.totalBytes)}`
          : '';

        return {
          icon: Download,
          text: `Downloading ${modelName}${speedInfo} · ${progress}%${bytesInfo}`,
          spinning: false
        };
      }

      if (activeModelDownload.status === 'queued') {
        return {
          icon: Download,
          text: `Queued model download · ${modelName}`,
          spinning: false
        };
      }

      if (activeModelDownload.status === 'pausing') {
        return {
          icon: Download,
          text: `Pausing model download · ${modelName}`,
          spinning: false
        };
      }

      return {
        icon: Download,
        text: `Cancelling model download · ${modelName}`,
        spinning: false
      };
    }

    if (networkAvailable === null || networkAvailable === undefined || modelLibraryLoaded === null || modelLibraryLoaded === undefined) {
      return {
        icon: RefreshCw,
        text: 'Checking network and model library...',
        spinning: true
      };
    }

    if (!networkAvailable) {
      return {
        icon: WifiOff,
        text: 'Network unavailable',
        spinning: false
      };
    }

    if (!modelLibraryLoaded) {
      return {
        icon: Clock,
        text: 'Model library database unavailable',
        spinning: false
      };
    }

    return {
      icon: Database,
      text: 'Network online · model library ready',
      spinning: false
    };
  };

  const status = getStatusInfo();
  const StatusIcon = status.icon;

  return (
    <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary)/0.3)] backdrop-blur-sm relative z-10 app-region-drag">
      {/* Main row: Single compact line with all controls */}
      <div className="h-8 px-3 pt-1 flex items-center justify-between gap-3">
        {/* Left: App name with update button */}
        <div className="flex items-center gap-2 flex-shrink-0 app-region-no-drag">
          <span className="text-xs font-semibold text-[hsl(var(--text-primary))]">AI Manager</span>
          {launcherUpdateAvailable ? (
            <IconButton
              icon={<ArrowUp className="text-[hsl(var(--accent-success))]" />}
              tooltip="Update available"
              size="sm"
            />
          ) : (
            <IconButton
              icon={<RefreshCw />}
              tooltip="Check for updates"
              size="sm"
            />
          )}
        </div>

        {/* Center: Status badge */}
        <div className="flex-1 flex items-center justify-center min-w-0 app-region-no-drag">
          <div className="flex items-center gap-1.5 px-2 py-0.5 bg-[hsl(var(--accent-success)/0.15)] rounded text-[10px] text-[hsl(var(--text-secondary))]">
            <StatusIcon className={`w-3 h-3 flex-shrink-0 ${status.spinning ? 'animate-spin' : ''}`} />
            <span className="truncate whitespace-nowrap">{status.text}</span>
          </div>
        </div>

        {/* Right: Window controls */}
        <div className="flex items-center gap-0.5 app-region-no-drag">
          <IconButton
            icon={<Minus />}
            tooltip="Minimize"
            onClick={onMinimize}
            size="sm"
          />
          <IconButton
            icon={<X className="group-hover:text-[hsl(var(--accent-error))] transition-colors" />}
            tooltip="Close"
            onClick={onClose}
            size="sm"
          />
        </div>
      </div>

      {/* Bottom strip: Very thin resource bar */}
      <div className="h-3 px-3 pb-1.5 flex items-center gap-2 app-region-no-drag">
        {/* Left: Biceps (load indicator) + CPU icon */}
        <div className="flex items-center gap-1 flex-shrink-0">
          <Tooltip content={`${cpuUsage}%`}>
            <BicepsFlexed
              className="w-3.5 h-3.5"
              style={{ color: getLoadColor(cpuUsage) }}
            />
          </Tooltip>
          <Tooltip content={`RAM ${formatBytes(ramUsed * 1024 * 1024 * 1024)}`}>
            <Cpu className="w-3.5 h-3.5 text-[hsl(var(--launcher-accent-cpu))]" />
          </Tooltip>
        </div>

        {/* Center: RAM/VRAM bars - very thin strips */}
        <div className="flex-1 flex items-center gap-0.5 min-w-0">
          {/* RAM bar - extends from left toward center */}
          <div className="flex-1 h-1.5 bg-[hsl(var(--surface-overlay))] rounded-sm overflow-hidden">
            <div
              className="h-full bg-[hsl(var(--launcher-accent-ram))] transition-all duration-300"
              style={{ width: `${ramPercent}%` }}
            />
          </div>

          {/* VRAM bar - extends from right toward center */}
          <div className="flex-1 h-1.5 bg-[hsl(var(--surface-overlay))] rounded-sm overflow-hidden flex justify-end">
            <div
              className="h-full bg-[hsl(var(--launcher-accent-gpu))] transition-all duration-300"
              style={{ width: `${vramPercent}%` }}
            />
          </div>
        </div>

        {/* Right: GPU icon + Biceps (load indicator, flipped) */}
        <div className="flex items-center gap-1 flex-shrink-0">
          <Tooltip content={`VRAM ${formatBytes(vramUsed * 1024 * 1024 * 1024)}`}>
            <Gpu className="w-3.5 h-3.5 text-[hsl(var(--launcher-accent-gpu))]" />
          </Tooltip>
          <Tooltip content={`${gpuUsage}%`}>
            <BicepsFlexed
              className="w-3.5 h-3.5 scale-x-[-1]"
              style={{ color: getLoadColor(gpuUsage) }}
            />
          </Tooltip>
        </div>
      </div>
    </div>
  );
};
