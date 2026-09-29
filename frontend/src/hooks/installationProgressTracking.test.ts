import { describe, expect, it } from 'vitest';
import type { RuntimeInstallationProgress } from '../generated/desktop-contract';
import { createNetworkStatusState } from '../utils/networkStatusMonitor';
import { normalizeInstallationProgress, projectInstallationProgress } from './installationProgressTracking';

describe('projectInstallationProgress', () => {
  it('projects the validated camelCase wire into the established UI model', () => {
    const wire: RuntimeInstallationProgress = {
      tag: ' vλ.1 ',
      startedAt: 'started',
      stage: 'dependencies',
      stageProgress: 125.5,
      overallProgress: 107.25,
      currentItem: 'torch',
      downloadSourceUrl: 'https://download.pytorch.org/whl/cu134/torch-2.14.0.whl',
      downloadActive: true,
      downloadMeasurementAvailable: true,
      downloadSpeed: 42.5,
      etaSeconds: 0.5,
      totalSize: 1024,
      downloadedBytes: 512,
      dependencyCount: 2,
      completedDependencies: 1,
      completedItems: [{ name: 'torch', type: 'package', size: null, completedAt: 'item done' }],
      error: null,
      completedAt: 'done',
      success: false,
      logPath: 'runtime/install.log',
    };

    expect(projectInstallationProgress(wire)).toEqual({
      tag: ' vλ.1 ',
      started_at: 'started',
      stage: 'dependencies',
      stage_progress: 125.5,
      overall_progress: 107.25,
      current_item: 'torch',
      download_source_url: 'https://download.pytorch.org/whl/cu134/torch-2.14.0.whl',
      download_active: true,
      download_measurement_available: true,
      download_speed: 42.5,
      eta_seconds: 0.5,
      total_size: 1024,
      downloaded_bytes: 512,
      dependency_count: 2,
      completed_dependencies: 1,
      completed_items: [{ name: 'torch', type: 'package', size: null, completed_at: 'item done' }],
      error: null,
      completed_at: 'done',
      success: false,
      log_path: 'runtime/install.log',
    });
  });

  it('uses presentation defaults only after nullable wire facts are validated', () => {
    const wire: RuntimeInstallationProgress = {
      tag: '', startedAt: '', stage: 'download', downloadedBytes: 0,
      completedDependencies: 0, completedItems: [], stageProgress: null,
      overallProgress: null, currentItem: null, downloadSpeed: null,
      downloadSourceUrl: null, downloadActive: false, downloadMeasurementAvailable: null,
      etaSeconds: null, totalSize: null, dependencyCount: null, error: null,
      completedAt: null, success: null, logPath: null,
    };

    expect(projectInstallationProgress(wire)).toMatchObject({
      tag: '', started_at: '', stage: 'download', stage_progress: 0,
      overall_progress: 0, downloaded_bytes: 0, completed_dependencies: 0,
      completed_items: [], download_active: false, download_measurement_available: null,
      completed_at: undefined, success: undefined,
    });
  });

  it('clears the sampled fallback rate when an unattributed transfer becomes inactive', () => {
    const trackerState = {
      lastDownloadTag: null,
      lastStage: null,
      lastDownloadSourceUrl: null,
      networkState: createNetworkStatusState(),
    };
    const progress = {
      tag: 'v2.14.0',
      started_at: 'started',
      stage: 'dependencies' as const,
      stage_progress: 0,
      overall_progress: 95,
      current_item: 'Installing official Torch packages',
      download_source_url: null,
      download_active: true,
      download_measurement_available: true,
      download_speed: null,
      eta_seconds: null,
      total_size: null,
      downloaded_bytes: 1024,
      dependency_count: 1,
      completed_dependencies: 0,
      completed_items: [],
      error: null,
    };

    const first = normalizeInstallationProgress(progress, [], trackerState, 1000);
    expect(first.adjustedProgress.download_speed).toBeNull();
    const duringTransfer = normalizeInstallationProgress(
      { ...progress, downloaded_bytes: 2048 }, [], trackerState, 2000,
    );
    expect(duringTransfer.adjustedProgress.download_speed).toBe(1024);

    const afterTransfer = normalizeInstallationProgress(
      { ...progress, download_active: false, downloaded_bytes: 2048 }, [], trackerState, 3000,
    );
    expect(afterTransfer.adjustedProgress.download_speed).toBeNull();
  });
});
