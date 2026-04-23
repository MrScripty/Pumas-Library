import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { MappingPreviewDetails } from './MappingPreviewDetails';
import type { MappingPreviewResponse } from './MappingPreviewTypes';

const preview: MappingPreviewResponse = {
  broken_to_remove: [],
  conflicts: [
    {
      existing_target: '/models/existing.safetensors',
      model_id: 'conflict-model',
      model_name: 'Conflict Model',
      reason: 'Target already exists',
      source_path: '/source/conflict.safetensors',
      target_path: '/models/conflict.safetensors',
    },
  ],
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
  to_skip_exists: [
    {
      model_id: 'existing-model',
      model_name: 'Existing Model',
      reason: 'Already linked',
      source_path: '/source/existing.safetensors',
      target_path: '/models/existing.safetensors',
    },
  ],
  total_actions: 3,
  warnings: ['Cross-device link may copy data'],
};

function renderDetails(overrides: Partial<React.ComponentProps<typeof MappingPreviewDetails>> = {}) {
  const props: React.ComponentProps<typeof MappingPreviewDetails> = {
    applyResult: {
      links_created: 1,
      links_removed: 0,
      success: true,
    },
    brokenCount: 0,
    conflictCount: preview.conflicts.length,
    crossFsWarning: {
      cross_filesystem: true,
      recommendation: 'Use the same library volume',
      warning: 'Source and target are on different filesystems.',
    },
    expandedSection: null,
    hasIssues: true,
    isApplying: false,
    isLoading: false,
    onApplyMapping: vi.fn(),
    onFetchPreview: vi.fn(),
    onToggleSection: vi.fn(),
    preview,
    showApplyButton: true,
    skipCount: preview.to_skip_exists.length,
    status: 'warnings',
    toCreateCount: preview.to_create.length,
    ...overrides,
  };

  render(<MappingPreviewDetails {...props} />);
  return props;
}

describe('MappingPreviewDetails', () => {
  it('renders mapping summaries, warnings, results, and action controls', () => {
    const props = renderDetails();

    expect(screen.getByText('Cross-Filesystem Warning')).toBeInTheDocument();
    expect(screen.getByText('Cross-device link may copy data')).toBeInTheDocument();
    expect(screen.getByText('Mapping Applied')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(props.onFetchPreview).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Apply Mapping' }));
    expect(props.onApplyMapping).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Links to Create (1)' }));
    expect(props.onToggleSection).toHaveBeenCalledWith('create');
  });

  it('shows the all-linked notice when only existing links remain', () => {
    renderDetails({
      applyResult: null,
      conflictCount: 0,
      crossFsWarning: null,
      hasIssues: false,
      preview: {
        ...preview,
        conflicts: [],
        to_create: [],
        warnings: [],
      },
      status: 'ready',
      toCreateCount: 0,
    });

    expect(screen.getByText('All models already linked')).toBeInTheDocument();
  });
});
