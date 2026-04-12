import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { RemoteModelDownloadMenu } from './RemoteModelDownloadMenu';

function createModel(overrides: Partial<RemoteModelInfo> = {}): RemoteModelInfo {
  return {
    repoId: 'org/test-model',
    name: 'Test Model',
    developer: 'org',
    kind: 'text-generation',
    formats: ['gguf'],
    quants: ['Q4_K_M'],
    url: 'https://huggingface.co/org/test-model',
    totalSizeBytes: 3 * 1024 ** 3,
    ...overrides,
  };
}

describe('RemoteModelDownloadMenu', () => {
  it('starts a grouped-file download with the selected filenames and clears selection', () => {
    const onStartDownload = vi.fn().mockResolvedValue(undefined);
    const onCloseMenu = vi.fn();
    const onClearSelection = vi.fn();
    const onToggleGroup = vi.fn();
    const model = createModel({
      downloadOptions: [
        {
          quant: 'Q4_K_M',
          sizeBytes: 2 * 1024 ** 3,
          fileGroup: {
            label: 'Q4_K_M',
            shardCount: 2,
            filenames: ['part-1.gguf', 'part-2.gguf'],
          },
        },
      ],
    });

    render(
      <RemoteModelDownloadMenu
        downloadOptions={model.downloadOptions!}
        hasExactDetails={true}
        hasFileGroups={true}
        isHydratingDetails={false}
        model={model}
        selectedGroups={new Set(['Q4_K_M'])}
        selectedTotalBytes={2 * 1024 ** 3}
        onClearSelection={onClearSelection}
        onCloseMenu={onCloseMenu}
        onStartDownload={onStartDownload}
        onToggleGroup={onToggleGroup}
        collectSelectedFilenames={() => ['part-1.gguf', 'part-2.gguf']}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /download selected/i }));

    expect(onCloseMenu).toHaveBeenCalledTimes(1);
    expect(onStartDownload).toHaveBeenCalledWith(model, null, ['part-1.gguf', 'part-2.gguf']);
    expect(onClearSelection).toHaveBeenCalledTimes(1);
  });

  it('toggles grouped-file selections through the checkbox controls', () => {
    const onToggleGroup = vi.fn();
    const model = createModel({
      downloadOptions: [
        {
          quant: 'Q4_K_M',
          sizeBytes: 2 * 1024 ** 3,
          fileGroup: {
            label: 'Q4_K_M',
            shardCount: 2,
            filenames: ['part-1.gguf', 'part-2.gguf'],
          },
        },
      ],
    });

    render(
      <RemoteModelDownloadMenu
        downloadOptions={model.downloadOptions!}
        hasExactDetails={true}
        hasFileGroups={true}
        isHydratingDetails={false}
        model={model}
        selectedGroups={new Set()}
        selectedTotalBytes={0}
        onClearSelection={vi.fn()}
        onCloseMenu={vi.fn()}
        onStartDownload={vi.fn().mockResolvedValue(undefined)}
        onToggleGroup={onToggleGroup}
        collectSelectedFilenames={() => []}
      />
    );

    fireEvent.click(screen.getByRole('checkbox'));

    expect(onToggleGroup).toHaveBeenCalledWith('Q4_K_M');
  });

  it('starts a quant-specific download in quant menu mode', () => {
    const onStartDownload = vi.fn().mockResolvedValue(undefined);
    const onCloseMenu = vi.fn();
    const model = createModel({
      downloadOptions: [
        {
          quant: 'Q4_K_M',
          sizeBytes: 2 * 1024 ** 3,
          fileGroup: null,
        },
      ],
    });

    render(
      <RemoteModelDownloadMenu
        downloadOptions={model.downloadOptions!}
        hasExactDetails={true}
        hasFileGroups={false}
        isHydratingDetails={false}
        model={model}
        selectedGroups={new Set()}
        selectedTotalBytes={0}
        onClearSelection={vi.fn()}
        onCloseMenu={onCloseMenu}
        onStartDownload={onStartDownload}
        onToggleGroup={vi.fn()}
        collectSelectedFilenames={() => []}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /q4_k_m/i }));

    expect(onCloseMenu).toHaveBeenCalledTimes(1);
    expect(onStartDownload).toHaveBeenCalledWith(model, 'Q4_K_M');
  });
});
