import { Clock, Database, Download, Package, RefreshCw, WifiOff, type LucideIcon } from 'lucide-react';
import type { ActiveModelDownload } from '../hooks/useActiveModelDownload';
import { formatBytes, formatSpeed } from '../utils/formatters';

export interface InstallationProgress {
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

export interface HeaderStatusInfo {
  icon: LucideIcon;
  spinning: boolean;
  text: string;
}

function getInstallStageName(stage: InstallationProgress['stage']): string {
  const stageNames: Partial<Record<InstallationProgress['stage'], string>> = {
    extract: 'Extracting',
    setup: 'Finalizing setup',
    venv: 'Creating environment',
  };
  return stageNames[stage] ?? 'Installing';
}

function getDependencyPackageInfo(installationProgress: InstallationProgress): string {
  if (installationProgress.dependency_count !== null) {
    return `${installationProgress.completed_dependencies}/${installationProgress.dependency_count} packages`;
  }
  return 'Installing packages';
}

function getInstallationStatus(installationProgress: InstallationProgress): HeaderStatusInfo {
  if (installationProgress.stage === 'download' && installationProgress.download_speed !== null) {
    return {
      icon: Download,
      spinning: false,
      text: `Downloading at ${formatSpeed(installationProgress.download_speed)} · ${installationProgress.overall_progress}% complete`,
    };
  }

  if (installationProgress.stage === 'dependencies') {
    const speedInfo = installationProgress.download_speed !== null
      ? ` · ${formatSpeed(installationProgress.download_speed)}`
      : '';
    return {
      icon: Package,
      spinning: false,
      text: `Installing · ${getDependencyPackageInfo(installationProgress)}${speedInfo}`,
    };
  }

  return {
    icon: Download,
    spinning: false,
    text: `${getInstallStageName(installationProgress.stage)} · ${installationProgress.overall_progress}% complete`,
  };
}

function getActiveDownloadName(activeModelDownload: ActiveModelDownload): string {
  return activeModelDownload.repoId?.split('/').pop() || activeModelDownload.repoId || 'model';
}

function getActiveDownloadSpeed(activeModelDownload: ActiveModelDownload): string {
  if (activeModelDownload.speed && activeModelDownload.speed > 0) {
    return ` at ${formatSpeed(activeModelDownload.speed)}`;
  }
  return '';
}

function getActiveDownloadBytes(activeModelDownload: ActiveModelDownload, speedInfo: string): string {
  if (
    speedInfo ||
    activeModelDownload.downloadedBytes === null ||
    activeModelDownload.totalBytes === null
  ) {
    return '';
  }

  return ` · ${formatBytes(activeModelDownload.downloadedBytes)} / ${formatBytes(activeModelDownload.totalBytes)}`;
}

function getActiveDownloadStatus(
  activeModelDownload: ActiveModelDownload,
  activeModelDownloadCount: number
): HeaderStatusInfo {
  if (activeModelDownload.status === 'downloading' && activeModelDownloadCount > 0) {
    const modelLabel = activeModelDownloadCount === 1 ? 'model' : 'models';
    return {
      icon: Download,
      spinning: false,
      text: `Downloading ${activeModelDownloadCount} ${modelLabel}${getActiveDownloadSpeed(activeModelDownload)}`,
    };
  }

  const progress = Math.max(0, Math.min(100, Math.round(activeModelDownload.progress)));
  const modelName = getActiveDownloadName(activeModelDownload);

  if (activeModelDownload.status === 'downloading') {
    const speedInfo = getActiveDownloadSpeed(activeModelDownload);
    const bytesInfo = getActiveDownloadBytes(activeModelDownload, speedInfo);
    return {
      icon: Download,
      spinning: false,
      text: `Downloading ${modelName}${speedInfo} · ${progress}%${bytesInfo}`,
    };
  }

  if (activeModelDownload.status === 'queued') {
    return {
      icon: Download,
      spinning: false,
      text: `Queued model download · ${modelName}`,
    };
  }

  if (activeModelDownload.status === 'pausing') {
    return {
      icon: Download,
      spinning: false,
      text: `Pausing model download · ${modelName}`,
    };
  }

  return {
    icon: Download,
    spinning: false,
    text: `Cancelling model download · ${modelName}`,
  };
}

export function getHeaderStatusInfo({
  activeModelDownload,
  activeModelDownloadCount,
  installationProgress,
  modelLibraryLoaded,
  networkAvailable,
}: {
  activeModelDownload?: ActiveModelDownload | null;
  activeModelDownloadCount: number;
  installationProgress?: InstallationProgress | null;
  modelLibraryLoaded?: boolean | null;
  networkAvailable?: boolean | null;
}): HeaderStatusInfo {
  if (installationProgress && !installationProgress.completed_at) {
    return getInstallationStatus(installationProgress);
  }

  if (activeModelDownload) {
    return getActiveDownloadStatus(activeModelDownload, activeModelDownloadCount);
  }

  if (
    networkAvailable === null ||
    networkAvailable === undefined ||
    modelLibraryLoaded === null ||
    modelLibraryLoaded === undefined
  ) {
    return {
      icon: RefreshCw,
      spinning: true,
      text: 'Checking network and model library...',
    };
  }

  if (!networkAvailable) {
    return {
      icon: WifiOff,
      spinning: false,
      text: 'Network unavailable',
    };
  }

  if (!modelLibraryLoaded) {
    return {
      icon: Clock,
      spinning: false,
      text: 'Model library database unavailable',
    };
  }

  return {
    icon: Database,
    spinning: false,
    text: 'Network online · model library ready',
  };
}
