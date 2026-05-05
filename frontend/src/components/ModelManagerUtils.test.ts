import { describe, expect, it } from 'vitest';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import type { ModelCategory, ModelInfo, RemoteModelInfo } from '../types/apps';
import {
  buildDownloadingModels,
  filterLocalModelGroups,
  isAuthRequiredError,
  mergeLocalModelGroups,
  resolveDownloadModelType,
  sortAndFilterRemoteResults,
} from './ModelManagerUtils';

function createLocalModel(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'model-1',
    name: 'Alpha Model',
    category: 'llm',
    path: '/models/alpha.gguf',
    repoId: 'Org/Alpha',
    size: 1024,
    ...overrides,
  };
}

function createDownloadStatus(overrides: Partial<DownloadStatus> = {}): DownloadStatus {
  return {
    downloadId: 'download-1',
    status: 'downloading',
    progress: 0.5,
    totalBytes: 4096,
    modelName: 'Alpha Remote',
    modelType: 'llm',
    ...overrides,
  };
}

function createRemoteModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'org/model-a',
    name: 'Model A',
    developer: 'org',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    url: 'https://huggingface.co/org/model-a',
    ...overrides,
  };
}

describe('ModelManagerUtils', () => {
  it('detects authentication-required download errors by HTTP status code', () => {
    expect(isAuthRequiredError('Request failed with HTTP 401 Unauthorized')).toBe(true);
    expect(isAuthRequiredError('Request failed with HTTP 403 Forbidden')).toBe(false);
  });

  it('builds downloading model overlays and ignores completed downloads', () => {
    const overlays = buildDownloadingModels({
      'org/model-a': createDownloadStatus({
        status: 'downloading',
        progress: 0.25,
        totalBytes: 1024,
      }),
      'org/model-b': createDownloadStatus({
        downloadId: 'download-2',
        status: 'completed',
        progress: 1,
        modelName: 'Completed Model',
      }),
      'org/model-c': createDownloadStatus({
        downloadId: 'download-3',
        status: 'error',
        modelName: undefined,
        modelType: undefined,
      }),
    });

    expect(overlays).toHaveLength(2);
    expect(overlays[0]).toMatchObject({
      id: 'download:org/model-a',
      name: 'Alpha Remote',
      category: 'llm',
      isDownloading: true,
      downloadRepoId: 'org/model-a',
      downloadProgress: 0.25,
    });
    expect(overlays[1]).toMatchObject({
      id: 'download:org/model-c',
      name: 'model-c',
      category: 'llm',
      downloadStatus: 'error',
    });
  });

  it('merges active downloads onto local models by repo id case-insensitively and prepends orphan downloads', () => {
    const localGroups: ModelCategory[] = [
      {
        category: 'llm',
        models: [createLocalModel()],
      },
    ];
    const downloadingModels: ModelInfo[] = [
      {
        id: 'download:org/alpha',
        name: 'Alpha Download',
        category: 'llm',
        isDownloading: true,
        downloadProgress: 0.7,
        downloadStatus: 'downloading',
        downloadRepoId: 'org/alpha',
        downloadTotalBytes: 9999,
      },
      {
        id: 'download:vision/model',
        name: 'Vision Download',
        category: 'vision',
        isDownloading: true,
        downloadProgress: 0.1,
        downloadStatus: 'queued',
        downloadRepoId: 'vision/model',
      },
    ];

    const merged = mergeLocalModelGroups(localGroups, downloadingModels);

    expect(merged.map((group) => group.category)).toEqual(['llm', 'vision']);
    expect(merged[0]?.models[0]).toMatchObject({
      id: 'model-1',
      isDownloading: true,
      downloadRepoId: 'org/alpha',
      downloadProgress: 0.7,
      downloadTotalBytes: 9999,
    });
    expect(merged[1]?.models[0]).toMatchObject({
      id: 'download:vision/model',
      name: 'Vision Download',
    });
  });

  it('does not merge an artifact-specific download onto a different quant from the same repo', () => {
    const localGroups: ModelCategory[] = [
      {
        category: 'vlm',
        models: [
          createLocalModel({
            id: 'vlm/davidau/qwen3_6-27b-heretic/q4',
            name: 'Qwen3.6 Heretic Q4_K_M',
            category: 'vlm',
            repoId: 'DavidAU/Qwen3.6-Heretic-GGUF',
            quant: 'Q4_K_M',
          }),
          createLocalModel({
            id: 'vlm/davidau/qwen3_6-27b-heretic/q5',
            name: 'Qwen3.6 Heretic Q5_K_M',
            category: 'vlm',
            repoId: 'DavidAU/Qwen3.6-Heretic-GGUF',
            quant: 'Q5_K_M',
            selectedArtifactId: 'davidau--qwen3_6-heretic-gguf__q5_k_m',
          }),
        ],
      },
    ];
    const downloadingModels: ModelInfo[] = [
      {
        id: 'download:davidau--qwen3_6-heretic-gguf__q4_k_m',
        name: 'Qwen3.6 Heretic Q4_K_M',
        category: 'vlm',
        isDownloading: true,
        downloadKey: 'davidau--qwen3_6-heretic-gguf__q4_k_m',
        downloadRepoId: 'davidau/qwen3.6-heretic-gguf',
        downloadSelectedArtifactId: 'davidau--qwen3_6-heretic-gguf__q4_k_m',
        downloadProgress: 0.33,
      },
    ];

    const merged = mergeLocalModelGroups(localGroups, downloadingModels);

    expect(merged[0]?.models[0]).toMatchObject({
      id: 'vlm/davidau/qwen3_6-27b-heretic/q4',
      isDownloading: true,
      downloadKey: 'davidau--qwen3_6-heretic-gguf__q4_k_m',
      downloadProgress: 0.33,
    });
    expect(merged[0]?.models[1]).toMatchObject({
      id: 'vlm/davidau/qwen3_6-27b-heretic/q5',
      quant: 'Q5_K_M',
    });
    expect(merged[0]?.models[1]?.isDownloading).toBeUndefined();
    expect(merged[0]?.models).toHaveLength(2);
  });

  it('falls back to repo matching when a download has no artifact identity', () => {
    const localGroups: ModelCategory[] = [
      {
        category: 'llm',
        models: [
          createLocalModel({
            repoId: 'Org/Alpha',
          }),
        ],
      },
    ];
    const downloadingModels: ModelInfo[] = [
      {
        id: 'download:org/alpha',
        name: 'Alpha Download',
        category: 'llm',
        isDownloading: true,
        downloadKey: 'org/alpha',
        downloadRepoId: 'org/alpha',
        downloadProgress: 0.25,
      },
    ];

    const merged = mergeLocalModelGroups(localGroups, downloadingModels);

    expect(merged[0]?.models[0]).toMatchObject({
      id: 'model-1',
      isDownloading: true,
      downloadKey: 'org/alpha',
      downloadProgress: 0.25,
    });
  });

  it('filters local groups by selected category and matches search against model path', () => {
    const groups: ModelCategory[] = [
      {
        category: 'llm',
        models: [createLocalModel()],
      },
      {
        category: 'vision',
        models: [
          createLocalModel({
            id: 'vision-1',
            name: 'Vision Encoder',
            category: 'vision',
            path: '/models/images/encoder.safetensors',
          }),
        ],
      },
    ];

    const filtered = filterLocalModelGroups(groups, 'encoder', 'vision');

    expect(filtered).toHaveLength(1);
    expect(filtered[0]?.category).toBe('vision');
    expect(filtered[0]?.models.map((model) => model.id)).toEqual(['vision-1']);
  });

  it('sorts remote results by release date descending after kind filtering', () => {
    const remoteResults: RemoteModelInfo[] = [
      createRemoteModel({
        repoId: 'org/model-old',
        name: 'Model Old',
        kind: 'image-to-text',
        releaseDate: '2024-01-10T00:00:00Z',
      }),
      createRemoteModel({
        repoId: 'org/model-new',
        name: 'Model New',
        kind: 'image-to-text',
        releaseDate: '2025-03-05T00:00:00Z',
      }),
      createRemoteModel({
        repoId: 'org/model-ignored',
        name: 'Ignored',
        kind: 'text-generation',
        releaseDate: '2026-01-01T00:00:00Z',
      }),
    ];

    const filtered = sortAndFilterRemoteResults(remoteResults, 'image-to-text');

    expect(filtered.map((model) => model.repoId)).toEqual([
      'org/model-new',
      'org/model-old',
    ]);
  });

  it('maps remote pipeline kinds to local download model categories', () => {
    expect(resolveDownloadModelType('image-text-to-text')).toBe('vlm');
    expect(resolveDownloadModelType('feature-extraction')).toBe('embedding');
    expect(resolveDownloadModelType('unknown-pipeline')).toBe('unknown');
  });
});
