import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { RetainedDownloadFailures } from './RetainedDownloadFailures';
import type { DownloadLocationReport } from '../generated/desktop-contract';
import type { DownloadStatus } from '../hooks/modelDownloadState';

const { inspect, resolve } = vi.hoisted(() => ({ inspect: vi.fn(), resolve: vi.fn() }));
vi.mock('../api/models', () => ({ modelsAPI: { inspectRetainedDownload: inspect, resolveRetainedDownload: resolve } }));
const downloadId = '46657526-e282-44b2-89e1-dbe5632904c9';
const report: DownloadLocationReport = {
  status: 'existing_local_candidate', downloadId, attemptId: '8f18f8bf-f1ce-40be-9723-4e349450ecdb',
  originalDestination: 'diffusion/inclusionai/legacy-model', requiresFormatUpgrade: true,
  message: 'Existing local files were found.', resolutionRequest: null,
  candidates: [{ modelId: 'diffusion/vit/legacy-model', displayName: 'LLaDA-Image-Turbo-FP8',
    relativeDestination: 'diffusion/vit/legacy-model', selectedFileCount: 33, sizeBytes: 27610502872,
    observation: 'a'.repeat(64) }],
};
const downloads = { failed: { downloadId, status: 'error', modelName: 'LLaDA-Image-Turbo-FP8',
  repoId: 'inclusionAI/LLaDA-Image-Turbo-FP8', progress: 0, totalBytes: 27607480398 } as DownloadStatus };

describe('retained failed download recovery', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    inspect.mockResolvedValue(report);
    resolve.mockResolvedValue({ ...report, status: 'resolved', requiresFormatUpgrade: false, message: 'Old failure resolved.' });
  });

  async function open(onResolved = vi.fn().mockResolvedValue(undefined)) {
    render(<RetainedDownloadFailures downloads={downloads} onResolved={onResolved} />);
    fireEvent.click(screen.getByRole('button', { name: 'Inspect existing files' }));
    await act(async () => { await Promise.resolve(); });
    await waitFor(() => expect(screen.getByRole('dialog')).toBeVisible());
    return screen.getByRole('dialog');
  }

  it('offers orphan failure inspection with provenance and compatibility warnings before confirming', async () => {
    const dialog = await open();
    expect(inspect).toHaveBeenCalledExactlyOnceWith(downloadId);
    expect(within(dialog).getByText(/33 selected files/)).toBeVisible();
    expect(within(dialog).getByText(/does not verify their historical source/)).toBeVisible();
    expect(within(dialog).getByRole('note')).toHaveTextContent('Older applications that open the library directly will no longer be able to read it');
    expect(resolve).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(resolve).not.toHaveBeenCalled();
  });

  it('confirms only the exact preview and refreshes in the same session', async () => {
    const refreshed = vi.fn().mockResolvedValue(undefined);
    const dialog = await open(refreshed);
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep existing files and resolve failure' }));
    await act(async () => { await Promise.resolve(); });
    expect(resolve).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ downloadId, attemptId: report.attemptId,
      candidateModelId: report.candidates[0]?.modelId, expectedObservation: 'a'.repeat(64),
      acceptance: 'keep_existing_local_files', acceptStorageUpgrade: true }));
    expect(refreshed).toHaveBeenCalledTimes(1);
    expect(within(dialog).getByText('Old failure resolved.')).toBeVisible();
    expect(within(dialog).getByRole('button', { name: 'Close' })).toBeEnabled();
  });

  it('retries the identical operation after a lost acknowledgement', async () => {
    resolve.mockRejectedValueOnce(new Error('disconnected'));
    const dialog = await open();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Keep existing files and resolve failure' }));
    await act(async () => { await Promise.resolve(); });
    expect(within(dialog).getByRole('alert')).toHaveTextContent('Retry this same confirmation');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Retry confirmation' }));
    await act(async () => { await Promise.resolve(); });
    expect(resolve.mock.calls[1]?.[0]).toEqual(resolve.mock.calls[0]?.[0]);
  });

  it('requires explicit selection for ambiguous candidates and permits refreshing a stale preview', async () => {
    inspect.mockResolvedValue({ ...report, status: 'ambiguous_candidates', candidates: [report.candidates[0],
      { ...report.candidates[0], modelId: 'diffusion/vit/other-weights', relativeDestination: 'diffusion/vit/other-weights' }] });
    resolve.mockResolvedValue({ ...report, status: 'stale_observation', candidates: [], message: 'Inspect again.' });
    const dialog = await open();
    const confirm = within(dialog).getByRole('button', { name: 'Keep existing files and resolve failure' });
    expect(confirm).toBeDisabled();
    fireEvent.click(within(dialog).getAllByRole('radio')[1] as HTMLElement);
    fireEvent.click(confirm);
    await act(async () => { await Promise.resolve(); });
    expect(within(dialog).getByRole('button', { name: 'Inspect again' })).toBeEnabled();
    expect(resolve).toHaveBeenCalledWith(expect.objectContaining({ candidateModelId: 'diffusion/vit/other-weights' }));
  });
});
