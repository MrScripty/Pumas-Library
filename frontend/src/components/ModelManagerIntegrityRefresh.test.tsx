import { act, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModelLibraryUpdateNotification, ModelRecord } from '../types/api';
import { useModels } from '../hooks/useModels';
import { ModelManager } from './ModelManager';

const {
  getElectronAPIMock,
  getModelsMock,
  isApiAvailableMock,
} = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  getModelsMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
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
    downloadStatusByRepo: {},
    hasActiveDownloads: false,
    pauseDownload: vi.fn(),
    resumeDownload: vi.fn(),
    setDownloadErrors: vi.fn(),
    startDownload: vi.fn(),
  }),
}));

vi.mock('../hooks/useModelImportPicker', () => ({
  useModelImportPicker: () => ({
    closeImportDialog: vi.fn(),
    completeImport: vi.fn(),
    importPaths: [],
    openImportPicker: vi.fn(),
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
    recoveringPartialRepoIds: new Set<string>(),
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

function makeRecord(id: string, hasIntegrityIssue: boolean): ModelRecord {
  return {
    id,
    path: `/models/${id}`,
    modelType: 'llm',
    officialName: 'Qwen Test',
    tags: [],
    hashes: {},
    metadata: hasIntegrityIssue
      ? {
          integrity_issue_duplicate_repo_id: true,
          integrity_issue_duplicate_repo_id_count: 2,
          repo_id: 'qwen/test',
        }
      : {
          repo_id: 'qwen/test',
        },
    updatedAt: '2026-05-04T00:00:00Z',
  };
}

function Harness() {
  const { modelGroups } = useModels();

  return (
    <ModelManager
      modelGroups={modelGroups}
      starredModels={new Set()}
      excludedModels={new Set()}
      onToggleStar={vi.fn()}
      onToggleLink={vi.fn()}
      selectedAppId="comfyui"
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
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
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
