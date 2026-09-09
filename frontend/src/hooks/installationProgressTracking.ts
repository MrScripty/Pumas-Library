import type { RuntimeInstallationProgress } from '../generated/desktop-contract';
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

export function resetInstallationProgressTracking(
  state: InstallationProgressTrackerState
): void {
  resetNetworkStatusState(state.networkState);
  state.lastDownloadTag = null;
  state.lastStage = null;
}

export function projectInstallationProgress(
  progress: RuntimeInstallationProgress
): InstallationProgress {
  return {
    tag: progress.tag,
    started_at: progress.startedAt,
    stage: progress.stage,
    stage_progress: progress.stageProgress ?? 0,
    overall_progress: progress.overallProgress ?? 0,
    current_item: progress.currentItem,
    download_speed: progress.downloadSpeed,
    eta_seconds: progress.etaSeconds,
    total_size: progress.totalSize,
    downloaded_bytes: progress.downloadedBytes,
    dependency_count: progress.dependencyCount,
    completed_dependencies: progress.completedDependencies,
    completed_items: progress.completedItems.map((item) => ({
      name: item.name,
      type: item.type,
      size: item.size,
      completed_at: item.completedAt,
    })),
    error: progress.error,
    completed_at: progress.completedAt ?? undefined,
    success: progress.success ?? undefined,
    log_path: progress.logPath,
  };
}

function computeExpectedTotal(
  progress: InstallationProgress,
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
  progress: InstallationProgress,
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
  progress: InstallationProgress,
  trackerState: InstallationProgressTrackerState,
  downloadedBytes: number,
  speed: number,
  now: number
): void {
  if (progress.tag !== trackerState.lastDownloadTag) {
    trackerState.lastDownloadTag = progress.tag || null;
    trackerState.lastStage = progress.stage;
    resetNetworkStatusState(trackerState.networkState);
    trackerState.networkState.lastDownload = { bytes: downloadedBytes, speed, ts: now };
    trackerState.networkState.topSpeed = speed || 0;
    return;
  }

  if (progress.stage !== trackerState.lastStage) {
    trackerState.networkState.downloadSamples = [];
    trackerState.lastStage = progress.stage;
  }
}

export function normalizeInstallationProgress(
  progress: InstallationProgress,
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
    stage: progress.stage,
    stage_progress: progress.stage_progress || 0,
    overall_progress: progress.overall_progress || 0,
    current_item: progress.current_item || null,
    download_speed: progress.download_speed ?? (averageSpeed > 0 ? averageSpeed : null),
    eta_seconds: etaSeconds,
    total_size: expectedTotal ?? progress.total_size ?? null,
    downloaded_bytes: downloadedBytes,
    dependency_count: progress.dependency_count ?? null,
    completed_dependencies: progress.completed_dependencies,
    completed_items: progress.completed_items,
    error: progress.error ?? null,
    completed_at: progress.completed_at,
    success: progress.success,
    log_path: progress.log_path ?? null,
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
