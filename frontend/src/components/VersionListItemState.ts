import type { CSSProperties } from 'react';
import type { InstallationProgress, InstallNetworkStatus, VersionRelease } from '../hooks/useVersions';

export interface VersionInstallDisplayState {
  displayTag: string;
  downloadIconClass: string;
  downloadIconStyle: CSSProperties;
  isComplete: boolean;
  isDownloadPending: boolean;
  isInstallFailed: boolean;
  packageLabel: string;
  ringColor: string;
  ringPercent: number | null;
  showUninstall: boolean;
  totalBytes: number | null;
}

function getDownloadPercent(progress: InstallationProgress | null): number | null {
  if (progress && progress.total_size && progress.total_size > 0) {
    return Math.min(100, Math.round((progress.downloaded_bytes / progress.total_size) * 100));
  }
  return null;
}

function getRingPercent(progress: InstallationProgress | null): number | null {
  const overallPercent = progress ? Math.round(progress.overall_progress || 0) : null;
  const downloadPercent = getDownloadPercent(progress);
  const stagePercent = progress ? progress.stage_progress : null;

  if (progress && (progress.stage === 'download' || progress.stage === 'dependencies')) {
    return downloadPercent ?? stagePercent ?? overallPercent;
  }
  return overallPercent ?? stagePercent;
}

function getPackageLabel(progress: InstallationProgress | null): string {
  if (progress?.stage === 'download') {
    const downloadPercent = getDownloadPercent(progress);
    if (downloadPercent !== null && (progress.downloaded_bytes > 0 || downloadPercent > 0)) {
      return `${downloadPercent}%`;
    }
    if (progress.stage_progress > 0) {
      return `${Math.round(progress.stage_progress)}%`;
    }
  }
  if (progress && progress.dependency_count !== null) {
    return `${progress.completed_dependencies}/${progress.dependency_count}`;
  }
  if (progress?.stage === 'dependencies') {
    return 'Installing...';
  }
  return 'Downloading...';
}

function isPendingDownload(
  isInstalling: boolean,
  isInstallFailed: boolean,
  progress: InstallationProgress | null
): boolean {
  if (!isInstalling || isInstallFailed) {
    return false;
  }
  if (!progress) {
    return true;
  }
  return (
    progress.stage === 'download' &&
    progress.downloaded_bytes <= 0 &&
    (progress.download_speed ?? 0) <= 0
  );
}

function getDownloadIconClass(installNetworkStatus: InstallNetworkStatus): string {
  switch (installNetworkStatus) {
    case 'stalled':
      return 'animate-pulse text-[hsl(var(--accent-warning))]';
    case 'failed':
      return 'animate-pulse text-[hsl(var(--accent-error))]';
    case 'idle':
    case 'downloading':
      return 'animate-pulse text-[hsl(var(--accent-success))]';
  }
}

function getDownloadIconStyle(installNetworkStatus: InstallNetworkStatus): CSSProperties {
  switch (installNetworkStatus) {
    case 'stalled':
      return { filter: 'drop-shadow(0 0 6px hsl(var(--accent-warning)))' };
    case 'failed':
      return { filter: 'drop-shadow(0 0 6px hsl(var(--accent-error)))' };
    case 'idle':
    case 'downloading':
      return { filter: 'drop-shadow(0 0 6px hsl(var(--accent-success)))' };
  }
}

function getDisplayTag(release: VersionRelease): string {
  const rawTag = release.tagName || release.name || 'Unknown version';
  if (release.tagName.includes('+') && release.name) {
    return release.name.replace(/^v/i, '') || release.name;
  }
  return rawTag.replace(/^v/i, '') || rawTag;
}

export function getVersionInstallDisplayState({
  installNetworkStatus,
  isHovered,
  isInstalled,
  isInstalling,
  progress,
  release,
}: {
  installNetworkStatus: InstallNetworkStatus;
  isHovered: boolean;
  isInstalled: boolean;
  isInstalling: boolean;
  progress: InstallationProgress | null;
  release: VersionRelease;
}): VersionInstallDisplayState {
  const isInstallFailed = installNetworkStatus === 'failed' || Boolean(progress?.error);

  return {
    displayTag: getDisplayTag(release),
    downloadIconClass: getDownloadIconClass(installNetworkStatus),
    downloadIconStyle: getDownloadIconStyle(installNetworkStatus),
    isComplete: isInstalled || (isInstalling && Boolean(progress?.success) && Boolean(progress?.completed_at)),
    isDownloadPending: isPendingDownload(isInstalling, isInstallFailed, progress),
    isInstallFailed,
    packageLabel: getPackageLabel(progress),
    ringColor: isInstallFailed ? 'hsl(var(--accent-error))' : 'hsl(var(--accent-success))',
    ringPercent: getRingPercent(progress),
    showUninstall: isInstalled && !isInstalling && isHovered,
    totalBytes: (progress ? progress.total_size : null) ?? release.totalSize ?? release.archiveSize ?? null,
  };
}
