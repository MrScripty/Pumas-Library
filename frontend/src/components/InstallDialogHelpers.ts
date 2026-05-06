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
  const prereleaseFiltered = availableVersions.filter((release) => {
    if (!showPreReleases && release.prerelease) {
      return false;
    }
    return true;
  });

  return filterLatestPatchVersions(prereleaseFiltered).filter((release) => {
    if (!showInstalled && installedVersions.includes(release.tagName)) {
      return false;
    }
    return true;
  });
}

interface ParsedVersionTag {
  major: number;
  minor: number;
  patch: number;
}

function parseVersionTag(tag: string): ParsedVersionTag | null {
  const match = tag.trim().match(/^v?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/i);
  if (!match) {
    return null;
  }

  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function compareParsedVersion(a: ParsedVersionTag, b: ParsedVersionTag): number {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}

function filterLatestPatchVersions(availableVersions: VersionRelease[]): VersionRelease[] {
  const latestByRelease = new Map<string, VersionRelease>();

  for (const release of availableVersions) {
    const parsed = parseVersionTag(release.tagName);
    if (!parsed) {
      continue;
    }

    const releaseKey = `${parsed.major}.${parsed.minor}`;
    const current = latestByRelease.get(releaseKey);
    if (!current) {
      latestByRelease.set(releaseKey, release);
      continue;
    }

    const currentParsed = parseVersionTag(current.tagName);
    if (!currentParsed || compareParsedVersion(parsed, currentParsed) > 0) {
      latestByRelease.set(releaseKey, release);
    }
  }

  return availableVersions.filter((release) => {
    const parsed = parseVersionTag(release.tagName);
    if (!parsed) {
      return true;
    }

    return latestByRelease.get(`${parsed.major}.${parsed.minor}`)?.tagName === release.tagName;
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
