import { APIError } from '../errors';
import type { InstallationProgress } from '../hooks/useVersions';
import { getLogger } from '../utils/logger';

const logger = getLogger('VersionSelector');

interface VersionSelectorStateInput {
  activeVersion: string | null;
  diskSpacePercent: number;
  hasNewVersion: boolean;
  installedVersions: string[];
  installNetworkStatus: 'idle' | 'downloading' | 'stalled' | 'failed';
  installationProgress?: InstallationProgress | null;
  installingVersion?: string | null;
  isLoading: boolean;
  latestVersion: string | null;
}

export interface VersionSelectorDisplayState {
  combinedVersions: string[];
  displayVersion: string;
  emphasizeInstall: boolean;
  folderIconColor: string;
  hasInstallActivity: boolean;
  hasInstalledVersions: boolean;
  hasVersionsToShow: boolean;
  isInstallComplete: boolean;
  isInstallFailed: boolean;
  isInstallPending: boolean;
  ringDegrees: number;
}

function combineInstalledVersions(
  installedVersions: string[],
  installingVersion?: string | null
): string[] {
  const unique = new Set(installedVersions);
  const merged = [...installedVersions];
  if (installingVersion && !unique.has(installingVersion)) {
    merged.push(installingVersion);
  }
  return merged.sort((a, b) => b.localeCompare(a, undefined, { numeric: true, sensitivity: 'base' }));
}

function getFolderIconColor(diskSpacePercent: number): string {
  if (diskSpacePercent >= 95) return 'text-accent-error';
  if (diskSpacePercent >= 85) return 'text-accent-warning';
  return 'text-tertiary';
}

function isPendingInstallProgress(progress?: InstallationProgress | null): boolean {
  return !progress
    || (
      progress.stage === 'download'
      && progress.downloaded_bytes <= 0
      && (progress.download_speed ?? 0) <= 0
      && !progress.error
    );
}

function getProgressDegrees(progressPercent: number): number {
  return Math.min(
    360,
    Math.max(0, Math.round((Math.min(100, Math.max(0, progressPercent)) / 100) * 360))
  );
}

export function getVersionSelectorDisplayState({
  activeVersion,
  diskSpacePercent,
  installedVersions,
  installNetworkStatus,
  installationProgress,
  installingVersion,
  isLoading,
}: VersionSelectorStateInput): VersionSelectorDisplayState {
  const combinedVersions = combineInstalledVersions(installedVersions, installingVersion);
  const hasInstalledVersions = installedVersions.length > 0;
  const isInstallComplete = Boolean(installationProgress?.completed_at);
  const hasInstallActivity = Boolean(installingVersion) && !isInstallComplete;
  const isInstallFailed = installNetworkStatus === 'failed';
  const isInstallPending =
    hasInstallActivity && !isInstallFailed && isPendingInstallProgress(installationProgress);
  const progressPercent = installationProgress?.overall_progress ?? 0;

  return {
    combinedVersions,
    displayVersion: activeVersion || 'No version selected',
    emphasizeInstall: !hasInstalledVersions && !isLoading,
    folderIconColor: getFolderIconColor(diskSpacePercent),
    hasInstallActivity,
    hasInstalledVersions,
    hasVersionsToShow: combinedVersions.length > 0,
    isInstallComplete,
    isInstallFailed,
    isInstallPending,
    ringDegrees: isInstallPending ? 60 : getProgressDegrees(progressPercent),
  };
}

export function reportVersionSwitchError(error: unknown, tag: string): void {
  if (error instanceof APIError) {
    logger.error('API error switching version', { error: error.message, endpoint: error.endpoint, tag });
  } else if (error instanceof Error) {
    logger.error('Failed to switch version', { error: error.message, tag });
  } else {
    logger.error('Unknown error switching version', { error, tag });
  }
}

export function reportOpenActiveInstallError(error: unknown, version: string): void {
  if (error instanceof APIError) {
    logger.error('API error opening installation path', { error: error.message, endpoint: error.endpoint, version });
  } else if (error instanceof Error) {
    logger.error('Failed to open installation path', { error: error.message, version });
  } else {
    logger.error('Unknown error opening installation path', { error, version });
  }
}

export function reportToggleDefaultError(error: unknown, version: string): void {
  if (error instanceof APIError) {
    logger.error('API error toggling default version', {
      error: error.message,
      endpoint: error.endpoint,
      version,
    });
  } else if (error instanceof Error) {
    logger.error('Failed to toggle default version', {
      error: error.message,
      version,
    });
  } else {
    logger.error('Unknown error toggling default version', {
      error: String(error),
      version,
    });
  }
}
