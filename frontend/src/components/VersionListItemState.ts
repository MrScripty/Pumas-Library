import type { CSSProperties } from 'react';
import type { InstallationProgress, InstallNetworkStatus, VersionRelease } from '../hooks/useVersions';
import { getInstallActivityPresentation, type InstallActivityPresentation } from '../utils/installActivityPresentation';

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

function getRingPercent(progress: InstallationProgress | null, indeterminate: boolean): number | null {
  if (indeterminate) return null;
  const overallPercent = progress ? Math.round(progress.overall_progress || 0) : null;
  const downloadPercent = getDownloadPercent(progress);
  const stagePercent = progress ? progress.stage_progress : null;

  if (progress && (progress.stage === 'download' || progress.stage === 'dependencies')) {
    return downloadPercent ?? stagePercent ?? overallPercent;
  }
  return overallPercent ?? stagePercent;
}

function getPackageLabel(progress: InstallationProgress | null, activity: InstallActivityPresentation): string {
  if (!progress || activity.indeterminate && progress.stage === 'setup') return activity.phase;
  if (progress.stage === 'download') {
    const downloadPercent = getDownloadPercent(progress);
    if (downloadPercent !== null && (progress.downloaded_bytes > 0 || downloadPercent > 0)) {
      return `${downloadPercent}%`;
    }
    if (progress.stage_progress > 0) {
      return `${Math.round(progress.stage_progress)}%`;
    }
  }
  if (progress.stage === 'dependencies' && progress.dependency_count !== null) {
    return `${progress.completed_dependencies}/${progress.dependency_count}`;
  }
  if (progress.stage === 'dependencies') {
    return 'Installing...';
  }
  return activity.phase;
}

function isPendingDownload(
  isInstalling: boolean,
  isInstallFailed: boolean,
  progress: InstallationProgress | null,
  indeterminate: boolean
): boolean {
  if (!isInstalling || isInstallFailed) {
    return false;
  }
  if (!progress || indeterminate) {
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
  appId,
  installNetworkStatus,
  isHovered,
  isInstalled,
  isInstalling,
  progress,
  release,
}: {
  appId?: string;
  installNetworkStatus: InstallNetworkStatus;
  isHovered: boolean;
  isInstalled: boolean;
  isInstalling: boolean;
  progress: InstallationProgress | null;
  release: VersionRelease;
}): VersionInstallDisplayState {
  const isInstallFailed = installNetworkStatus === 'failed' || Boolean(progress?.error);
  const activity = getInstallActivityPresentation({
    appId,
    installingTag: isInstalling ? release.tagName : null,
    progress,
  });

  return {
    displayTag: getDisplayTag(release),
    downloadIconClass: getDownloadIconClass(installNetworkStatus),
    downloadIconStyle: getDownloadIconStyle(installNetworkStatus),
    isComplete: isInstalled || (isInstalling && Boolean(progress?.success) && Boolean(progress?.completed_at)),
    isDownloadPending: isPendingDownload(isInstalling, isInstallFailed, progress, activity.indeterminate),
    isInstallFailed,
    packageLabel: getPackageLabel(progress, activity),
    ringColor: isInstallFailed ? 'hsl(var(--accent-error))' : 'hsl(var(--accent-success))',
    ringPercent: getRingPercent(progress, activity.indeterminate),
    showUninstall: isInstalled && !isInstalling && isHovered,
    totalBytes: (progress ? progress.total_size : null) ?? release.totalSize ?? release.archiveSize ?? null,
  };
}
