import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalModelsList } from '../components/LocalModelsList';
import { buildDownloadingModels, mergeLocalModelGroups } from '../components/ModelManagerUtils';
import type { CatalogModel, DownloadProgressOutcome } from '@pumas/desktop-contract';
import type { ModelDownloadUpdateNotification } from '../types/api';
import { useDownloadCompletionRefresh } from './useDownloadCompletionRefresh';
import { useModelDownloads } from './useModelDownloads';
import { useModels } from './useModels';

const { getModels, listDownloads, resumeDownload, serveModel, electronAPI } = vi.hoisted(() => ({
  getModels: vi.fn(), listDownloads: vi.fn(), resumeDownload: vi.fn(), serveModel: vi.fn(),
  electronAPI: { onModelDownloadUpdate: vi.fn(), onModelLibraryUpdate: vi.fn() },
}));
vi.mock('../api/adapter', () => ({
  api: { get_models: getModels, list_model_downloads: listDownloads, resume_model_download: resumeDownload },
  getElectronAPI: () => electronAPI, isAPIAvailable: () => true,
}));
vi.mock('./useLauncherRootRecovery', () => ({ useLibraryScopeId: () => `display-v1:${'a'.repeat(64)}` }));

const selectedId = 'reranker/clm/weights-outq2';
const siblingId = 'reranker/clm/weights-original';
const failure = 'The model download did not complete successfully.';

function catalog(id: string, complete = false): CatalogModel {
  return { id, modelDir: `/models/${id}`, modelType: 'reranker', displayName: 'CLM-v0.1-8B-GGUF',
    inputModalities: [], outputModalities: [], adapter: false,
    format: 'gguf', quantization: 'Q8_0', sizeBytes: id === selectedId ? 8252495488 : 8709518976,
    dependencyCount: 0, relatedAvailable: false, integrity: { state: 'clean' },
    artifact: complete ? { state: 'complete' }
      : { state: 'partial', downloadProgressFraction: 0, reasons: ['expected_files_missing'] },
  };
}

function progress(overrides: Partial<DownloadProgressOutcome> = {}): DownloadProgressOutcome {
  return { downloadId: 'download-outq2', libraryModelId: selectedId, status: 'downloading',
    repoId: 'czl/CLM-v0.1-8B-GGUF', selectedArtifactId: 'artifact-outq2',
    progress: 0.9875525236129761, downloadedBytes: 8149772661, totalBytes: 8252495488,
    speed: null, etaSeconds: null, modelName: 'CLM-v0.1-8B-GGUF', modelType: 'reranker',
    retryAttempt: null, retryLimit: null, retrying: null, nextRetryDelaySeconds: null,
    error: null, ...overrides };
}

function LibraryFixture() {
  const models = useModels();
  const downloads = useModelDownloads();
  useDownloadCompletionRefresh({ downloadStatusByRepo: downloads.downloadStatusByRepo,
    onModelsImported: models.fetchModels });
  return <LocalModelsList
    modelGroups={mergeLocalModelGroups(models.modelGroups, buildDownloadingModels(downloads.downloadStatusByRepo))}
    starredModels={new Set()} excludedModels={new Set()} selectedAppId={null}
    totalModels={2} hasFilters={false} relatedModelsById={{}} expandedRelated={new Set()}
    onToggleStar={vi.fn()} onToggleLink={vi.fn()} onToggleRelated={vi.fn()} onOpenRelatedUrl={vi.fn()}
    onPauseDownload={downloads.pauseDownload} onResumeDownload={downloads.resumeDownload}
    onServeModel={serveModel} downloadErrors={downloads.downloadErrors}
  />;
}

describe('download outcome in the library', () => {
  let push: (notification: ModelDownloadUpdateNotification) => void;
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    localStorage.clear();
    getModels.mockResolvedValue({ success: true, models: {
      [selectedId]: catalog(selectedId), [siblingId]: catalog(siblingId),
    } });
    listDownloads.mockResolvedValue({ success: true, downloads: [progress()] });
    resumeDownload.mockResolvedValue({ success: true });
    electronAPI.onModelDownloadUpdate.mockImplementation((callback: (notification: ModelDownloadUpdateNotification) => void) => {
      push = callback;
      return vi.fn();
    });
    electronAPI.onModelLibraryUpdate.mockReturnValue(vi.fn());
  });
  afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); });

  async function update(downloads: DownloadProgressOutcome[], revision = 2) {
    await act(async () => push({ cursor: `download:${revision}`, snapshot: {
      cursor: `download:${revision}`, revision, downloads,
    }, stale_cursor: false, snapshot_required: false }));
  }

  it('shows a transfer failure on the exact weight and lets that download retry', async () => {
    render(<LibraryFixture />);
    await act(async () => { await Promise.resolve(); });
    await update([progress({ status: 'error', error: failure })]);
    const row = screen.getByText('7.69 GB').closest('.group') as HTMLElement;
    const sibling = screen.getByText('8.11 GB').closest('.group') as HTMLElement;
    expect(within(row).getByRole('alert')).toHaveTextContent(failure);
    expect(within(sibling).queryByRole('alert')).not.toBeInTheDocument();
    expect(within(row).queryByRole('button', { name: 'Serve model' })).not.toBeInTheDocument();
    fireEvent.click(within(row).getByRole('button', { name: 'Retry download' }));
    await act(async () => { await Promise.resolve(); });
    expect(resumeDownload).toHaveBeenCalledExactlyOnceWith('download-outq2');
    await update([progress()], 3);
    expect(within(row).queryByRole('alert')).not.toBeInTheDocument();
  });

  it('enables play only after confirmed completion and a fresh complete catalog entry', async () => {
    render(<LibraryFixture />);
    await act(async () => { await Promise.resolve(); });
    const row = screen.getByText('7.69 GB').closest('.group') as HTMLElement;
    await update([progress({ progress: 1, downloadedBytes: 8252495488 })]);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(getModels).toHaveBeenCalledTimes(1);
    expect(within(row).queryByRole('button', { name: 'Serve model' })).not.toBeInTheDocument();
    getModels.mockResolvedValue({ success: true, models: {
      [selectedId]: catalog(selectedId, true), [siblingId]: catalog(siblingId),
    } });
    await update([progress({ status: 'completed', progress: 1, downloadedBytes: 8252495488 })], 3);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(getModels).toHaveBeenCalledTimes(2);
    const play = within(row).getByRole('button', { name: 'Serve model' });
    expect(play).toBeEnabled();
    expect(within(row).queryByRole('button', { name: /download/i })).not.toBeInTheDocument();
    expect(within(screen.getByText('8.11 GB').closest('.group') as HTMLElement)
      .getByRole('button', { name: 'Serve model' })).toBeDisabled();
    fireEvent.click(play);
    expect(serveModel).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ id: selectedId }));
  });

  it('also reports an unassociated download failure without attaching it to a same-name weight', async () => {
    listDownloads.mockResolvedValue({ success: true, downloads: [progress({ libraryModelId: null })] });
    render(<LibraryFixture />);
    await act(async () => { await Promise.resolve(); });
    await update([progress({ libraryModelId: null, status: 'error', error: failure })]);
    const activity = screen.getByText('Download activity · error').closest('.group') as HTMLElement;
    expect(within(activity).getByRole('alert')).toHaveTextContent(failure);
    for (const size of ['7.69 GB', '8.11 GB']) {
      expect(within(screen.getByText(size).closest('.group') as HTMLElement)
        .queryByRole('alert')).not.toBeInTheDocument();
    }
    fireEvent.click(within(activity).getByRole('button', { name: 'Retry download' }));
    await act(async () => { await Promise.resolve(); });
    expect(resumeDownload).toHaveBeenCalledExactlyOnceWith('download-outq2');
  });

  it('keeps play disabled if a terminal update is followed by an incomplete catalog entry', async () => {
    render(<LibraryFixture />);
    await act(async () => { await Promise.resolve(); });
    await update([progress({ status: 'completed', progress: 1, downloadedBytes: 8252495488 })]);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(getModels).toHaveBeenCalledTimes(2);
    const row = screen.getByText('7.69 GB').closest('.group') as HTMLElement;
    expect(within(row).getByRole('button', { name: 'Serve model' })).toBeDisabled();
    expect(serveModel).not.toHaveBeenCalled();
  });
});
