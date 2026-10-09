import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { ModelManager } from './ModelManager';
import { APIError } from '../errors';
import type { GetHFDownloadDetailsResponse, SearchHFModelsResponse } from '../types/api-models';
const mocks = vi.hoisted(() => ({ search: vi.fn(), details: vi.fn(), start: vi.fn(), admit: vi.fn() }));
vi.mock('../api/adapter', () => ({ api: { search_hf_models: mocks.search, get_hf_download_details: mocks.details, start_model_download_from_hf: mocks.admit }, isAPIAvailable: () => true }));
vi.mock('../hooks/useDownloadCompletionRefresh', () => ({
  useDownloadCompletionRefresh: vi.fn(),
}));

vi.mock('../hooks/useExistingLibraryChooser', () => ({
  useExistingLibraryChooser: () => ({
    chooseExistingLibrary: vi.fn(),
    isChoosingExistingLibrary: false,
  }),
}));

vi.mock('../hooks/useHfAuthPrompt', () => ({
  useHfAuthPrompt: () => ({
    closeHfAuth: vi.fn(),
    isHfAuthOpen: false,
    openHfAuth: vi.fn(),
  }),
}));

vi.mock('../hooks/useModelDownloads', () => ({
  useModelDownloads: () => ({
    cancelDownload: vi.fn(),
    downloadErrors: {},
    downloadStatusByRepo: {},
    hasActiveDownloads: false,
    pauseDownload: vi.fn(),
    resumeDownload: vi.fn(),
    setDownloadErrors: vi.fn(),
    startDownload: mocks.start,
  }),
}));

vi.mock('../hooks/useModelImportPicker', () => ({
  useModelImportPicker: () => ({
    closeImportDialog: vi.fn(),
    completeImport: vi.fn(),
    importPaths: [],
    openImportPicker: vi.fn(),
    pickerError: null,
    isPicking: false,
    showImportDialog: false,
  }),
}));

vi.mock('../hooks/useModelLibraryActions', () => ({
  useModelLibraryActions: () => ({
    expandedRelated: new Set<string>(),
    handleDeleteModel: vi.fn(),
    handleRecoverPartialDownload: vi.fn(),
    handleToggleRelated: vi.fn(),
    openRemoteUrl: vi.fn(),
    recoveringPartialModelIds: new Set<string>(),
    relatedModelsById: {},
  }),
}));


vi.mock('../hooks/useNetworkStatus', () => ({
  useNetworkStatus: () => ({
    circuitBreakerRejections: 0,
    isOffline: true,
    isRateLimited: false,
    successRate: 1,
  }),
}));


vi.mock('./HuggingFaceAuthDialog', () => ({
  HuggingFaceAuthDialog: () => null,
}));

vi.mock('./LinkHealthStatus', () => ({
  LinkHealthStatus: () => null,
}));

vi.mock('./MigrationReportsPanel', () => ({
  MigrationReportsPanel: () => null,
}));

vi.mock('./ModelImportDialog', () => ({
  ModelImportDialog: () => null,
}));



vi.mock('./LocalModelsList', () => ({ LocalModelsList: () => null }));
function fixture(name: string): RemoteModelInfo {
  return { repoId: `acme/${name}`, name, developer: 'acme', kind: 'text-generation', formats: ['gguf'], quants: ['Q4_K_M'], url: `https://huggingface.co/acme/${name}`, downloadOptions: [] };
}
function trigger(index: number): HTMLElement {
  const button = screen.getAllByRole('button', { name: 'Download options' })[index];
  if (!button) throw new TypeError('Missing download menu trigger');
  return button;
}
function mount() {
  render(<ModelManager modelGroups={[]} libraryLoadStatus="ready" starredModels={new Set()} excludedModels={new Set()} selectedAppId={null} onToggleStar={vi.fn()} onToggleLink={vi.fn()} onAddModels={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: 'Search Hugging Face models' }));
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'demand-fixture' } });
}
function details(name: string, size: number | null = 4096) {
  return { success: true, details: { repoId: `acme/${name}`, downloadOptions: [{ quant: 'Q4_K_M', sizeBytes: size }], totalSizeBytes: size } };
}
const rpcBase = process.env['PUMAS_HF_DEMAND_RPC_URL'];
async function rpc<T>(method: string, params: object): Promise<T> {
  if (!rpcBase?.startsWith('http://127.0.0.1:')) throw new TypeError('Owned loopback RPC required');
  const response = await fetch(`${rpcBase}/rpc`, { method: 'POST', headers: { 'Content-Type': 'application/json' },
    signal: AbortSignal.timeout(5000), body: JSON.stringify({ jsonrpc: '2.0', id: 417, method, params }) });
  const body = await response.json() as { error?: unknown; id: number; result: T };
  if (!response.ok || body.error || body.id !== 417) throw new APIError('RPC response envelope failed', method);
  return body.result;
}
describe('mounted demand-only HF details', () => {
  beforeEach(() => {
    mocks.search.mockReset().mockResolvedValue({ success: true, models: [fixture('First'), fixture('Second')] });
    mocks.details.mockReset(); mocks.start.mockReset(); mocks.admit.mockReset().mockResolvedValue({ success: true, download_id: 'owned-download' });
  });
  afterEach(cleanup);
  it('discovers without eager hydration and loads only the selected row before downloading', async () => {
    let release!: (value: ReturnType<typeof details>) => void;
    mocks.details.mockReturnValueOnce(new Promise(resolve => { release = resolve; }));
    mount(); await screen.findByText('Second');
    expect(mocks.search).toHaveBeenCalledWith('demand-fixture', null, 25, 0);
    expect(mocks.details).not.toHaveBeenCalled();
    fireEvent.click(trigger(1));
    const menu = screen.getByRole('dialog', { name: 'Download options for Second' });
    expect(await within(menu).findByText('Loading exact download details...')).toBeInTheDocument();
    expect(within(menu).queryByRole('button', { name: 'All files' })).not.toBeInTheDocument();
    expect(mocks.details).toHaveBeenCalledTimes(1);
    expect(mocks.details).toHaveBeenCalledWith('acme/Second', ['Q4_K_M']);
    expect(mocks.start).not.toHaveBeenCalled();
    await act(async () => { release(details('Second')); });
    fireEvent.click(await within(menu).findByRole('button', { name: /Q4_K_M/ }));
    await waitFor(() => expect(mocks.admit).toHaveBeenCalledWith('acme/Second', 'acme', 'Second', 'llm', 'text-generation', null, 'https://huggingface.co/acme/Second', 'Q4_K_M', null));
    expect(mocks.details).toHaveBeenCalledTimes(1);
  });
  it('shows a failed selected lookup, blocks download choices and retries only on explicit click', async () => {
    mocks.details.mockResolvedValueOnce({ success: false, error: 'Owned detail lookup unavailable' })
      .mockResolvedValueOnce(details('First'));
    mount(); await screen.findByText('First');
    fireEvent.click(trigger(0));
    const menu = screen.getByRole('dialog', { name: 'Download options for First' });
    expect(await within(menu).findByRole('alert')).toHaveTextContent('Owned detail lookup unavailable');
    expect(within(menu).queryByRole('button', { name: /All files|Q4_K_M/ })).not.toBeInTheDocument();
    expect(mocks.details).toHaveBeenCalledTimes(1); expect(mocks.start).not.toHaveBeenCalled();
    fireEvent.click(trigger(0));
    fireEvent.click(trigger(0));
    expect(screen.getByRole('alert')).toHaveTextContent('Owned detail lookup unavailable');
    expect(mocks.details).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Retry download details' }));
    await within(menu).findByRole('button', { name: /Q4_K_M/ });
    expect(within(menu).queryByRole('alert')).not.toBeInTheDocument();
    expect(mocks.details.mock.calls).toEqual([['acme/First', ['Q4_K_M']], ['acme/First', ['Q4_K_M']]]);
    expect(mocks.start).not.toHaveBeenCalled();
  });
  it('accepts fetched unknown sizes and empty results without inventing download options', async () => {
    mocks.details.mockResolvedValueOnce(details('First', null)).mockResolvedValueOnce({ success: true, details: { repoId: 'acme/Second', downloadOptions: [], totalSizeBytes: null } });
    mount(); await screen.findByText('First');
    fireEvent.click(trigger(0));
    await screen.findByRole('button', { name: 'Q4_K_M (Unknown)' });
    fireEvent.click(trigger(0));
    fireEvent.click(trigger(1));
    const second = screen.getByRole('dialog', { name: 'Download options for Second' });
    await within(second).findByText('No downloadable files were found.');
    expect(within(second).queryByRole('button', { name: /All files|Q4_K_M/ })).not.toBeInTheDocument();
    expect(mocks.start).not.toHaveBeenCalled();
  });
  it.skipIf(!rpcBase)('defers real RPC detail requests until selection and retries only the chosen repository', async () => {
    mocks.search.mockImplementation((query: string, kind: string | null, limit: number, hydrateLimit: number) =>
      rpc<SearchHFModelsResponse>('search_hf_models', { query, kind, limit, hydrate_limit: hydrateLimit }));
    mocks.details.mockImplementation((repoId: string, quants: string[]) =>
      rpc<GetHFDownloadDetailsResponse>('get_hf_download_details', { repo_id: repoId, quants }));
    mount(); await screen.findByText('Second');
    expect(mocks.search).toHaveBeenCalledWith('demand-fixture', null, 25, 0);
    expect(mocks.details).not.toHaveBeenCalled();
    fireEvent.click(trigger(1));
    const menu = screen.getByRole('dialog', { name: 'Download options for Second' });
    await within(menu).findByRole('alert');
    expect(within(menu).queryByRole('button', { name: /All files|Q4_K_M/ })).not.toBeInTheDocument();
    expect(mocks.details.mock.calls).toEqual([['acme/Second', ['Q4_K_M']]]);
    fireEvent.click(within(menu).getByRole('button', { name: 'Retry download details' }));
    await waitFor(() => expect(mocks.details).toHaveBeenCalledTimes(2));
    await within(menu).findByRole('alert');
    expect(mocks.details.mock.calls).toEqual([['acme/Second', ['Q4_K_M']], ['acme/Second', ['Q4_K_M']]]);
    expect(mocks.admit).not.toHaveBeenCalled(); expect(mocks.start).not.toHaveBeenCalled();
  });
});
