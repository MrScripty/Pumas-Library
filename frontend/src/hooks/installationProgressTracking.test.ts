import { describe, expect, it } from 'vitest';
import type { RuntimeInstallationProgress } from '../generated/desktop-contract';
import { projectInstallationProgress } from './installationProgressTracking';

describe('projectInstallationProgress', () => {
  it('projects the validated camelCase wire into the established UI model', () => {
    const wire: RuntimeInstallationProgress = {
      tag: ' vλ.1 ',
      startedAt: 'started',
      stage: 'dependencies',
      stageProgress: 125.5,
      overallProgress: 107.25,
      currentItem: 'torch',
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
      etaSeconds: null, totalSize: null, dependencyCount: null, error: null,
      completedAt: null, success: null, logPath: null,
    };

    expect(projectInstallationProgress(wire)).toMatchObject({
      tag: '', started_at: '', stage: 'download', stage_progress: 0,
      overall_progress: 0, downloaded_bytes: 0, completed_dependencies: 0,
      completed_items: [], completed_at: undefined, success: undefined,
    });
  });
});
