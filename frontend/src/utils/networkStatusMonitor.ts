/**
 * Network Status Monitor
 *
 * Utilities for monitoring download/installation network status.
 * Detects stalls, failures, and computes average speeds.
 * Extracted from hooks/useVersions.ts
 */

import type { InstallationProgress, InstallNetworkStatus } from '../types/versions';

export interface NetworkStatusState {
  lastDownload: { bytes: number; speed: number; ts: number };
  topSpeed: number;
  lowSince: number | null;
  downloadSamples: { ts: number; bytes: number }[];
}

export function createNetworkStatusState(): NetworkStatusState {
  return {
    lastDownload: { bytes: 0, speed: 0, ts: 0 },
    topSpeed: 0,
    lowSince: null,
    downloadSamples: [],
  };
}

export function resetNetworkStatusState(state: NetworkStatusState): void {
  state.lastDownload = { bytes: 0, speed: 0, ts: 0 };
  state.topSpeed = 0;
  state.lowSince = null;
  state.downloadSamples = [];
}

/**
 * Compute average download speed from samples
 */
export function computeAverageSpeed(samples: { ts: number; bytes: number }[]): number {
  if (samples.length < 2) return 0;

  const sampleStart = samples[0];
  const sampleEnd = samples[samples.length - 1];

  if (!sampleStart || !sampleEnd || sampleEnd.ts <= sampleStart.ts) {
    return 0;
  }

  const deltaBytes = sampleEnd.bytes - sampleStart.bytes;
  const deltaTime = (sampleEnd.ts - sampleStart.ts) / 1000;

  if (deltaTime > 0 && deltaBytes >= 0) {
    return deltaBytes / deltaTime;
  }

  return 0;
}

/**
 * Determine network status based on download progress
 */
export function computeNetworkStatus(
  progress: InstallationProgress,
  state: NetworkStatusState,
  now: number
): InstallNetworkStatus {
  if (progress.error) {
    return 'failed';
  }

  if (progress.stage !== 'download') {
    return 'downloading';
  }

  const downloadedBytes = progress.downloaded_bytes || 0;
  const speed = progress.download_speed || 0;
  const deltaTime = now - state.lastDownload.ts;
  const deltaBytes = downloadedBytes - state.lastDownload.bytes;
  const instantaneous = deltaTime > 0 ? deltaBytes / (deltaTime / 1000) : speed;
  const currentSpeed = speed || instantaneous;

  // Track top speed (never reduced by slow periods to avoid drift)
  if (currentSpeed > state.topSpeed * 0.9) {
    state.topSpeed = currentSpeed;
  } else if (state.topSpeed === 0) {
    state.topSpeed = currentSpeed;
  }

  const threshold = state.topSpeed * 0.5;
  const belowThreshold = state.topSpeed > 0 && currentSpeed > 0 && currentSpeed < threshold;

  if (belowThreshold) {
    if (state.lowSince === null) {
      state.lowSince = now;
    }
    const lowDuration = now - state.lowSince;
    if (lowDuration >= 5000) {
      return 'stalled';
    }
  } else {
    state.lowSince = null;
  }

  state.lastDownload = {
    bytes: downloadedBytes,
    speed: currentSpeed,
    ts: now,
  };

  return 'downloading';
}

/**
 * Update download samples (5-second window)
 */
export function updateDownloadSamples(
  samples: { ts: number; bytes: number }[],
  now: number,
  bytes: number
): { ts: number; bytes: number }[] {
  const newSamples = [...samples, { ts: now, bytes }];
  return newSamples.filter((sample) => now - sample.ts <= 5000);
}
