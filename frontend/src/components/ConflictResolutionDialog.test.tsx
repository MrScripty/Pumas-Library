import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { MappingAction } from '../types/api';
import { ConflictResolutionDialog } from './ConflictResolutionDialog';

function createConflict(overrides: Partial<MappingAction> = {}): MappingAction {
  return {
    model_id: 'model-1',
    model_name: 'Model One',
    source_path: '/library/source/model-one.gguf',
    target_path: '/links/model-one.gguf',
    reason: 'file exists',
    existing_target: '/existing/model-one.gguf',
    ...overrides,
  };
}

describe('ConflictResolutionDialog', () => {
  it('renders as a named dialog and closes from backdrop or Escape key', () => {
    const onClose = vi.fn();

    render(
      <ConflictResolutionDialog
        isOpen={true}
        conflicts={[createConflict()]}
        onClose={onClose}
        onApply={vi.fn().mockResolvedValue(undefined)}
      />
    );

    expect(screen.getByRole('dialog', { name: 'Resolve Conflicts' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Close conflict resolution dialog' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('applies default skip resolutions when submitted without changes', async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);

    render(
      <ConflictResolutionDialog
        isOpen={true}
        conflicts={[
          createConflict(),
          createConflict({
            model_id: 'model-2',
            model_name: 'Model Two',
            source_path: '/library/source/model-two.gguf',
            target_path: '/links/model-two.gguf',
          }),
        ]}
        onClose={vi.fn()}
        onApply={onApply}
        versionTag="v1.2.3"
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /apply resolutions/i }));

    await waitFor(() => {
      expect(onApply).toHaveBeenCalledWith({
        'model-1': 'skip',
        'model-2': 'skip',
      });
    });
  });

  it('applies bulk rename selection and shows expanded conflict details', async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);

    render(
      <ConflictResolutionDialog
        isOpen={true}
        conflicts={[
          createConflict(),
          createConflict({
            model_id: 'model-2',
            model_name: 'Model Two',
            source_path: '/library/source/model-two.gguf',
            target_path: '/links/model-two.gguf',
            reason: 'different source',
          }),
        ]}
        onClose={vi.fn()}
        onApply={onApply}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /rename existing/i }));

    expect(
      screen.getByText((_, node) => node?.textContent?.replace(/\s+/g, ' ').trim() === '2 rename')
    ).toBeInTheDocument();
    screen.getAllByRole('combobox').forEach((select) => {
      expect(select).toHaveValue('rename');
    });

    const summaryButton = screen.getByText('Model One').closest('button');
    if (summaryButton === null) {
      throw new TypeError('Expected conflict summary button');
    }
    fireEvent.click(summaryButton);

    expect(screen.getByText('Source:')).toBeInTheDocument();
    expect(screen.getByText('source/model-one.gguf')).toBeInTheDocument();
    expect(screen.getByText('links/model-one.gguf')).toBeInTheDocument();
    expect(screen.getByText('/existing/model-one.gguf')).toBeInTheDocument();
  });
});
