import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';
import { useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
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
