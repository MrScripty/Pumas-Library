import type { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { getLogger } from '../utils/logger';
import { APIError, NetworkError } from '../errors';

const logger = getLogger('InstallDialog');

interface FailedInstallState {
  tag: string | null;
  log: string | null;
}

export function filterVersions(
  availableVersions: VersionRelease[],
  installedVersions: string[],
  showPreReleases: boolean,
  showInstalled: boolean
): VersionRelease[] {
  return availableVersions.filter((release) => {
    if (!showPreReleases && release.prerelease) {
      return false;
    }
    if (!showInstalled && installedVersions.includes(release.tagName)) {
      return false;
    }
    return true;
  });
}

export function getStickyFailure(
  progress: InstallationProgress | null,
  failedInstall: FailedInstallState | null
): FailedInstallState {
  const progressFailed = progress && progress.completed_at && !progress.success;
  return {
    tag: progressFailed ? progress.tag : failedInstall?.tag ?? null,
    log: progressFailed ? progress.log_path || null : failedInstall?.log ?? null,
  };
}

export function isInstallationCancellation(
  error: unknown,
  cancellationRequested: boolean
): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return cancellationRequested || message.toLowerCase().includes('cancel');
}

export function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function reportInstallationError(tag: string, error: unknown): void {
  if (error instanceof APIError) {
    logger.error('API error during installation', { error: error.message, endpoint: error.endpoint, tag });
  } else if (error instanceof NetworkError) {
    logger.error('Network error during installation', { error: error.message, url: error.url ?? undefined, status: error.status ?? undefined, tag });
  } else if (error instanceof Error) {
    logger.error('Installation failed', { error: error.message, tag });
  } else {
    logger.error('Unknown error during installation', { error, tag });
  }
}

export function reportCancelError(error: unknown): void {
  if (error instanceof APIError) {
    logger.error('API error cancelling installation', { error: error.message, endpoint: error.endpoint });
  } else if (error instanceof Error) {
    logger.error('Error cancelling installation', { error: error.message });
  } else {
    logger.error('Unknown error cancelling installation', { error });
  }
}
