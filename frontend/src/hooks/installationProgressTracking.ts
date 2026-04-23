import type { InstallationProgressResponse } from '../types/api';
import type {
  InstallationProgress,
  InstallNetworkStatus,
  VersionRelease,
} from '../types/versions';
import {
  computeAverageSpeed,
  computeNetworkStatus,
  resetNetworkStatusState,
  updateDownloadSamples,
  type NetworkStatusState,
} from '../utils/networkStatusMonitor';

export interface InstallationProgressTrackerState {
  lastDownloadTag: string | null;
  lastStage: InstallationProgress['stage'] | null;
  networkState: NetworkStatusState;
}

type InstallationProgressSource = InstallationProgressResponse | InstallationProgress;

export function resetInstallationProgressTracking(
  state: InstallationProgressTrackerState
): void {
  resetNetworkStatusState(state.networkState);
  state.lastDownloadTag = null;
  state.lastStage = null;
}

function computeExpectedTotal(
  progress: InstallationProgressSource,
  availableVersions: VersionRelease[]
): number | null {
  const release = availableVersions.find((candidate) => candidate.tagName === progress.tag);
  const archiveEstimate = release?.archiveSize ?? null;
  const dependencyEstimate =
    release?.totalSize && release.archiveSize
      ? Math.max(release.totalSize - release.archiveSize, 0)
      : null;

  if (progress.stage === 'download') {
    return progress.total_size ?? archiveEstimate ?? release?.totalSize ?? null;
  }
  if (progress.stage === 'dependencies') {
    return dependencyEstimate ?? release?.totalSize ?? null;
  }

  return null;
}

function computeEtaSeconds(
  progress: InstallationProgressSource,
  averageSpeed: number,
  expectedTotal: number | null
): number | null {
  const fallbackSpeed = progress.download_speed || 0;
  const etaSpeed = averageSpeed > 0 ? averageSpeed : fallbackSpeed;

  if (
    (progress.stage !== 'download' && progress.stage !== 'dependencies')
    || !expectedTotal
    || etaSpeed <= 0
  ) {
    return null;
  }

  const remaining = Math.max(expectedTotal - (progress.downloaded_bytes || 0), 0);
  return Math.ceil(remaining / etaSpeed);
}

function synchronizeTrackerState(
  progress: InstallationProgressSource,
  trackerState: InstallationProgressTrackerState,
  downloadedBytes: number,
  speed: number,
  now: number
): void {
  if (progress.tag !== trackerState.lastDownloadTag) {
    trackerState.lastDownloadTag = progress.tag || null;
    trackerState.lastStage = progress.stage || null;
    resetNetworkStatusState(trackerState.networkState);
    trackerState.networkState.lastDownload = { bytes: downloadedBytes, speed, ts: now };
    trackerState.networkState.topSpeed = speed || 0;
    return;
  }

  if (progress.stage !== trackerState.lastStage) {
    trackerState.networkState.downloadSamples = [];
    trackerState.lastStage = progress.stage || null;
  }
}

export function normalizeInstallationProgress(
  progress: InstallationProgressSource,
  availableVersions: VersionRelease[],
  trackerState: InstallationProgressTrackerState,
  now: number
): {
  adjustedProgress: InstallationProgress;
  networkStatus: InstallNetworkStatus;
} {
  const downloadedBytes = progress.downloaded_bytes || 0;
  const speed = progress.download_speed || 0;

  synchronizeTrackerState(progress, trackerState, downloadedBytes, speed, now);
  trackerState.networkState.downloadSamples = updateDownloadSamples(
    trackerState.networkState.downloadSamples,
    now,
    downloadedBytes
  );

  const averageSpeed = computeAverageSpeed(trackerState.networkState.downloadSamples);
  const expectedTotal = computeExpectedTotal(progress, availableVersions);
  const etaSeconds = computeEtaSeconds(progress, averageSpeed, expectedTotal);

  const adjustedProgress: InstallationProgress = {
    tag: progress.tag || '',
    started_at: progress.started_at || '',
    stage: progress.stage || 'download',
    stage_progress: progress.stage_progress || 0,
    overall_progress: progress.overall_progress || 0,
    current_item: progress.current_item || null,
    download_speed: progress.download_speed ?? (averageSpeed > 0 ? averageSpeed : null),
    eta_seconds: etaSeconds,
    total_size: expectedTotal ?? progress.total_size ?? null,
    downloaded_bytes: downloadedBytes,
    dependency_count: progress.dependency_count ?? null,
    completed_dependencies: progress.completed_dependencies ?? 0,
    completed_items: progress.completed_items ?? [],
    error: progress.error ?? null,
  };

  const networkStatus = computeNetworkStatus(
    adjustedProgress,
    trackerState.networkState,
    now
  );

  return {
    adjustedProgress,
    networkStatus,
  };
}
