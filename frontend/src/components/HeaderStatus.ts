import { Clock, Database, Download, RefreshCw, WifiOff, type LucideIcon } from 'lucide-react';
import type { ActiveModelDownload } from '../hooks/useActiveModelDownload';
import { formatBytes, formatSpeed } from '../utils/formatters';
import { getInstallActivityPresentation } from '../utils/installActivityPresentation';
import { getInstallationDownloadPresentation } from '../utils/installationDownloadPresentation';

export interface InstallationProgress {
  tag: string;
  started_at: string;
  stage: 'resolving' | 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress: number;
  overall_progress: number;
  current_item: string | null;
  download_active?: boolean;
  download_measurement_available?: boolean | null;
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

function pluralize(count: number, singular: string): string {
  return `${count} ${count === 1 ? singular : `${singular}s`}`;
}

function joinDownloadParts(parts: string[]): string {
  if (parts.length <= 1) {
    return parts[0] ?? '';
  }

  return `${parts.slice(0, -1).join(', ')} & ${parts[parts.length - 1]}`;
}

function getCombinedDownloadStatus({
  appId,
  activeModelDownload,
  activeModelDownloadCount,
  installationProgress,
  runtimePending = false,
}: {
  appId?: string | null;
  activeModelDownload?: ActiveModelDownload | null;
  activeModelDownloadCount: number;
  installationProgress?: InstallationProgress | null;
  runtimePending?: boolean;
}): HeaderStatusInfo | null {
  const runtimeActive = runtimePending || Boolean(installationProgress && !installationProgress.completed_at);
  const modelCount = activeModelDownload ? Math.max(activeModelDownloadCount, 1) : 0;
  const runtimeCount = runtimeActive ? 1 : 0;

  if (!runtimeActive) {
    return null;
  }

  const parts = [
    ...(modelCount > 0 ? [pluralize(modelCount, 'model')] : []),
    ...(runtimeCount > 0 ? [pluralize(runtimeCount, 'runtime')] : []),
  ];
  const modelSpeed = activeModelDownload?.speed && activeModelDownload.speed > 0
    ? activeModelDownload.speed
    : 0;
  const runtimeSpeed = installationProgress?.download_speed && installationProgress.download_speed > 0
    ? installationProgress.download_speed
    : 0;
  const totalSpeed = modelSpeed + runtimeSpeed;
  const downloadPresentation = getInstallationDownloadPresentation(appId, installationProgress);
  const runtimeDownloadExpected = installationProgress?.download_active === true ||
    installationProgress?.stage === 'download';
  const speedInfo = totalSpeed > 0 ? ` @ ${formatSpeed(totalSpeed)}` : '';
  const hasDownloadActivity =
    Boolean(activeModelDownload?.status === 'downloading') ||
    installationProgress?.stage === 'download' ||
    runtimeDownloadExpected ||
    totalSpeed > 0 ||
    downloadPresentation?.kind === 'measuring';
  const visibleSpeedInfo = totalSpeed > 0
    ? speedInfo
    : downloadPresentation ? ` · ${downloadPresentation.label.toLowerCase()}` : '';
  const activityVerb = hasDownloadActivity
    ? 'Downloading'
    : installationProgress?.stage === 'resolving' ? 'Preparing' : 'Installing';

  return {
    icon: Download,
    spinning: installationProgress?.stage === 'resolving',
    text: `${activityVerb} ${joinDownloadParts(parts)}${visibleSpeedInfo}`,
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

  const progress = Math.max(0, Math.min(100, Math.round(activeModelDownload.progress * 100)));
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
  appId,
  activeModelDownload,
  activeModelDownloadCount,
  installationProgress,
  installingTag,
  modelLibraryLoaded,
  networkAvailable,
}: {
  appId?: string | null;
  activeModelDownload?: ActiveModelDownload | null;
  activeModelDownloadCount: number;
  installationProgress?: InstallationProgress | null;
  installingTag?: string | null;
  modelLibraryLoaded?: boolean | null;
  networkAvailable?: boolean | null;
}): HeaderStatusInfo {
  const activity = getInstallActivityPresentation({ appId, installingTag, progress: installationProgress });
  const combinedDownloadStatus = getCombinedDownloadStatus({
    appId,
    activeModelDownload,
    activeModelDownloadCount,
    installationProgress,
    runtimePending: activity.active,
  });
  if (activity.active && activeModelDownload && activeModelDownload.status !== 'downloading') {
    const modelStatus = getActiveDownloadStatus(activeModelDownload, activeModelDownloadCount);
    const downloadPresentation = appId === 'torch'
      ? getInstallationDownloadPresentation(appId, installationProgress)
      : null;
    const downloadStatus = downloadPresentation
      ? ` · Torch download: ${downloadPresentation.label}`
      : '';
    return {
      ...modelStatus,
      text: `${modelStatus.text}${downloadStatus} · ${appId === 'torch' ? 'Torch ' : ''}runtime: ${activity.phase}`,
    };
  }
  const runtimeDownloadActive = installationProgress?.download_active === true ||
    installationProgress?.stage === 'download' ||
    (installationProgress?.download_speed ?? 0) > 0 ||
    getInstallationDownloadPresentation(appId, installationProgress) !== null;
  const modelDownloadActive = activeModelDownload?.status === 'downloading';
  if (
    appId === 'torch' && activity.active && installationProgress && combinedDownloadStatus &&
    (runtimeDownloadActive || modelDownloadActive)
  ) {
    return {
      ...combinedDownloadStatus,
      text: `${combinedDownloadStatus.text} · Torch: ${activity.phase}`,
    };
  }
  if (activity.active && !installationProgress && activeModelDownload && combinedDownloadStatus) {
    return {
      ...combinedDownloadStatus,
      text: `${combinedDownloadStatus.text} · ${activity.phase}`,
    };
  }
  if (activity.active && (appId === 'torch' || !installationProgress)) {
    return {
      icon: activity.indeterminate ? RefreshCw : Download,
      spinning: activity.indeterminate,
      text: appId === 'torch'
        ? `Installing Torch ${installingTag ?? installationProgress?.tag} · ${activity.phase}`
        : activity.phase,
    };
  }
  if (combinedDownloadStatus) {
    return combinedDownloadStatus;
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
