import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MappingPreview } from './MappingPreview';
import type { MappingPreviewResponse } from './MappingPreviewTypes';

const {
  applyModelMappingMock,
  getCrossFilesystemWarningMock,
  isApiAvailableMock,
  previewModelMappingMock,
} = vi.hoisted(() => ({
  applyModelMappingMock: vi.fn(),
  getCrossFilesystemWarningMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  previewModelMappingMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    apply_model_mapping: applyModelMappingMock,
    get_cross_filesystem_warning: getCrossFilesystemWarningMock,
    preview_model_mapping: previewModelMappingMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

const preview: MappingPreviewResponse = {
  broken_to_remove: [],
  conflicts: [],
  errors: [],
  success: true,
  to_create: [
    {
      model_id: 'new-model',
      model_name: 'New Model',
      reason: 'Missing link',
      source_path: '/source/new.safetensors',
      target_path: '/models/new.safetensors',
    },
  ],
  to_skip_exists: [],
  total_actions: 1,
  warnings: [],
};

describe('MappingPreview', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    getCrossFilesystemWarningMock.mockResolvedValue({
      cross_filesystem: false,
      success: true,
    });
  });

  it('loads preview details and applies mapping from the expanded view', async () => {
    const onMappingApplied = vi.fn();
    previewModelMappingMock.mockResolvedValue(preview);
    applyModelMappingMock.mockResolvedValue({
      links_created: 1,
      links_removed: 0,
      success: true,
    });

    render(<MappingPreview versionTag="v1.2.3" onMappingApplied={onMappingApplied} />);

    expect(await screen.findByText('1 links ready')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Mapping Preview/ }));

    expect(screen.getByText('To Create')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Apply Mapping' }));

    await waitFor(() => {
      expect(onMappingApplied).toHaveBeenCalledWith({
        links_created: 1,
        links_removed: 0,
      });
    });
  });

  it('renders preview errors and retries loading', async () => {
    previewModelMappingMock.mockResolvedValue({
      error: 'Mapping service unavailable',
      success: false,
    });

    render(<MappingPreview versionTag="v1.2.3" />);

    expect(await screen.findByText('Failed to load mapping preview')).toBeInTheDocument();
    expect(screen.getByText('Mapping service unavailable')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

    await waitFor(() => {
      expect(previewModelMappingMock).toHaveBeenCalledTimes(2);
    });
  });
});
