import { describe, expect, it } from 'vitest';
import { decodeDownloadListOutcome, type DownloadProgressOutcome } from '@pumas/desktop-contract';
import { projectDownloadProgress } from './downloadProgressProjection';

describe('RPC progress to pushed-snapshot presentation adapter', () => {
  it('keeps nullable absence and error status without inventing model association', () => {
    const progress: DownloadProgressOutcome = {
      downloadId: 'download-1', status: 'error', repoId: null, selectedArtifactId: null, libraryModelId: null,
      progress: null, downloadedBytes: null, totalBytes: null, speed: null, etaSeconds: null,
      modelName: null, modelType: null, retryAttempt: null, retryLimit: null, retrying: null,
      nextRetryDelaySeconds: null, error: 'Interrupted',
    };
    expect(projectDownloadProgress(progress)).toMatchObject({
      downloadId: 'download-1', status: 'error', error: 'Interrupted',
      repoId: undefined, progress: undefined, downloadedBytes: undefined, libraryModelId: null,
    });
    expect(projectDownloadProgress(progress)).not.toHaveProperty('modelId');
    expect(projectDownloadProgress({ ...progress, libraryModelId: 'llm/acme/model' })).toMatchObject({
      libraryModelId: 'llm/acme/model',
    });
    expect(decodeDownloadListOutcome({ success: true, downloads: [{
      ...progress, status: 'unknown-status',
    }] }).status).toBe('invalid');
  });
});
