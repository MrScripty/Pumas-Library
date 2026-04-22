import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { MappingAction } from '../types/api';
import { useConflictResolutions } from './useConflictResolutions';

function createConflict(overrides: Partial<MappingAction> = {}): MappingAction {
  return {
    model_id: 'model-a',
    model_name: 'Model A',
    source_path: '/models/source.gguf',
    target_path: '/targets/source.gguf',
    reason: 'file exists',
    ...overrides,
  };
}

describe('useConflictResolutions', () => {
  it('defaults conflicts to skip and counts resolution types', () => {
    const { result } = renderHook(() => useConflictResolutions({
      conflicts: [
        createConflict({ model_id: 'model-a' }),
        createConflict({ model_id: 'model-b' }),
      ],
      onApply: vi.fn().mockResolvedValue(undefined),
    }));

    expect(result.current.effectiveResolutions).toEqual({
      'model-a': 'skip',
      'model-b': 'skip',
    });
    expect(result.current.resolutionCounts).toEqual({
      skip: 2,
      overwrite: 0,
      rename: 0,
    });
  });

  it('updates individual and bulk resolutions', () => {
    const { result } = renderHook(() => useConflictResolutions({
      conflicts: [
        createConflict({ model_id: 'model-a' }),
        createConflict({ model_id: 'model-b' }),
      ],
      onApply: vi.fn().mockResolvedValue(undefined),
    }));

    act(() => {
      result.current.handleResolutionChange('model-a', 'overwrite');
    });

    expect(result.current.resolutionCounts).toEqual({
      skip: 1,
      overwrite: 1,
      rename: 0,
    });

    act(() => {
      result.current.handleApplyToAll('rename');
    });

    expect(result.current.effectiveResolutions).toEqual({
      'model-a': 'rename',
      'model-b': 'rename',
    });
  });

  it('tracks expanded conflict rows', () => {
    const { result } = renderHook(() => useConflictResolutions({
      conflicts: [createConflict()],
      onApply: vi.fn().mockResolvedValue(undefined),
    }));

    act(() => {
      result.current.toggleExpanded('model-a');
    });
    expect(result.current.expandedConflict).toBe('model-a');

    act(() => {
      result.current.toggleExpanded('model-a');
    });
    expect(result.current.expandedConflict).toBeNull();
  });

  it('applies effective resolutions and clears pending state', async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    const { result } = renderHook(() => useConflictResolutions({
      conflicts: [createConflict()],
      onApply,
    }));

    act(() => {
      result.current.handleResolutionChange('model-a', 'overwrite');
    });
    await act(async () => {
      await result.current.handleApply();
    });

    expect(onApply).toHaveBeenCalledWith({ 'model-a': 'overwrite' });
    expect(result.current.isApplying).toBe(false);
  });
});
