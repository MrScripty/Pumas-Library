import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalModelsList } from '../components/LocalModelsList';
import { APIError } from '../errors';
import { useModelLibraryActions } from './useModelLibraryActions';

const { deleteModel } = vi.hoisted(() => ({ deleteModel: vi.fn() }));
vi.mock('../api/models', () => ({ modelsAPI: { deleteModel } }));
vi.mock('../api/adapter', () => ({ api: {}, isAPIAvailable: () => true }));

const selectedId = 'reranker/clm-v0_1/weights-original';
const otherId = 'reranker/clm-v0_1/weights-outq2';
const conflict = 'A download or another operation is using this model library. Finish or cancel it, then try again.';

function LibraryFixture() {
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});
  const actions = useModelLibraryActions({ setDownloadErrors, startDownload: vi.fn() });
  return <LocalModelsList
    modelGroups={[{ category: 'reranker', models: [
      { id: selectedId, name: 'CLM-v0.1-8B-GGUF', category: 'reranker', provenance: 'catalog',
        format: 'gguf', quant: 'Q8_0', size: 8709518976, isPartialDownload: true },
      { id: otherId, name: 'CLM-v0.1-8B-GGUF', category: 'reranker', provenance: 'catalog',
        format: 'gguf', quant: 'Q8_0', size: 8252495488, isPartialDownload: true },
    ] }]}
    starredModels={new Set()} excludedModels={new Set()} selectedAppId={null}
    totalModels={2} hasFilters={false} relatedModelsById={{}} expandedRelated={new Set()}
    onToggleStar={vi.fn()} onToggleLink={vi.fn()} onToggleRelated={vi.fn()}
    onOpenRelatedUrl={vi.fn()} onDeleteModel={actions.handleDeleteModel}
    downloadErrors={downloadErrors}
    deletionStateByModelId={actions.deletionStateByModelId}
  />;
}

describe('library deletion feedback', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'requestAnimationFrame', 'cancelAnimationFrame', 'performance'] });
  });
  afterEach(() => vi.useRealTimers());

  it('keeps the selected delete control disabled while the request is pending', async () => {
    let resolve!: (value: { success: boolean; error: string }) => void;
    deleteModel.mockReturnValueOnce(new Promise((finish) => { resolve = finish; }));
    render(<LibraryFixture />);
    const selectedRow = screen.getByText('8.11 GB').closest('.group') as HTMLElement;
    const otherRow = screen.getByText('7.69 GB').closest('.group') as HTMLElement;
    fireEvent.pointerDown(within(selectedRow).getByRole('button', { name: 'Hold to delete' }));
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(within(selectedRow).getByRole('button', { name: 'Deleting...' })).toBeDisabled();
    expect(within(otherRow).getByRole('button', { name: 'Hold to delete' })).toBeEnabled();
    await act(async () => { resolve({ success: false, error: conflict }); });
    expect(within(selectedRow).getByRole('button', { name: 'Hold to delete' })).toBeEnabled();
    expect(within(selectedRow).getByRole('alert')).toHaveTextContent(conflict);
  });

  it.each(['exception', 'unsuccessful response'])(
    'shows a refused deletion on the selected weight for an %s even when partial recovery is unavailable', async (outcome) => {
      if (outcome === 'exception') {
        deleteModel.mockRejectedValueOnce(new APIError(conflict, 'delete_model_with_cascade'));
      } else {
        deleteModel.mockResolvedValueOnce({ success: false, error: conflict });
      }
      render(<LibraryFixture />);
      const size = screen.getByText('8.11 GB');
      const row = size.closest('.group');
      expect(row).not.toBeNull();
      fireEvent.pointerDown(within(row as HTMLElement).getByRole('button', { name: 'Hold to delete' }));
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(deleteModel).toHaveBeenCalledExactlyOnceWith(selectedId);
      expect(within(row as HTMLElement).getByRole('alert')).toHaveTextContent(conflict);
      expect(screen.getByText('7.69 GB')).toBeVisible();
      expect(screen.getAllByRole('alert')).toHaveLength(1);
      expect(within(row as HTMLElement).getByRole('button', { name: 'Hold to delete' })).toBeEnabled();
      deleteModel.mockResolvedValueOnce({ success: true });
      fireEvent.pointerDown(within(row as HTMLElement).getByRole('button', { name: 'Hold to delete' }));
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(deleteModel).toHaveBeenLastCalledWith(selectedId);
      expect(deleteModel).toHaveBeenCalledTimes(2);
      expect(within(row as HTMLElement).queryByRole('alert')).not.toBeInTheDocument();
  });
});
