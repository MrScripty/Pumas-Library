import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelLibraryUpdateNotification } from '../types/api';
import type { CatalogModel } from '../generated/desktop-contract';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import { useModels } from '../hooks/useModels';
import { ModelManager } from './ModelManager';
import { writeModelLibrarySnapshot } from '../utils/modelLibrarySnapshot';
import { LauncherRootRecoveryProvider } from '../hooks/useLauncherRootRecovery';
import userEvent from '@testing-library/user-event';

const libraryScopeId = `display-v1:${'a'.repeat(64)}`;

const {
  getElectronAPIMock,
  getModelsMock,
  isApiAvailableMock,
  downloadActivities,
  resumeDownloadMock,
  pickerState,
  retryPickerMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  getModelsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  downloadActivities: {} as Record<string, DownloadStatus>,
  resumeDownloadMock: vi.fn(),
  pickerState: { error: null as string | null, busy: false },
  retryPickerMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
  isAPIAvailable: isApiAvailableMock,
}));

vi.mock('../api/models', () => ({
  modelsAPI: {
    getModels: getModelsMock,
    scanSharedStorage: vi.fn(),
  },
}));

vi.mock('../api/import', () => ({
  importAPI: {
    searchModelsFTS: vi.fn(),
  },
}));

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
    downloadStatusByRepo: downloadActivities,
    hasActiveDownloads: false,
    pauseDownload: vi.fn(),
    resumeDownload: resumeDownloadMock,
    setDownloadErrors: vi.fn(),
    startDownload: vi.fn(),
  }),
}));

vi.mock('../hooks/useModelImportPicker', () => ({
  useModelImportPicker: () => ({
    closeImportDialog: vi.fn(),
    completeImport: vi.fn(),
    importPaths: [],
    openImportPicker: retryPickerMock,
    pickerError: pickerState.error,
    isPicking: pickerState.busy,
    showImportDialog: false,
  }),
}));

vi.mock('../hooks/useModelLibraryActions', () => ({
  useModelLibraryActions: () => ({
    expandedRelated: new Set<string>(),
    handleConvertModel: vi.fn(),
    handleDeleteModel: vi.fn(),
    handleRecoverPartialDownload: vi.fn(),
    handleToggleRelated: vi.fn(),
    openRemoteUrl: vi.fn(),
    recoveringPartialModelIds: new Set<string>(),
    relatedModelsById: {},
  }),
}));

vi.mock('../hooks/useModelManagerFilters', () => ({
  useModelManagerFilters: () => ({
    clearLocalFilters: vi.fn(),
    clearRemoteFilters: vi.fn(),
    hasLocalFilters: false,
    isCategoryFiltered: false,
    isDownloadMode: false,
    searchDeveloper: vi.fn(),
    searchQuery: '',
    selectedCategory: 'all',
    selectedFilter: 'all',
    selectedKind: 'all',
    selectFilter: vi.fn(),
    setSearchQuery: vi.fn(),
    showCategoryMenu: false,
    toggleFilterMenu: vi.fn(),
    toggleMode: vi.fn(),
  }),
}));

vi.mock('../hooks/useNetworkStatus', () => ({
  useNetworkStatus: () => ({
    circuitBreakerRejections: 0,
    isOffline: false,
    isRateLimited: false,
    successRate: 1,
  }),
}));

vi.mock('../hooks/useRemoteModelSearch', () => ({
  useRemoteModelSearch: () => ({
    error: null,
    hydrateModelDetails: vi.fn(),
    hydratingRepoIds: new Set<string>(),
    isLoading: false,
    kinds: [],
    results: [],
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

vi.mock('./NetworkStatusBanner', () => ({
  NetworkStatusBanner: () => null,
}));

vi.mock('./RemoteModelsList', () => ({
  RemoteModelsList: () => null,
}));

function makeRecord(id: string, hasIntegrityIssue: boolean): CatalogModel {
  return {
    id,
    modelDir: `/models/${id}`,
    modelType: 'llm',
    displayName: 'Qwen Test',
    dependencyCount: 0,
    relatedAvailable: false,
    artifact: { state: 'complete' },
    integrity: hasIntegrityIssue ? { state: 'duplicate', count: 2, otherModelIds: ['other'] } : { state: 'clean' },
  };
}

function Harness() {
  getElectronAPIMock.mockReturnValue({
    get_launcher_root_bootstrap: () => ({ status: 'ready', selectionAction: 'select-library', libraryScopeId }),
    notify_launcher_root_presentation_committed: async () => undefined,
    onLauncherRootPresentationTimeout: () => () => undefined,
    ...getElectronAPIMock(),
  });
  return <LauncherRootRecoveryProvider><HarnessContent /></LauncherRootRecoveryProvider>;
}

function HarnessContent() {
  const { modelGroups, libraryLoadStatus } = useModels();

  return (
    <ModelManager
      modelGroups={modelGroups}
      libraryLoadStatus={libraryLoadStatus}
      starredModels={new Set()}
      excludedModels={new Set()}
      onToggleStar={vi.fn()}
      onToggleLink={vi.fn()}
      selectedAppId="ollama"
      onChooseExistingLibrary={vi.fn()}
    />
  );
}

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('ModelManager integrity refresh acceptance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    pickerState.error = null;
    pickerState.busy = false;
    for (const key of Object.keys(downloadActivities)) delete downloadActivities[key];
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    localStorage.clear();
    getElectronAPIMock.mockReturnValue(null);
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('shows picker failure with keyboard retry and disables import while choosing', async () => {
    vi.useRealTimers();
    getModelsMock.mockResolvedValue({ success: true, models: {} });
    pickerState.error = 'Model file picker unavailable. Try again.';
    const { rerender } = render(<Harness />);
    expect(screen.getByRole('alert')).toHaveTextContent(pickerState.error);
    const retry = screen.getByRole('button', { name: 'Retry model selection' });
    retry.focus();
    await userEvent.keyboard('{Enter}');
    expect(retryPickerMock).toHaveBeenCalledTimes(1);
    pickerState.error = null;
    pickerState.busy = true;
    rerender(<Harness />);
    expect(screen.getByRole('button', { name: 'Import models' })).toBeDisabled();
    expect(screen.getByText('Choosing model files…')).toHaveAttribute('role', 'status');
    expect(screen.queryByRole('button', { name: 'Retry model selection' })).not.toBeInTheDocument();
  });

  it('counts catalog models separately from visibly identified download activity with exact controls', async () => {
    getModelsMock.mockResolvedValue({ success: true, models: { qwen: makeRecord('qwen', false) } });
    downloadActivities['download-exact-id'] = {
      downloadId: 'download-exact-id',
      status: 'paused', repoId: 'publisher/Qwen', modelName: 'Qwen Test',
      progress: 0.25,
    };
    render(<Harness />);
    await flushMicrotasks();

    expect(screen.getByPlaceholderText('Search 1 models')).toBeInTheDocument();
    expect(screen.getAllByText('Qwen Test')).toHaveLength(2);
    expect(screen.getByText('Download activity · paused')).toBeVisible();
    fireEvent.click(screen.getByTitle('Resume download'));
    expect(resumeDownloadMock).toHaveBeenCalledExactlyOnceWith('download-exact-id');
  });

  it('renders an associated paused download once with the catalog name and exact resume control', async () => {
    getModelsMock.mockResolvedValue({ success: true, models: { qwen: makeRecord('qwen', true) } });
    downloadActivities['download-exact-id'] = {
      downloadId: 'download-exact-id', libraryModelId: 'qwen',
      status: 'paused', repoId: 'publisher/Qwen', modelName: 'Qwen Test',
      progress: 0.25,
    };
    render(<Harness />);
    await flushMicrotasks();

    expect(screen.getByPlaceholderText('Search 1 models')).toBeInTheDocument();
    expect(screen.getAllByText('Qwen Test')).toHaveLength(1);
    expect(screen.queryByText('Download activity · paused')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Resume download'));
    expect(resumeDownloadMock).toHaveBeenCalledExactlyOnceWith('download-exact-id');
  });

  it('shows loading rather than an empty library or a zero count before the first response', () => {
    getModelsMock.mockReturnValue(new Promise(() => {}));
    render(<Harness />);

    expect(screen.getByRole('status')).toHaveTextContent('Loading library');
    expect(screen.queryByText('No library models found')).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText('Search 0 models')).not.toBeInTheDocument();
    expect(screen.getByPlaceholderText('Search library models')).toBeInTheDocument();
  });

  it.each(['rejected', 'unsuccessful', 'unavailable'] as const)(
    'does not claim an empty library when the backend is %s',
    async (failure) => {
      if (failure === 'rejected') getModelsMock.mockRejectedValue(new Error('Backend startup failed'));
      if (failure === 'unsuccessful') getModelsMock.mockRejectedValue(new Error('Invalid desktop catalog response'));
      if (failure === 'unavailable') isApiAvailableMock.mockReturnValue(false);
      render(<Harness />);
      await flushMicrotasks();

      expect(screen.getByRole('alert')).toHaveTextContent('Library unavailable');
      expect(screen.queryByText('No library models found')).not.toBeInTheDocument();
      expect(screen.queryByPlaceholderText('Search 0 models')).not.toBeInTheDocument();
    },
  );

  it('still displays the genuine empty-library state after a successful empty response', async () => {
    getModelsMock.mockResolvedValue({ success: true, models: {} });
    render(<Harness />);
    await flushMicrotasks();

    expect(screen.getByText('No library models found')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Search 0 models')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('keeps saved partial models visible while clearly identifying a failed refresh', async () => {
    writeModelLibrarySnapshot([{
      category: 'llm',
      models: [{ id: 'saved-partial', name: 'Saved partial model', category: 'llm', isPartialDownload: true }],
    }], libraryScopeId);
    getModelsMock.mockRejectedValue(new Error('Backend startup failed'));
    render(<Harness />);
    expect(screen.getByText('Saved partial model')).toBeInTheDocument();
    await flushMicrotasks();

    expect(screen.getByText('Saved partial model')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Showing previously loaded models');
    expect(screen.getByText('PARTIAL')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Search library models')).toBeInTheDocument();
    expect(screen.queryByText('No library models found')).not.toBeInTheDocument();
  });

  it('clears the integrity header and ISSUE badge after backend-pushed refresh returns clean model data', async () => {
    let notifyModelLibraryUpdate: ((notification: unknown) => void) | null = null;

    getElectronAPIMock.mockReturnValue({
      onModelLibraryUpdate: vi.fn((callback: (notification: ModelLibraryUpdateNotification) => void) => {
        notifyModelLibraryUpdate = (notification) =>
          callback(notification as ModelLibraryUpdateNotification);
        return vi.fn();
      }),
    });
    getModelsMock
      .mockResolvedValueOnce({
        success: true,
        models: {
          'llm/qwen/test': makeRecord('llm/qwen/test', true),
        },
      })
      .mockResolvedValueOnce({
        success: true,
        models: {
          'llm/qwen/test': makeRecord('llm/qwen/test', false),
        },
      });

    render(<Harness />);

    await flushMicrotasks();

    expect(screen.getByText(/Library integrity warning:/)).toBeInTheDocument();
    expect(screen.getByText('ISSUE')).toBeInTheDocument();

    await act(async () => {
      notifyModelLibraryUpdate?.({
        cursor: 'model-library-updates:2',
        events: [
          {
            cursor: 'model-library-updates:2',
            model_id: 'llm/qwen/test',
            change_kind: 'metadata_modified',
            fact_family: 'metadata',
            refresh_scope: 'summary_and_detail',
          },
        ],
        stale_cursor: false,
        snapshot_required: false,
      });
      vi.advanceTimersByTime(250);
      await Promise.resolve();
    });

    expect(screen.queryByText(/Library integrity warning:/)).not.toBeInTheDocument();
    expect(screen.queryByText('ISSUE')).not.toBeInTheDocument();
    expect(screen.getByText('Qwen Test')).toBeInTheDocument();
  });
});
