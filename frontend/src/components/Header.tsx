import React from 'react';
import { X, ArrowUp, HardDrive, Cpu, Gpu, MemoryStick, RefreshCw, WifiOff, Clock, Database, Download, Package } from 'lucide-react';
import type { SystemResources } from '../types/apps';
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

interface HeaderProps {
  systemResources?: SystemResources;
  diskSpacePercent: number;
  launcherVersion: string | null;
  launcherUpdateAvailable: boolean;
  isUpdatingLauncher: boolean;
  onUpdate: () => void;
  onClose: () => void;
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

// Helper to get color based on load percentage
const getLoadColor = (load: number): string => {
  if (load < 30) return 'hsl(var(--accent-info))'; // Blue for low load
  if (load < 70) return 'hsl(var(--accent-warning))'; // Purple/Yellow for medium load
  return 'hsl(var(--accent-error))'; // Red for high load
};

// Helper to get disk color based on usage percentage
const getDiskColor = (percent: number): string => {
  if (percent < 70) return 'hsl(var(--text-secondary))'; // Grey for good
  if (percent < 85) return 'hsl(var(--accent-warning))'; // Yellow/Orange for warning
  if (percent < 95) return 'hsl(var(--launcher-accent-warning))'; // Orange for high
  return 'hsl(var(--accent-error))'; // Red for critical
};

// Helper to get RAM icon color based on usage
const getRamIconColor = (ramPercent: number, vramPercent: number): string => {
  const maxUsage = Math.max(ramPercent, vramPercent);
  if (maxUsage < 70) return 'hsl(var(--text-secondary))'; // Grey for normal
  if (maxUsage < 85) return 'hsl(var(--accent-warning))'; // Yellow/Orange for warning
  if (maxUsage < 95) return 'hsl(var(--launcher-accent-warning))'; // Orange for high
  return 'hsl(var(--accent-error))'; // Red for critical
};

// Biceps icon component for load indicator
const BicepsFlexed: React.FC<{ className?: string; style?: React.CSSProperties }> = ({ className, style }) => (
  <svg
    xmlns="http://www.w3.org/2000/svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    style={style}
  >
    <path d="M12.5 3.5c-1.5 0-2.5 1-2.5 2.5v3c0 .5-.2 1-.5 1.5L8 12.5c-.5.8-.5 1.7 0 2.5l1.5 2c.3.5.5 1 .5 1.5v1c0 1.5 1 2.5 2.5 2.5s2.5-1 2.5-2.5v-1c0-.5.2-1 .5-1.5l1.5-2c.5-.8.5-1.7 0-2.5L16 10.5c-.3-.5-.5-1-.5-1.5v-3c0-1.5-1-2.5-2.5-2.5" />
    <path d="M14 11s.5-1 2-1 2.5.5 3 2-1 3-3 3" />
    <path d="M10 11s-.5-1-2-1-2.5.5-3 2 1 3 3 3" />
  </svg>
);

export const Header: React.FC<HeaderProps> = ({
  systemResources,
  diskSpacePercent,
  launcherVersion,
  launcherUpdateAvailable,
  isUpdatingLauncher,
  onUpdate,
  onClose,
  cacheStatus,
  installationProgress,
}) => {
  const cpuUsage = Math.round(systemResources?.cpu?.usage ?? 0);
  const gpuUsage = Math.round(systemResources?.gpu?.usage ?? 0);
  const ramPercent = Math.round(systemResources?.ram?.usage ?? 0);
  const vramTotal = systemResources?.gpu?.memory_total ?? 1;
  const vramUsed = systemResources?.gpu?.memory ?? 0;
  const vramPercent = Math.round((vramUsed / vramTotal) * 100);

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

    // FETCHING STATE
    if (cacheStatus.is_fetching) {
      return {
        icon: RefreshCw,
        text: 'Fetching releases...',
        spinning: true
      };
    }

    // NO CACHE STATE
    if (!cacheStatus.has_cache) {
      return {
        icon: WifiOff,
        text: 'No cache available - offline mode',
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
      spinning: false
    };
  };

  const status = getStatusInfo();
  const StatusIcon = status.icon;

  return (
    <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary)/0.3)] backdrop-blur-sm relative z-10">
      {/* Top row: App name, update icon, status text, close button */}
      <div className="px-4 py-2 flex items-center justify-between gap-4">
        {/* Left: App name */}
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-[hsl(var(--text-primary))]">AI Manager</span>
        </div>

        {/* Center: Update icon + Status text (flex-1 to take remaining space) */}
        <div className="flex-1 flex items-center gap-3 min-w-0">
          {/* Update icon with tooltip */}
          <div className="flex-shrink-0 group relative">
            <button
              onClick={launcherUpdateAvailable ? onUpdate : undefined}
              disabled={isUpdatingLauncher}
              className={`p-1.5 rounded transition-colors ${
                launcherUpdateAvailable
                  ? 'bg-[hsl(var(--accent-warning)/0.2)] hover:bg-[hsl(var(--accent-warning)/0.3)] cursor-pointer'
                  : 'cursor-default'
              }`}
              title={launcherVersion || 'dev'}
            >
              <ArrowUp
                className={`w-3.5 h-3.5 ${
                  launcherUpdateAvailable
                    ? 'text-[hsl(var(--accent-warning))]'
                    : 'text-[hsl(var(--text-muted))]'
                }`}
              />
            </button>
            {/* Tooltip showing version/git ID */}
            <div className="absolute left-0 top-full mt-1 px-2 py-1 bg-[hsl(var(--surface-overlay))] border border-[hsl(var(--border-default))] rounded text-xs text-[hsl(var(--text-secondary))] whitespace-nowrap opacity-0 group-hover:opacity-100 pointer-events-none transition-opacity z-50">
              {launcherVersion || 'dev'}
            </div>
          </div>

          {/* Status text display */}
          <div className="flex-1 flex items-center gap-2 min-w-0 px-2">
            <StatusIcon className={`w-3.5 h-3.5 flex-shrink-0 text-[hsl(var(--text-secondary))] ${status.spinning ? 'animate-spin' : ''}`} />
            <span className="text-xs text-[hsl(var(--text-secondary))] truncate">
              {status.text}
            </span>
          </div>
        </div>

        {/* Right: Close button */}
        <button
          onClick={onClose}
          className="flex-shrink-0 p-1.5 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors group"
        >
          <X className="w-5 h-5 text-[hsl(var(--text-secondary))] group-hover:text-[hsl(var(--accent-error))] transition-colors" />
        </button>
      </div>

      {/* Bottom row: Resource monitors */}
      <div className="px-4 pb-3 pt-1 flex items-end justify-between gap-4">
        {/* Left side: Disk + CPU */}
        <div className="flex flex-col gap-2">
          {/* Disk icon - color coded */}
          <div className="flex items-center gap-1.5">
            <HardDrive
              className="w-4 h-4"
              style={{ color: getDiskColor(diskSpacePercent) }}
            />
          </div>

          {/* CPU + Biceps */}
          <div className="flex items-center gap-1.5">
            <BicepsFlexed
              className="w-4 h-4"
              style={{ color: getLoadColor(cpuUsage) }}
            />
            <Cpu className="w-4 h-4 text-[hsl(var(--launcher-accent-cpu))]" />
            <span className="text-xs font-mono text-[hsl(var(--text-primary))]">
              {cpuUsage}%
            </span>
          </div>
        </div>

        {/* Center: RAM/VRAM bars and RAM icon */}
        <div className="flex-1 flex flex-col gap-1 min-w-0">
          {/* RAM icon - color coded */}
          <div className="flex justify-center">
            <MemoryStick
              className="w-4 h-4"
              style={{ color: getRamIconColor(ramPercent, vramPercent) }}
            />
          </div>

          {/* Progress bars container */}
          <div className="relative h-4 flex items-center gap-0.5">
            {/* RAM bar - extends from left (CPU side) toward center */}
            <div className="flex-1 h-2 bg-[hsl(var(--surface-overlay))] rounded-l overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-[hsl(var(--accent-info))] to-purple-500 transition-all duration-300"
                style={{ width: `${ramPercent}%` }}
              />
            </div>

            {/* VRAM bar - extends from right (GPU side) toward center */}
            <div className="flex-1 h-2 bg-[hsl(var(--surface-overlay))] rounded-r overflow-hidden flex justify-end">
              <div
                className="h-full bg-gradient-to-l from-[hsl(var(--launcher-accent-gpu))] to-blue-400 transition-all duration-300"
                style={{ width: `${vramPercent}%` }}
              />
            </div>
          </div>

          {/* Labels */}
          <div className="flex justify-between text-[10px] font-mono text-[hsl(var(--text-tertiary))]">
            <span>RAM {ramPercent}%</span>
            <span>VRAM {vramPercent}%</span>
          </div>
        </div>

        {/* Right side: GPU + Biceps (flipped) */}
        <div className="flex flex-col gap-2 items-end">
          {/* Empty space to align with Disk icon */}
          <div className="h-6" />

          {/* GPU + Biceps (horizontally flipped) */}
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-mono text-[hsl(var(--text-primary))]">
              {gpuUsage}%
            </span>
            <Gpu className="w-4 h-4 text-[hsl(var(--launcher-accent-gpu))]" />
            <BicepsFlexed
              className="w-4 h-4 scale-x-[-1]"
              style={{ color: getLoadColor(gpuUsage) }}
            />
          </div>
        </div>
      </div>
    </div>
  );
};
