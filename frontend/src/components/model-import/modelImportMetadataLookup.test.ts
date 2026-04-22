import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SetStateAction } from 'react';
import type {
  EmbeddedMetadataResponse,
  FileTypeValidationResponse,
  HFMetadataLookupResponse,
} from '../../types/api';
import type { ImportEntryStatus } from './modelImportWorkflowTypes';
import { runMetadataLookup, type MetadataLookupEntry } from './modelImportMetadataLookup';

const {
  getEmbeddedMetadataMock,
  lookupHFMetadataForBundleDirectoryMock,
  lookupHFMetadataMock,
  validateFileTypeMock,
} = vi.hoisted(() => ({
  getEmbeddedMetadataMock: vi.fn<(_path: string) => Promise<EmbeddedMetadataResponse>>(),
  lookupHFMetadataForBundleDirectoryMock: vi.fn<(_path: string) => Promise<HFMetadataLookupResponse>>(),
  lookupHFMetadataMock: vi.fn<(_filename: string, _path?: string | null) => Promise<HFMetadataLookupResponse>>(),
  validateFileTypeMock: vi.fn<(_path: string) => Promise<FileTypeValidationResponse>>(),
}));

vi.mock('../../api/import', () => ({
  importAPI: {
    getEmbeddedMetadata: getEmbeddedMetadataMock,
    lookupHFMetadata: lookupHFMetadataMock,
    lookupHFMetadataForBundleDirectory: lookupHFMetadataForBundleDirectoryMock,
    validateFileType: validateFileTypeMock,
  },
}));

function createEntry(overrides: Partial<ImportEntryStatus> = {}): ImportEntryStatus {
  return {
    path: '/imports/model.gguf',
    originPath: '/imports/model.gguf',
    filename: 'model.gguf',
    kind: 'single_file',
    status: 'pending',
    securityTier: 'safe',
    securityAcknowledged: true,
    metadataStatus: 'pending',
    suggestedFamily: 'qwen',
    suggestedOfficialName: 'Qwen 3 8B',
    modelType: 'llm',
    ...overrides,
  };
}

function createHarness(initialEntries: ImportEntryStatus[]) {
  let entries = initialEntries;
  let lookupProgress = { current: 0, total: 0 };
  return {
    get entries() {
      return entries;
    },
    get lookupProgress() {
      return lookupProgress;
    },
    setEntries: (updater: SetStateAction<ImportEntryStatus[]>) => {
      entries = typeof updater === 'function' ? updater(entries) : updater;
    },
    setLookupProgress: (
      updater: SetStateAction<{ current: number; total: number }>
    ) => {
      lookupProgress = typeof updater === 'function'
        ? updater(lookupProgress)
        : updater;
    },
  };
}

describe('runMetadataLookup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('uses embedded metadata repo matches for GGUF files without querying HF search', async () => {
    validateFileTypeMock.mockResolvedValue({
      success: true,
      valid: true,
      detected_type: 'gguf',
    });
    getEmbeddedMetadataMock.mockResolvedValue({
      success: true,
      file_type: 'gguf',
      metadata: {
        'general.repo_url': 'https://huggingface.co/Qwen/Qwen3-8B-GGUF',
      },
    });
    const harness = createHarness([createEntry()]);
    const entriesToProcess: MetadataLookupEntry[] = [
      { path: '/imports/model.gguf', filename: 'model.gguf', kind: 'single_file' },
    ];

    await runMetadataLookup({
      entriesToProcess,
      setEntries: harness.setEntries,
      setLookupProgress: harness.setLookupProgress,
    });

    expect(harness.entries[0]?.metadataStatus).toBe('found');
    expect(harness.entries[0]?.hfMetadata?.repo_id).toBe('Qwen/Qwen3-8B-GGUF');
    expect(harness.lookupProgress).toEqual({ current: 1, total: 1 });
    expect(lookupHFMetadataMock).not.toHaveBeenCalled();
  });

  it('looks up external bundle directory metadata through the bundle endpoint', async () => {
    lookupHFMetadataForBundleDirectoryMock.mockResolvedValue({
      success: true,
      found: true,
      metadata: {
        repo_id: 'org/bundle',
        official_name: 'Bundle',
        family: 'org',
        match_method: 'filename_exact',
        match_confidence: 0.9,
        requires_confirmation: false,
      },
    });
    const harness = createHarness([
      createEntry({
        path: '/imports/bundle',
        filename: 'bundle',
        kind: 'external_diffusers_bundle',
      }),
    ]);

    await runMetadataLookup({
      entriesToProcess: [
        { path: '/imports/bundle', filename: 'bundle', kind: 'external_diffusers_bundle' },
      ],
      setEntries: harness.setEntries,
      setLookupProgress: harness.setLookupProgress,
    });

    expect(harness.entries[0]?.metadataStatus).toBe('found');
    expect(harness.entries[0]?.hfMetadata?.repo_id).toBe('org/bundle');
    expect(validateFileTypeMock).not.toHaveBeenCalled();
  });

  it('marks invalid single-file entries as lookup errors and still advances progress', async () => {
    validateFileTypeMock.mockResolvedValue({
      success: true,
      valid: false,
      detected_type: 'unknown',
    });
    const harness = createHarness([createEntry()]);

    await runMetadataLookup({
      entriesToProcess: [
        { path: '/imports/model.gguf', filename: 'model.gguf', kind: 'single_file' },
      ],
      setEntries: harness.setEntries,
      setLookupProgress: harness.setLookupProgress,
    });

    expect(harness.entries[0]?.validFileType).toBe(false);
    expect(harness.entries[0]?.metadataStatus).toBe('error');
    expect(harness.lookupProgress).toEqual({ current: 1, total: 1 });
  });
});
