import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { useState } from 'react';
import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { LauncherRootRecoveryProvider } from '../src/hooks/useLauncherRootRecovery';
import { useModels } from '../src/hooks/useModels';
import { useModelLibraryActions } from '../src/hooks/useModelLibraryActions';
import { useModelDownloads } from '../src/hooks/useModelDownloads';
import { buildDownloadingModels, mergeLocalModelGroups } from '../src/components/ModelManagerUtils';
import { LocalModelsList } from '../src/components/LocalModelsList';
import { LinkHealthStatus } from '../src/components/LinkHealthStatus';
import { ValidationError } from '../src/errors';
import { useModelImportPicker } from '../src/hooks/useModelImportPicker';
import { chooseModelImportPaths } from '../../electron/src/model-import-picker';
import { useRemoteModelSearch } from '../src/hooks/useRemoteModelSearch';
import { useAvailableVersionState } from '../src/hooks/useAvailableVersionState';
import { ModelMetadataModal } from '../src/components/ModelMetadataModal';
import type { RemoteModelInfo } from '../src/types/apps';
import { decodeHfDownloadDetailsOutcome } from '../src/generated/desktop-contract';

const fixturePath = process.env['PUMAS_DESKTOP_CONTRACT_FIXTURES'];
if (!fixturePath) throw new ValidationError('Actual desktop producer fixtures are required; run test:desktop-contract.', 'producer-fixtures');
function isFixtureRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
function readFixture(path: string): Record<string, unknown> {
  const value: unknown = JSON.parse(readFileSync(path, 'utf8'));
  if (!isFixtureRecord(value)) {
    throw new ValidationError('Invalid producer fixture envelope.', 'producer-fixtures');
  }
  return value;
}
const fixture = readFixture(fixturePath);
const preload = readFileSync(resolve('../electron/dist/preload.js'), 'utf8');

function installActualPreload(
  recoveryOutcome = 'recovery_outcome',
  listeners = new Map<string, (event: unknown, payload: unknown) => void>(),
  initialDownloads: unknown = fixture['download_list'],
  healthOutcomes: unknown[] = [],
  picker: () => Promise<unknown> = async () => { throw new ValidationError('No picker fixture', 'producer-fixtures'); },
  conversionResponse: unknown = fixture['conversion_missing'],
  setupResponse: unknown = fixture['conversion_setup_idle'],
  hfReads?: { models: RemoteModelInfo[]; details: unknown },
  inferenceRead: unknown = fixture['inference_settings'],
  metadataRead: unknown = fixture['library_model_metadata_empty'],
  mutationResponses: { notes?: () => unknown; settings?: () => unknown } = {},
  availableVersions: () => unknown = () => fixture['available_versions'],
  githubCacheStatus: () => unknown = () => fixture['github_cache_status_no_manager'],
) {
  const requests: Array<{ method: string; params: unknown }> = [];
  const module = { exports: {} };
  const electron = {
    contextBridge: {
      exposeInMainWorld: (name: string, bridge: typeof window.electronAPI) => {
        expect(name).toBe('electronAPI');
        window.electronAPI = bridge;
      },
    },
    ipcRenderer: {
      on: (channel: string, listener: (event: unknown, payload: unknown) => void) => { listeners.set(channel, listener); },
      removeListener: (channel: string) => { listeners.delete(channel); },
      sendSync: (channel: string) => {
        expect(channel).toBe('launcher:getRootBootstrap');
        return { status: 'ready', selectionAction: 'select-library', libraryScopeId: null };
      },
      invoke: async (channel: string, method: string, params: unknown) => {
        if (channel === 'dialog:openFile') return picker();
        if (channel === 'launcher-root:presentation-committed') return undefined;
        if (channel === 'model-download:subscribe' || channel === 'model-download:unsubscribe') return undefined;
        expect(channel).toBe('api:call');
        const requestParams: unknown = JSON.parse(JSON.stringify(params));
        requests.push({ method, params: requestParams });
        if (method === 'get_available_versions') return availableVersions();
        if (method === 'get_github_cache_status') return githubCacheStatus();
        if (method === 'get_inference_settings') return inferenceRead;
        if (method === 'update_inference_settings') return mutationResponses.settings ? mutationResponses.settings() : fixture['update_inference_settings'];
        if (method === 'update_model_notes' && mutationResponses.notes) return mutationResponses.notes();
        if (method === 'update_model_notes' && typeof params === 'object' && params !== null && 'notes' in params) {
          return params.notes === null ? fixture['update_model_notes_clear'] : fixture['update_model_notes_text'];
        }
        if (method === 'get_library_model_metadata') return metadataRead;
        if (method === 'search_hf_models' && hfReads) return { success: true, models: hfReads.models };
        if (method === 'get_hf_download_details' && hfReads) return hfReads.details;
        if (method === 'get_conversion_progress') return conversionResponse;
        if (['get_conversion_setup', 'start_conversion_setup', 'get_backend_setup', 'start_backend_setup'].includes(method)) return setupResponse;
        if (method === 'list_model_conversions') return fixture['conversion_list'];
        const conversionOperations: Record<string, unknown> = {
          start_model_conversion: fixture['conversion_started'],
          setup_conversion_environment: fixture['conversion_setup_success'],
          setup_quantization_backend: fixture['conversion_setup_success'],
          get_supported_quant_types: fixture['conversion_quant_types'],
          get_backend_status: fixture['conversion_backend_status'],
        };
        if (Object.hasOwn(conversionOperations, method)) return conversionOperations[method];
        if (method === 'cancel_model_conversion' || method === 'check_conversion_environment') {
          const variants = fixture[method === 'cancel_model_conversion' ? 'conversion_cancelled' : 'conversion_environment'];
          if (!Array.isArray(variants)) throw new ValidationError('Missing operation variants','producer-fixtures');
          const first: unknown = variants[0];
          return first;
        }
        if (method === 'get_models') return fixture['models'];
        if (method === 'get_link_health') {
          if (healthOutcomes.length === 0) throw new ValidationError('No link-health fixture response remains.', 'producer-fixtures');
          return healthOutcomes.shift();
        }
        if (method === 'search_models_fts') return fixture['search'];
        if (method === 'resume_partial_download') return fixture[recoveryOutcome];
        if (method === 'list_model_downloads') return initialDownloads;
        if (method === 'resume_model_download' || method === 'pause_model_download' || method === 'cancel_model_download') return fixture['download_mutation'];
        throw new ValidationError(`Unprovided fixture operation: ${method}`, 'producer-fixtures');
      },
    },
    webUtils: { getPathForFile: () => '' },
  };
  runInNewContext(preload, {
    exports: module.exports, module,
    require: (name: string) => {
      expect(name).toBe('electron');
      return electron;
    },
  }, { filename: 'electron/dist/preload.js' });
  expect(window.electronAPI).toBeDefined();
  return requests;
}

type StartDownload = Parameters<typeof useModelLibraryActions>[0]['startDownload'];

function LiveDownloadLibrary() {
  const { modelGroups, libraryLoadStatus } = useModels();
  const downloads = useModelDownloads();
  const actions = useModelLibraryActions({ setDownloadErrors: downloads.setDownloadErrors, startDownload: downloads.startDownload });
  const merged = mergeLocalModelGroups(modelGroups, buildDownloadingModels(downloads.downloadStatusByRepo));
  return <>
    <div role="status">{libraryLoadStatus}</div>
    <LocalModelsList modelGroups={merged} totalModels={modelGroups.flatMap(group => group.models).length}
      starredModels={new Set()} excludedModels={new Set()} selectedAppId={null} hasFilters={false}
      onToggleStar={() => undefined} onToggleLink={() => undefined}
      relatedModelsById={actions.relatedModelsById} expandedRelated={actions.expandedRelated}
      onToggleRelated={actions.handleToggleRelated} onOpenRelatedUrl={actions.openRemoteUrl}
      onRecoverPartialDownload={actions.handleRecoverPartialDownload}
      onPauseDownload={downloads.pauseDownload} onResumeDownload={downloads.resumeDownload}
      onCancelDownload={downloads.cancelDownload} downloadErrors={downloads.downloadErrors}
    />
  </>;
}

function Library({ onStarted }: { onStarted: StartDownload }) {
  const { modelGroups, libraryLoadStatus } = useModels();
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});
  const actions = useModelLibraryActions({
    setDownloadErrors, startDownload: onStarted,
  });
  return <>
    <div role="status">{libraryLoadStatus}</div>
    <LocalModelsList
      modelGroups={modelGroups} totalModels={modelGroups.flatMap((group) => group.models).length}
      starredModels={new Set()} excludedModels={new Set()} selectedAppId={null} hasFilters={false}
      onToggleStar={() => undefined} onToggleLink={() => undefined}
      relatedModelsById={actions.relatedModelsById} expandedRelated={actions.expandedRelated}
      onToggleRelated={actions.handleToggleRelated} onOpenRelatedUrl={actions.openRemoteUrl}
      onRecoverPartialDownload={actions.handleRecoverPartialDownload}
      recoveringPartialModelIds={actions.recoveringPartialModelIds} downloadErrors={downloadErrors}
    />
  </>;
}

describe('actual Rust catalog through bundled preload and renderer', () => {
  it('polls producer cache snapshots and keeps the last valid snapshot on malformed responses', async () => {
    vi.useFakeTimers();
    let unmount: (() => void) | undefined;
    try {
      let response: unknown = fixture['github_cache_status_populated'];
      const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, () => response);
      const hook = renderHook(() => useAvailableVersionState({ isEnabled: true, resolvedAppId: 'ollama', trackAvailableVersions: true }));
      unmount = hook.unmount;
      await act(async () => { await vi.advanceTimersByTimeAsync(0); });
      expect(hook.result.current.cacheStatus).toEqual(response);
      expect(requests.at(-1)?.params).toEqual({ app_id: 'ollama' });
      for (const key of ['github_cache_status_empty', 'github_cache_status_fetching', 'github_cache_status_no_manager', 'github_cache_status_populated']) {
        response = fixture[key];
        await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
        expect(hook.result.current.cacheStatus).toEqual(response);
      }
      response = { has_cache: 'bad', is_valid: false, is_fetching: false };
      await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
      expect(hook.result.current.cacheStatus).toEqual(fixture['github_cache_status_populated']);
      expect(requests.filter(request => request.method === 'get_github_cache_status')).toHaveLength(6);
    } finally {
      unmount?.();
      vi.useRealTimers();
    }
  });

  it('consumes producer releases and rate limits without reading absent versions or dropping valid rows', async () => {
    let response: unknown = fixture['available_versions'];
    const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, () => response);
    const { result } = renderHook(() => useAvailableVersionState({ isEnabled: true, resolvedAppId: 'ollama', trackAvailableVersions: false }));
    await act(async () => { await result.current.fetchAvailableVersions(false); });
    const initial = result.current.availableVersions;
    expect(initial.length).toBeGreaterThan(0);
    expect(requests.at(-1)?.params).toEqual({ force_refresh: false, app_id: 'ollama' });
    const producer = fixture['available_versions'];
    if (!isFixtureRecord(producer) || !Array.isArray(producer['versions'])) throw new ValidationError('Missing releases', 'producer-fixtures');
    expect(initial).toEqual(producer['versions'].map((version: unknown) => {
      if (!isFixtureRecord(version)) throw new ValidationError('Invalid release', 'producer-fixtures');
      return { ...version, body: version['body'] ?? undefined, installing: version['installing'] ?? false };
    }));
    for (const key of ['available_versions_rate_limited', 'available_versions_rate_limited_unknown']) {
      response = fixture[key];
      await act(async () => { await result.current.fetchAvailableVersions(false); });
      expect(result.current.isRateLimited).toBe(true);
      expect(result.current.availableVersions).toEqual(initial);
      if (!isFixtureRecord(response)) throw new ValidationError('Missing rate limit', 'producer-fixtures');
      expect(result.current.rateLimitRetryAfter).toBe(response['retry_after_secs']);
    }
    response = { success: true, versions: [{}] };
    await act(async () => { await expect(result.current.fetchAvailableVersions(false)).rejects.toThrow(/Desktop contract/); });
    expect(result.current.availableVersions).toEqual(initial);
    response = fixture['available_versions_empty'];
    await act(async () => { await result.current.fetchAvailableVersions(false); });
    expect(result.current.availableVersions).toEqual([]);
    expect(result.current.isRateLimited).toBe(false);
    expect(result.current.rateLimitRetryAfter).toBeNull();
    expect(requests.filter(request => request.method === 'get_available_versions')).toHaveLength(5);
  });

  it.each(['malformed', 'wrong-model', 'transport', 'missing'] as const)('preserves notes draft on %s save response without retry', async (kind) => {
    const response = () => {
      if (kind === 'transport') throw new ValidationError('private transport detail', 'producer-fixtures');
      if (kind === 'missing') return fixture['update_model_notes_missing'];
      if (kind === 'wrong-model') return { success: true, model_id: 'other', notes: 'wrong notes' };
      return { success: true, model_id: 'llm/Exact Model', notes: 42 };
    };
    const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, { notes: response });
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Notes fixture" onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Notes' }));
    const editor = await screen.findByRole('textbox');
    fireEvent.change(editor, { target: { value: ' My unsaved λ draft ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save Notes' }));
    await screen.findByText(kind === 'missing' ? /Notes were not saved/ : /Save could not be confirmed/);
    expect(editor).toHaveValue(' My unsaved λ draft ');
    expect(screen.queryByText('Saved')).not.toBeInTheDocument();
    expect(screen.queryByText(/private transport detail/)).not.toBeInTheDocument();
    expect(requests.filter(request => request.method === 'update_model_notes')).toHaveLength(1);
  });

  it.each(['malformed', 'wrong-model', 'transport'] as const)('preserves settings draft on %s save response without retry', async (kind) => {
    const response = () => {
      if (kind === 'transport') throw new ValidationError('private transport detail', 'producer-fixtures');
      return kind === 'wrong-model' ? { success: true, model_id: 'other' } : { success: 'true', model_id: 'llm/Exact Model' };
    };
    const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, { settings: response });
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Settings fixture" onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Inference' }));
    const numeric = screen.getAllByRole('spinbutton')[0];
    if (!numeric) throw new ValidationError('Missing inference editor', 'producer-fixtures');
    fireEvent.change(numeric, { target: { value: '2048' } });
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));
    await screen.findByText(/Save could not be confirmed/);
    expect(numeric).toHaveValue(2048);
    expect(screen.queryByText('Saved')).not.toBeInTheDocument();
    expect(screen.queryByText(/private transport detail/)).not.toBeInTheDocument();
    expect(requests.filter(request => request.method === 'update_inference_settings')).toHaveLength(1);
  });

  it('submits exact notes and intentional clears through bundled preload', async () => {
    const requests = installActualPreload();
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Notes fixture" onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Notes' }));
    const editor = await screen.findByRole('textbox');
    const exact = '  # Exact λ\n\n**notes**  ';
    fireEvent.change(editor, { target: { value: exact } });
    fireEvent.click(screen.getByRole('button', { name: 'Save Notes' }));
    await screen.findByText('Saved');
    expect(requests.filter(request => request.method === 'update_model_notes').at(-1)?.params)
      .toEqual({ model_id: 'llm/Exact Model', notes: exact });
    fireEvent.change(editor, { target: { value: ' \n\t ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save Notes' }));
    await waitFor(() => expect(requests.filter(request => request.method === 'update_model_notes')).toHaveLength(2));
    expect(requests.filter(request => request.method === 'update_model_notes').at(-1)?.params)
      .toEqual({ model_id: 'llm/Exact Model', notes: null });
    await waitFor(() => expect(editor).toHaveValue(''));
  });

  it('renders actual metadata payloads and all manifest states through bundled preload', async () => {
    const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined,
      fixture['library_model_metadata']);
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Metadata fixture" onClose={() => undefined} />);
    expect(await screen.findByText('exact')).toBeInTheDocument();
    expect(screen.getByText('/Exact Library/λ/model.safetensors')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Components \(4\)/ }));
    for (const state of ['Present', 'Missing', 'Unreadable', 'Invalid Path']) {
      expect(screen.getByText(state)).toBeInTheDocument();
    }
    fireEvent.click(screen.getByRole('button', { name: 'Stored' }));
    expect(screen.getByText('Array (4)')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Expand' }));
    expect(screen.getByText(/"edge": 9007199254740991/)).toBeInTheDocument();
    expect(requests.find(request => request.method === 'get_library_model_metadata')?.params)
      .toEqual({ model_id: 'llm/Exact Model' });
  });

  it('renders decoded GGUF objects without coercing a structured URL into a link', async () => {
    installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined,
      fixture['library_model_metadata_gguf']);
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="GGUF fixture" onClose={() => undefined} />);
    expect(await screen.findByText('Exact GGUF')).toBeInTheDocument();
    expect(screen.getByText('{"nested":[null,{"value":"λ"}]}')).toBeInTheDocument();
    expect(screen.getByText('Exact base')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Exact base' })).not.toBeInTheDocument();
  });

  it.each(['invalid-shape', 'wrong-model'] as const)('does not display %s metadata from preload', async (kind) => {
    const metadata = kind === 'invalid-shape'
      ? { success: true, model_id: 'llm/Exact Model', stored_metadata: [] }
      : { success: true, model_id: 'different-model', stored_metadata: { name: 'Wrong content' } };
    installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined, metadata);
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Metadata fixture" onClose={() => undefined} />);
    expect(await screen.findByText('Failed to load metadata')).toBeInTheDocument();
    expect(screen.queryByText('Wrong content')).not.toBeInTheDocument();
  });

  it('renders producer inference settings through the bundled preload and preserves editable drafts', async () => {
    const requests = installActualPreload();
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Fixture" onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Inference' }));
    expect(await screen.findByText('Label 0 λ')).toBeInTheDocument();
    expect(screen.getByText('Label 3 λ')).toBeInTheDocument();
    expect(screen.getAllByText('Structured default (read-only)')).toHaveLength(3);
    expect(requests.find(request => request.method === 'get_inference_settings')?.params).toEqual({ model_id: 'llm/Exact Model' });
    const numeric = screen.getAllByRole('spinbutton')[0];
    if (!numeric) throw new ValidationError('Missing inference editor', 'producer-fixtures');
    fireEvent.change(numeric, { target: { value: '2048' } });
    expect(numeric).toHaveValue(2048);
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));
    await waitFor(() => expect(requests.some(request => request.method === 'update_inference_settings')).toBe(true));
    expect(await screen.findByText('Saved')).toBeInTheDocument();
    const updated = requests.find(request => request.method === 'update_inference_settings')?.params;
    expect(updated).toMatchObject({ model_id: 'llm/Exact Model', settings: [
      { key: ' Exact key 0 ', default: { nested: [null, true, ' λ ', 0.25, { edge: 9007199254740991 }] } },
      { key: ' Exact key 1 ', default: 2048 },
      { key: ' Exact key 2 ' }, { key: ' Exact key 3 ' },
    ] });
  });

  it('shows malformed producer settings as unavailable without an editable empty list', async () => {
    installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, undefined, undefined,
      { success: true, model_id: 'llm/Exact Model', inference_settings: [{ key: 'incomplete' }] });
    render(<ModelMetadataModal modelId="llm/Exact Model" modelName="Fixture" onClose={() => undefined} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Inference' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unable to load inference settings');
    expect(screen.queryByRole('button', { name: /Save/ })).not.toBeInTheDocument();
  });

  it('hydrates exact producer download details through bundled preload and the real search hook', async () => {
    const produced = fixture['hf_download_details_success'];
    const decoded = decodeHfDownloadDetailsOutcome(produced);
    if (decoded.status !== 'valid' || !decoded.value.success) {
      throw new ValidationError('Missing successful HF producer fixture', 'producer-fixtures');
    }
    const details = decoded.value.details;
    const model: RemoteModelInfo = {
      repoId: details.repoId, name: 'Fixture model', developer: 'fixture',
      kind: 'text-generation', formats: ['gguf'], quants: ['Q4_K_M'],
      url: 'https://huggingface.co/fixture/model',
    };
    const requests = installActualPreload(undefined, undefined, undefined, undefined,
      undefined, undefined, undefined, { models: [model], details: produced });
    const { result } = renderHook(() => useRemoteModelSearch({ enabled: true, searchQuery: 'fixture', debounceMs: 0 }));
    await waitFor(() => expect(result.current.results).toHaveLength(1));
    await act(async () => { await result.current.hydrateModelDetails(model); });
    expect(result.current.results[0]?.repoId).toBe(details.repoId);
    expect(result.current.results[0]?.downloadOptions).toEqual(details.downloadOptions);
    expect(result.current.results[0]?.totalSizeBytes).toBe(details.totalSizeBytes);
    expect(requests.find(request => request.method === 'get_hf_download_details')?.params)
      .toEqual({ repo_id: details.repoId, quants: ['Q4_K_M'] });
  });

  it('does not project malformed download details received through the bundled preload', async () => {
    const model: RemoteModelInfo = {
      repoId: 'fixture/model', name: 'Fixture model', developer: 'fixture',
      kind: 'text-generation', formats: ['gguf'], quants: ['Q4_K_M'],
      url: 'https://huggingface.co/fixture/model', totalSizeBytes: null,
    };
    installActualPreload(undefined, undefined, undefined, undefined,
      undefined, undefined, undefined, { models: [model], details: {
        success: true, details: { repoId: model.repoId,
          downloadOptions: [{ quant: 'Q4_K_M', sizeBytes: -1 }], totalSizeBytes: null },
      } });
    const { result } = renderHook(() => useRemoteModelSearch({ enabled: true, searchQuery: 'fixture', debounceMs: 0 }));
    await waitFor(() => expect(result.current.results).toHaveLength(1));
    await act(async () => { await result.current.hydrateModelDetails(model); });
    expect(result.current.results).toEqual([model]);
    expect(result.current.hydratingRepoIds.size).toBe(0);
  });

  it('preserves setup identity, terminal states and retry tokens through the bundled preload', async () => {
    const snapshots = fixture['conversion_setup_started'];
    if (!Array.isArray(snapshots)) throw new ValidationError('Missing setup snapshots', 'producer-fixtures');
    for (const snapshot of snapshots) {
      const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, snapshot);
      const bridge = window.electronAPI;
      if (!bridge) throw new ValidationError('Missing preload bridge', 'producer-fixtures');
      const started = await bridge.start_conversion_setup();
      expect(started).toEqual(snapshot);
      expect(requests[0]).toEqual({method:'start_conversion_setup',params:{expected_previous_operation_id:null}});
      const previous = started.setup.operationId;
      expect(await bridge.start_conversion_setup(previous)).toEqual(snapshot);
      expect(requests[1]).toEqual({method:'start_conversion_setup',params:{expected_previous_operation_id:previous}});
      await bridge.start_conversion_setup(null);
      expect(requests[2]).toEqual({method:'start_conversion_setup',params:{expected_previous_operation_id:null}});
      expect(await bridge.get_conversion_setup()).toEqual(snapshot);
      expect(requests[3]).toEqual({method:'get_conversion_setup',params:{}});
    }
    installActualPreload();
    expect(await window.electronAPI?.get_conversion_setup()).toEqual({success:true,setup:null});
    await expect(window.electronAPI?.start_conversion_setup()).rejects.toMatchObject({status:'invalid'});
  });

  it('preserves backend selection and all producer setup states for typed standalone consumers', async () => {
    const snapshots = fixture['conversion_setup_started'];
    if (!Array.isArray(snapshots)) throw new ValidationError('Missing setup snapshots', 'producer-fixtures');
    for (const backend of ['python_conversion', 'llama_cpp', 'nvfp4', 'sherry'] as const) {
      for (const snapshot of snapshots) {
        const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, snapshot);
        const bridge = window.electronAPI;
        if (!bridge) throw new ValidationError('Missing compiled bridge', 'producer-fixtures');
        const started = await bridge.start_backend_setup(backend);
        expect(started).toEqual(snapshot);
        expect(await bridge.get_backend_setup(backend)).toEqual(snapshot);
        expect(await bridge.start_backend_setup(backend, started.setup.operationId)).toEqual(snapshot);
        expect(requests).toEqual([
          {method:'start_backend_setup', params:{backend, expected_previous_operation_id:null}},
          {method:'get_backend_setup', params:{backend}},
          {method:'start_backend_setup', params:{backend, expected_previous_operation_id:started.setup.operationId}},
        ]);
      }
    }
    installActualPreload();
    expect(await window.electronAPI?.get_backend_setup('sherry')).toEqual({success:true, setup:null});
  });

  it('rejects malformed setup state before exposing it to typed consumers', async () => {
    const malformed = {success:true,setup:{operationId:'2e038924-e0e3-4266-95ef-f7a02997b7b6',status:'failed',error:null}};
    installActualPreload(undefined, undefined, undefined, undefined, undefined, undefined, malformed);
    await expect(window.electronAPI?.get_conversion_setup()).rejects.toMatchObject({status:'invalid'});
    await expect(window.electronAPI?.start_conversion_setup()).rejects.toMatchObject({status:'invalid'});
  });

  it('forwards all conversion options and preserves operation outcomes through the bundled preload', async () => {
    const requests = installActualPreload();
    const bridge = window.electronAPI;
    if (!bridge) throw new ValidationError('Missing preload bridge','producer-fixtures');
    expect(await bridge.start_model_conversion('llm/example/model','gguf_to_quantized_gguf','Q4_K_M',null,'/fixture/calibration text.txt',false)).toEqual(fixture['conversion_started']);
    expect(requests[0]).toEqual({method:'start_model_conversion',params:{model_id:'llm/example/model',direction:'gguf_to_quantized_gguf',target_quant:'Q4_K_M',output_name:null,imatrix_calibration_file:'/fixture/calibration text.txt',force_imatrix:false}});
    await bridge.start_model_conversion('llm/example/model','gguf_to_safetensors');
    expect(requests[1]).toEqual({method:'start_model_conversion',params:{model_id:'llm/example/model',direction:'gguf_to_safetensors'}});
    expect((await bridge.cancel_model_conversion('missing')).cancelled).toBe(false);
    expect((await bridge.check_conversion_environment()).ready).toBe(false);
    expect(await bridge.setup_conversion_environment()).toEqual(fixture['conversion_setup_success']);
    expect(await bridge.get_supported_quant_types()).toEqual(fixture['conversion_quant_types']);
    expect(await bridge.get_backend_status()).toEqual(fixture['conversion_backend_status']);
    expect(await bridge.setup_quantization_backend('llama_cpp')).toEqual(fixture['conversion_setup_success']);
    expect(requests.at(-1)).toEqual({method:'setup_quantization_backend',params:{backend:'llama_cpp'}});
  });
  it('exposes canonical conversion progress to typed renderer callers without a GUI-owned conversion service', async () => {
    const source = fixture['conversion_progress'];
    if (!Array.isArray(source)) throw new ValidationError('Missing conversion fixture', 'producer-fixtures');
    const cases: unknown[] = source;
    for (const response of cases) {
      if (!isFixtureRecord(response) || !isFixtureRecord(response['progress'])) throw new ValidationError('Invalid progress fixture', 'producer-fixtures');
      const expected = response['progress'];
      const requests = installActualPreload(undefined, undefined, undefined, undefined, undefined, response);
      const bridge = window.electronAPI;
      if (!bridge) throw new ValidationError('Missing preload bridge', 'producer-fixtures');
      const result = await bridge.get_conversion_progress(String(expected['conversionId']));
      expect(result).toEqual(response);
      expect(result.progress?.conversionId).toBe(expected['conversionId']);
      expect(result.progress?.pipelineStepLabel).toBeNull();
      expect(requests[0]).toEqual({method:'get_conversion_progress',params:{conversion_id:expected['conversionId']}});
    }
    installActualPreload();
    const bridge = window.electronAPI;
    if (!bridge) throw new ValidationError('Missing preload bridge', 'producer-fixtures');
    expect(await bridge.get_conversion_progress('not-found')).toEqual({success:true,progress:null});
    expect(await bridge.list_model_conversions()).toEqual(fixture['conversion_list']);
    installActualPreload(undefined, undefined, undefined, undefined, undefined, {success:true,progress:{conversion_id:'old-shape'}});
    await expect(window.electronAPI?.get_conversion_progress('id')).rejects.toMatchObject({status:'invalid'});
  });
  it('preserves native selection, cancellation and failure through the actual preload into picker state', async () => {
    const paths = ['/models/ e\u0301.gguf', '/models/ e\u0301.gguf'];
    const chooser = vi.fn()
      .mockRejectedValueOnce(new Error('/private/native-error'))
      .mockResolvedValueOnce({ canceled: true, filePaths: [] })
      .mockResolvedValueOnce({ canceled: false, filePaths: paths });
    installActualPreload(undefined, undefined, undefined, undefined, () => chooseModelImportPaths(chooser));
    function Picker() {
      const state = useModelImportPicker({});
      return <><button onClick={() => void state.openImportPicker()}>Choose</button>
        <output>{JSON.stringify({paths:state.importPaths, open:state.showImportDialog, error:state.pickerError})}</output></>;
    }
    render(<Picker />);
    fireEvent.click(screen.getByRole('button', {name:'Choose'}));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Model file picker unavailable. Try again.'));
    expect(screen.getByRole('status')).not.toHaveTextContent('/private');
    fireEvent.click(screen.getByRole('button', {name:'Choose'}));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('"error":null'));
    expect(screen.getByRole('status')).toHaveTextContent('"open":false');
    fireEvent.click(screen.getByRole('button', {name:'Choose'}));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('"open":true'));
    const observed: unknown = JSON.parse(screen.getByRole('status').textContent ?? '{}');
    if (!isFixtureRecord(observed)) throw new ValidationError('Invalid observed state', 'producer-fixtures');
    expect(observed['paths']).toEqual(paths);
  });
  afterEach(() => { window.electronAPI = undefined; });

  it('renders complete, partial, and duplicate facts and sends the exact producer-admitted recovery ticket', async () => {
    const requests = installActualPreload();
    const onStarted = vi.fn<StartDownload>();
    render(<LauncherRootRecoveryProvider><Library onStarted={onStarted} /></LauncherRootRecoveryProvider>);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('ready'));
    expect(screen.getByText('complete', { exact: true })).toBeVisible();
    expect(screen.getByText('partial', { exact: true })).toBeVisible();
    expect(screen.getByText('PARTIAL 50%')).toBeVisible();
    expect(screen.getAllByText('ISSUE')).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Show related' })).toBeEnabled();
    const resume = screen.getByRole('button', { name: 'Resume partial download (50%)' });
    fireEvent.click(resume);
    await waitFor(() => expect(onStarted).toHaveBeenCalledOnce());
    expect(requests.find((request) => request.method === 'resume_partial_download')?.params)
      .toEqual(fixture['recovery_request']);
    expect(onStarted).toHaveBeenCalledWith('fixture-download', 'fixture-download', {
      libraryModelId: 'llm/example/partial', modelName: 'partial', modelType: 'llm', repoId: 'example/model', selectedArtifactId: 'example/model::Q4',
    });
  });

  it('keeps one catalog row from recovery admission through decoded paused updates and exact controls', async () => {
    const listeners = new Map<string, (event: unknown, payload: unknown) => void>();
    const requests = installActualPreload('recovery_outcome', listeners, { success: true, downloads: [] });
    render(<LauncherRootRecoveryProvider><LiveDownloadLibrary /></LauncherRootRecoveryProvider>);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('ready'));
    expect(screen.getAllByText('partial', { exact: true })).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Resume partial download (50%)' }));
    await waitFor(() => expect(screen.getByTitle('Pause download')).toBeVisible());
    expect(screen.getAllByText('partial', { exact: true })).toHaveLength(1);
    await act(async () => { listeners.get('model-download:update')?.({}, fixture['download_push']); });
    expect(screen.getAllByText('partial', { exact: true })).toHaveLength(1);
    expect(screen.getByText('PARTIAL 50%')).toBeVisible();
    expect(screen.queryByText('Download activity · paused')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Resume download'));
    await waitFor(() => expect(requests.find(request => request.method === 'resume_model_download')?.params)
      .toEqual({ download_id: 'fixture-download' }));
  });

  it('restores associated activity as one row and rejects an invalid pushed library identity', async () => {
    const listeners = new Map<string, (event: unknown, payload: unknown) => void>();
    installActualPreload('recovery_outcome', listeners);
    render(<LauncherRootRecoveryProvider><LiveDownloadLibrary /></LauncherRootRecoveryProvider>);
    await waitFor(() => expect(screen.getByTitle('Resume download')).toBeVisible());
    expect(screen.getAllByText('partial', { exact: true })).toHaveLength(1);
    const invalid = structuredClone(fixture['download_push']);
    if (!isFixtureRecord(invalid) || !isFixtureRecord(invalid['snapshot']) || !Array.isArray(invalid['snapshot']['downloads'])) throw new ValidationError('Invalid producer fixture', 'producer-fixtures');
    const entry: unknown = invalid['snapshot']['downloads'][0];
    if (!isFixtureRecord(entry)) throw new ValidationError('Invalid producer download', 'producer-fixtures');
    entry['libraryModelId'] = '../outside';
    entry['progress'] = 0.9;
    await act(async () => { listeners.get('model-download:update')?.({}, invalid); });
    expect(screen.getAllByText('partial', { exact: true })).toHaveLength(1);
    expect(screen.getByText('PARTIAL 50%')).toBeVisible();
  });

  it('shows the bounded busy outcome without starting a download', async () => {
    const requests = installActualPreload('recovery_busy_outcome');
    const onStarted = vi.fn<StartDownload>();
    render(<LauncherRootRecoveryProvider><Library onStarted={onStarted} /></LauncherRootRecoveryProvider>);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('ready'));
    fireEvent.click(screen.getByRole('button', { name: 'Resume partial download (50%)' }));
    await waitFor(() => expect(screen.getByText('The partial download could not be resumed.')).toBeVisible());
    expect(onStarted).not.toHaveBeenCalled();
    expect(requests.find((request) => request.method === 'resume_partial_download')?.params)
      .toEqual(fixture['recovery_request']);
  });

  it('accepts the same typed catalog projection from the actual search producer', async () => {
    installActualPreload();
    const bridge = window.electronAPI;
    if (!bridge) throw new ValidationError('Preload did not expose its bridge.', 'preload');
    const response = await bridge.search_models_fts('', 100, 0);
    expect(response.success).toBe(true);
    expect(response.models).toHaveLength(4);
    expect(response.models.find((model) => model.id === 'llm/example/partial')?.artifact.state).toBe('partial');
  });

  it('rejects a contradictory producer report before UI use and recovers through visible retry', async () => {
    const healthy = fixture['link_health_healthy'];
    if (!isFixtureRecord(healthy)) throw new ValidationError('Missing link-health fixture.', 'producer-fixtures');
    const requests = installActualPreload(undefined, undefined, undefined, [
      { ...healthy, status: 'degraded' }, healthy,
    ]);
    render(<LinkHealthStatus />);
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Link health unavailable'));
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry link health' }));
    await waitFor(() => expect(screen.getByText('All links healthy')).toBeVisible());
    expect(requests.filter(request => request.method === 'get_link_health')).toHaveLength(2);
  });

  it('renders real registered-link degradation through the bundled preload', async () => {
    installActualPreload(undefined, undefined, undefined, [fixture['link_health_degraded']]);
    render(<LinkHealthStatus />);
    await waitFor(() => expect(screen.getByText('Issues detected')).toBeVisible());
    expect(screen.queryByText('All links healthy')).not.toBeInTheDocument();
  });
});
