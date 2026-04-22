import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { MappingAction } from '../types/api';
import { ConflictResolutionItem } from './ConflictResolutionItem';

function createConflict(overrides: Partial<MappingAction> = {}): MappingAction {
  return {
    model_id: 'model-1',
    model_name: 'Model One',
    source_path: '/library/source/model-one.gguf',
    target_path: '/links/model-one.gguf',
    reason: 'different source',
    existing_target: '/existing/model-one.gguf',
    ...overrides,
  };
}

describe('ConflictResolutionItem', () => {
  it('renders conflict summary and changes the selected resolution', () => {
    const onResolutionChange = vi.fn();

    render(
      <ConflictResolutionItem
        conflict={createConflict()}
        currentResolution="skip"
        isApplying={false}
        isExpanded={false}
        onResolutionChange={onResolutionChange}
        onToggleExpanded={vi.fn()}
      />
    );

    expect(screen.getByText('Model One')).toBeInTheDocument();
    expect(screen.getByText('Symlink points to a different model file')).toBeInTheDocument();

    fireEvent.change(screen.getByRole('combobox'), {
      target: { value: 'rename' },
    });

    expect(onResolutionChange).toHaveBeenCalledWith('model-1', 'rename');
  });

  it('renders expanded path details and radio descriptions', () => {
    render(
      <ConflictResolutionItem
        conflict={createConflict({ reason: 'file exists' })}
        currentResolution="rename"
        isApplying={false}
        isExpanded={true}
        onResolutionChange={vi.fn()}
        onToggleExpanded={vi.fn()}
      />
    );

    expect(screen.getByText('A regular file exists at this location')).toBeInTheDocument();
    expect(screen.getByText('Source:')).toBeInTheDocument();
    expect(screen.getByText('source/model-one.gguf')).toBeInTheDocument();
    expect(screen.getByText('links/model-one.gguf')).toBeInTheDocument();
    expect(screen.getByText('/existing/model-one.gguf')).toBeInTheDocument();
    expect(screen.getByText('Rename existing to .old, create new link')).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /rename existing/i })).toBeChecked();
  });
});
