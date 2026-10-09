import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteModelInfo } from '../types/apps';
import { APIError } from '../errors';
import { ModelManager } from './ModelManager';
const mocks = vi.hoisted(() => ({ search: vi.fn(), details: vi.fn(), start: vi.fn() }));
vi.mock('../api/adapter', () => ({ api: { search_hf_models: mocks.search, get_hf_download_details: mocks.details }, isAPIAvailable: () => true }));
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
const base = process.env['PUMAS_HF_UI_RPC_URL'];
function fixture(name: string): RemoteModelInfo {
  return { repoId: `acme/${name}`, name, developer: 'acme', kind: 'text-generation', formats: ['gguf'], quants: ['Q4_K_M'], url: `https://huggingface.co/acme/${name}`, downloads: 1, compatibleEngines: [], downloadOptions: [], modelCard: { pumas_discovery: { source: 'anonymous-hf-detail-cache', discovery_only: true, visibility_observed: 'public-ungated', source_url: `https://huggingface.co/api/models/acme/${name}`, observed_at: '2026-01-01T00:00:00Z', fresh_until: '2026-01-01T01:00:00Z', freshness: 'stale', revision_observed: name === 'NoRevision' ? null : name === 'Beta' ? 'b'.repeat(40) : 'a'.repeat(40) } } };
}
async function search(query: string, kind: string | null, limit: number, hydrateLimit: number) {
  if (base) {
    const response = await fetch(`${base}/rpc`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, signal: AbortSignal.timeout(5000), body: JSON.stringify({ jsonrpc: '2.0', id: 321, method: 'search_hf_models', params: { query, kind, limit, hydrate_limit: hydrateLimit } }) });
    const body = await response.json() as { error?: unknown; id: number; result: { success: boolean; error?: string; models?: RemoteModelInfo[] } };
    if (body.error || body.id !== 321) throw new APIError('Actual RPC envelope failed', 'search_hf_models');
    return body.result;
  }
  if (query.startsWith('cache:')) {
    const tokens = query.slice(6).trim().toLowerCase().split(/\s+/).filter(Boolean);
    return { success: true, models: ['Alpha', 'Beta', 'NoRevision'].map(fixture).filter(m => tokens.every(token => `${m.repoId} multilingual`.toLowerCase().includes(token))) };
  }
  return query === 'online-fixture' ? { success: true, models: [{ ...fixture('OnlineFixture'), totalSizeBytes: 1048576 }] } : { success: false, error: 'Owned upstream unavailable' };
}
function mount() {
  render(<ModelManager modelGroups={[]} libraryLoadStatus="ready" starredModels={new Set()} excludedModels={new Set()} selectedAppId={null} onToggleStar={vi.fn()} onToggleLink={vi.fn()} onAddModels={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: 'Search Hugging Face models' }));
  fireEvent.click(screen.getByRole('radio', { name: 'Cached details (offline)' }));
}
function query(value: string) { fireEvent.change(screen.getByRole('textbox'), { target: { value } }); }
describe('mounted cached discovery workflow (actual loopback RPC when configured)', () => {
  beforeEach(() => { mocks.search.mockReset().mockImplementation(search); mocks.details.mockReset(); mocks.start.mockReset(); });
  afterEach(cleanup);
  it('shows saved provenance, excludes nonpublic rows and never offers new downloads', async () => {
    mount(); await screen.findByText('Beta');
    expect(screen.queryByText('Hidden')).not.toBeInTheDocument();
    expect(screen.getAllByText('Cached observation · stale').length).toBeGreaterThan(0);
    expect(screen.getByText('Source: https://huggingface.co/api/models/acme/Beta')).toBeInTheDocument();
    expect(screen.getByText('b'.repeat(40))).toBeInTheDocument();
    expect(screen.getByText('Not recorded')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /download options|load details|all files/i })).not.toBeInTheDocument();
    expect(screen.getAllByText(/Switch to Hugging Face search for current download details/)).toHaveLength(3);
    expect(mocks.search).toHaveBeenCalledWith('cache:', null, 25, 0);
    expect(mocks.details).not.toHaveBeenCalled(); expect(mocks.start).not.toHaveBeenCalled();
    query('Beta multilingual');
    await waitFor(() => expect(screen.getAllByLabelText('Cached discovery observation')).toHaveLength(1));
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText(/^Observed:/).querySelector('time')).toHaveAttribute('datetime');
    query('online-fixture'); fireEvent.click(screen.getByRole('radio', { name: 'Hugging Face search' }));
    expect(screen.queryByLabelText('Cached discovery observation')).not.toBeInTheDocument();
    await screen.findByText('OnlineFixture');
    expect(screen.queryByLabelText('Cached discovery observation')).not.toBeInTheDocument();
    expect(mocks.search).toHaveBeenLastCalledWith('online-fixture', null, 25, 0);
  });
  it('cancels queued queries and ignores a cached reply delivered after source switching', async () => {
    let deliver!: () => void; const held = new Promise<void>(resolve => { deliver = resolve; });
    let entered!: () => void; const fetched = new Promise<void>(resolve => { entered = resolve; });
    mocks.search.mockImplementation(async (...args: Parameters<typeof search>) => { const response = await search(...args); if (args[0] === 'cache:Beta') { entered(); await held; } return response; });
    mount(); query('will-be-cancelled'); query('Beta'); await fetched;
    expect(mocks.search.mock.calls.some(call => call[0] === 'cache:will-be-cancelled')).toBe(false);
    query('online-fixture'); fireEvent.click(screen.getByRole('radio', { name: 'Hugging Face search' }));
    await screen.findByText('OnlineFixture');
    await act(async () => { deliver(); await held; });
    expect(screen.queryByText('Beta')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Cached discovery observation')).not.toBeInTheDocument();
    expect(mocks.details).not.toHaveBeenCalled(); expect(mocks.start).not.toHaveBeenCalled();
  });
  it('keeps ordinary unavailable search an error until cached discovery is explicitly selected', async () => {
    mount(); await screen.findByText('Beta');
    query('owned-upstream-unavailable'); fireEvent.click(screen.getByRole('radio', { name: 'Hugging Face search' }));
    await waitFor(() => expect(screen.getByText(/Owned upstream unavailable|Failed to search|error sending request|Hugging Face search failed|A required operation is currently unavailable/i)).toBeInTheDocument());
    expect(screen.queryByText('Beta')).not.toBeInTheDocument();
    query('Beta'); fireEvent.click(screen.getByRole('radio', { name: 'Cached details (offline)' }));
    await screen.findByText('Beta'); expect(screen.getByText('Cached observation · stale')).toBeInTheDocument();
  });
});
