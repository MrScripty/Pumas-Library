import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { RemoteModelListItem } from './RemoteModelListItem';

function createModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    compatibleEngines: ['llama.cpp'],
    developer: 'org',
    downloadOptions: [
      {
        fileGroup: null,
        quant: 'Q4_K_M',
        sizeBytes: 2 * 1024 ** 3,
      },
    ],
    downloads: 123,
    formats: ['gguf'],
    kind: 'text-generation',
    name: 'Remote Model',
    quants: ['Q4_K_M'],
    releaseDate: '2026-01-01T00:00:00Z',
    repoId: 'org/remote-model',
    url: 'https://huggingface.co/org/remote-model',
    ...overrides,
  };
}

function renderItem(overrides: Partial<React.ComponentProps<typeof RemoteModelListItem>> = {}) {
  const props: React.ComponentProps<typeof RemoteModelListItem> = {
    isHydratingDetails: false,
    isMenuOpen: false,
    model: createModel(),
    onCancelDownload: vi.fn().mockResolvedValue(undefined),
    onClearSelection: vi.fn(),
    onCloseMenu: vi.fn(),
    onOpenUrl: vi.fn(),
    onPauseDownload: vi.fn().mockResolvedValue(undefined),
    onResumeDownload: vi.fn().mockResolvedValue(undefined),
    onStartDownload: vi.fn().mockResolvedValue(undefined),
    onToggleGroup: vi.fn(),
    onToggleMenu: vi.fn(),
    selectedGroups: new Set<string>(),
    ...overrides,
  };

  render(<RemoteModelListItem {...props} />);
  return props;
}

describe('RemoteModelListItem', () => {
  it('opens the remote model URL and toggles download options', () => {
    const props = renderItem();

    fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(props.onOpenUrl).toHaveBeenCalledWith('https://huggingface.co/org/remote-model');

    fireEvent.click(screen.getByRole('button', { name: 'Download options' }));
    expect(props.onToggleMenu).toHaveBeenCalledTimes(1);
  });

  it('hydrates missing exact details before showing the menu', () => {
    const onHydrateModelDetails = vi.fn().mockResolvedValue(undefined);
    const props = renderItem({
      model: createModel({
        downloadOptions: undefined,
        quantSizes: {},
      }),
      onHydrateModelDetails,
    });

    fireEvent.click(screen.getByRole('button', { name: 'Download options' }));

    expect(props.onToggleMenu).toHaveBeenCalledTimes(1);
    expect(onHydrateModelDetails).toHaveBeenCalledWith(props.model);
  });

  it('cancels active downloads from the primary download button', () => {
    const props = renderItem({
      downloadStatus: {
        downloadId: 'download-1',
        progress: 0.4,
        status: 'downloading',
      },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Cancel download' }));

    expect(props.onCloseMenu).toHaveBeenCalledTimes(1);
    expect(props.onCancelDownload).toHaveBeenCalledWith('org/remote-model');
  });
});
