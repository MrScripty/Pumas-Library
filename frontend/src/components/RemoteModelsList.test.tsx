import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { RemoteModelsList } from './RemoteModelsList';
import type { RemoteModelInfo } from '../types/apps';

function createModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'test/model',
    name: 'Test Model',
    developer: 'test',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    downloadOptions: [],
    url: 'https://huggingface.co/test/model',
    releaseDate: '2026-01-01T00:00:00Z',
    downloads: 123,
    totalSizeBytes: null,
    quantSizes: undefined,
    compatibleEngines: ['ollama'],
    ...overrides,
  };
}

describe('RemoteModelsList', () => {
  const baseProps = {
    isLoading: false,
    error: null,
    searchQuery: 'test',
    downloadStatusByRepo: {},
    downloadErrors: {},
    hydratingRepoIds: new Set<string>(),
    onHydrateModelDetails: vi.fn().mockResolvedValue(undefined),
    onStartDownload: vi.fn().mockResolvedValue(undefined),
    onCancelDownload: vi.fn().mockResolvedValue(undefined),
    onPauseDownload: vi.fn().mockResolvedValue(undefined),
    onResumeDownload: vi.fn().mockResolvedValue(undefined),
    onOpenUrl: vi.fn(),
    onSearchDeveloper: vi.fn(),
    onClearFilters: vi.fn(),
    selectedKind: 'all',
    onHfAuthClick: vi.fn(),
  };

  it('hydrates download details on demand before opening options', () => {
    const onHydrateModelDetails = vi.fn().mockResolvedValue(undefined);
    const onStartDownload = vi.fn().mockResolvedValue(undefined);

    render(
      <RemoteModelsList
        {...baseProps}
        models={[createModel()]}
        onHydrateModelDetails={onHydrateModelDetails}
        onStartDownload={onStartDownload}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Download options' }));

    expect(onHydrateModelDetails).toHaveBeenCalledTimes(1);
    expect(onHydrateModelDetails).toHaveBeenCalledWith(
      expect.objectContaining({ repoId: 'test/model' })
    );
    expect(onStartDownload).not.toHaveBeenCalled();
  });

  it('shows exact download options immediately when details are already present', () => {
    render(
      <RemoteModelsList
        {...baseProps}
        models={[
          createModel({
            downloadOptions: [
              {
                quant: 'Q4_K_M',
                sizeBytes: 2 * 1024 ** 3,
                fileGroup: null,
              },
            ],
            totalSizeBytes: 2 * 1024 ** 3,
          }),
        ]}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Download options' }));

    expect(screen.getByText(/Q4_K_M \(2\.00 GB\)/)).toBeInTheDocument();
  });

  it('labels active same-repo artifact downloads', () => {
    render(
      <RemoteModelsList
        {...baseProps}
        models={[
          createModel({
            quants: ['Q4_K_M', 'Q5_K_M'],
            downloadOptions: [
              {
                quant: 'Q4_K_M',
                selectedArtifactId: 'test--model__q4_k_m',
                sizeBytes: 2 * 1024 ** 3,
                fileGroup: null,
              },
              {
                quant: 'Q5_K_M',
                selectedArtifactId: 'test--model__q5_k_m',
                sizeBytes: 3 * 1024 ** 3,
                fileGroup: null,
              },
            ],
          }),
        ]}
        downloadStatusByRepo={{
          'test--model__q4_k_m': {
            downloadId: 'download-q4',
            repoId: 'test/model',
            selectedArtifactId: 'test--model__q4_k_m',
            status: 'downloading',
            progress: 0.4,
          },
          'test--model__q5_k_m': {
            downloadId: 'download-q5',
            repoId: 'test/model',
            selectedArtifactId: 'test--model__q5_k_m',
            status: 'queued',
            progress: 0,
          },
        }}
      />
    );

    expect(screen.getByText('Active:')).toBeInTheDocument();
    expect(screen.getByTitle('Active artifact Q4_K_M')).toBeInTheDocument();
    expect(screen.getByTitle('Active artifact Q5_K_M')).toBeInTheDocument();
  });
});
