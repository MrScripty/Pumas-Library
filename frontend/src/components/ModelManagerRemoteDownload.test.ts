import { describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { RemoteModelInfo } from '../types/apps';
import { startRemoteModelDownload } from './ModelManagerRemoteDownload';

function createRemoteModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'org/model',
    name: 'Model',
    developer: 'org',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    url: 'https://huggingface.co/org/model',
    releaseDate: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function createDownloadErrorState(initial: Record<string, string> = {}) {
  let errors = initial;
  const setDownloadErrors = vi.fn((updater: (prev: Record<string, string>) => Record<string, string>) => {
    errors = updater(errors);
  });
  return {
    get errors() {
      return errors;
    },
    setDownloadErrors,
  };
}

describe('startRemoteModelDownload', () => {
  it('starts a backend download and clears stale repo errors', async () => {
    const apiClient = {
      start_model_download_from_hf: vi.fn().mockResolvedValue({
        success: true,
        download_id: 'download-1',
        selectedArtifactId: 'org/model::Q4_K_M',
      }),
    };
    const startDownload = vi.fn();
    const downloadErrors = createDownloadErrorState({ 'org/model': 'previous error' });

    await startRemoteModelDownload({
      apiClient,
      isApiAvailable: () => true,
      loggerInstance: { error: vi.fn(), info: vi.fn() },
      model: createRemoteModel(),
      openHfAuth: vi.fn(),
      quant: 'Q4_K_M',
      filenames: ['model.gguf'],
      setDownloadErrors: downloadErrors.setDownloadErrors,
      startDownload,
    });

    expect(apiClient.start_model_download_from_hf).toHaveBeenCalledWith(
      'org/model',
      'org',
      'Model',
      'llm',
      'text-generation',
      '2026-01-01T00:00:00Z',
      'https://huggingface.co/org/model',
      'Q4_K_M',
      ['model.gguf']
    );
    expect(startDownload).toHaveBeenCalledWith('org/model::Q4_K_M', 'download-1', {
      repoId: 'org/model',
      selectedArtifactId: 'org/model::Q4_K_M',
      artifactId: undefined,
      modelName: 'Model',
      modelType: 'llm',
    });
    expect(downloadErrors.errors).toEqual({});
  });

  it('records backend failures without starting a local download', async () => {
    const apiClient = {
      start_model_download_from_hf: vi.fn().mockResolvedValue({
        success: false,
        error: 'quota exceeded',
      }),
    };
    const startDownload = vi.fn();
    const downloadErrors = createDownloadErrorState();

    await startRemoteModelDownload({
      apiClient,
      isApiAvailable: () => true,
      loggerInstance: { error: vi.fn(), info: vi.fn() },
      model: createRemoteModel(),
      openHfAuth: vi.fn(),
      setDownloadErrors: downloadErrors.setDownloadErrors,
      startDownload,
    });

    expect(startDownload).not.toHaveBeenCalled();
    expect(downloadErrors.errors).toEqual({ 'org/model': 'quota exceeded' });
  });

  it('opens Hugging Face auth when a thrown API error requires auth', async () => {
    const apiClient = {
      start_model_download_from_hf: vi.fn().mockRejectedValue(
        new APIError('HTTP 401 Unauthorized', 'start_model_download_from_hf')
      ),
    };
    const openHfAuth = vi.fn();
    const downloadErrors = createDownloadErrorState();

    await startRemoteModelDownload({
      apiClient,
      isApiAvailable: () => true,
      loggerInstance: { error: vi.fn(), info: vi.fn() },
      model: createRemoteModel(),
      openHfAuth,
      setDownloadErrors: downloadErrors.setDownloadErrors,
      startDownload: vi.fn(),
    });

    expect(downloadErrors.errors).toEqual({ 'org/model': 'HTTP 401 Unauthorized' });
    expect(openHfAuth).toHaveBeenCalledTimes(1);
  });
});
