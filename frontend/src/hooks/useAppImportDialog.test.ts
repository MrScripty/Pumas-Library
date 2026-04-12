import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useAppImportDialog } from './useAppImportDialog';

describe('useAppImportDialog', () => {
  it('opens the import dialog when paths are dropped and clears it on close', () => {
    const onImportComplete = vi.fn();
    const { result } = renderHook(() => useAppImportDialog({ onImportComplete }));

    act(() => {
      result.current.handlePathsDropped(['/models/a.gguf', '/models/b.gguf']);
    });

    expect(result.current.showImportDialog).toBe(true);
    expect(result.current.importPaths).toEqual(['/models/a.gguf', '/models/b.gguf']);

    act(() => {
      result.current.handleImportDialogClose();
    });

    expect(result.current.showImportDialog).toBe(false);
    expect(result.current.importPaths).toEqual([]);
  });

  it('invokes the completion callback when import finishes', () => {
    const onImportComplete = vi.fn();
    const { result } = renderHook(() => useAppImportDialog({ onImportComplete }));

    act(() => {
      result.current.handleImportComplete();
    });

    expect(onImportComplete).toHaveBeenCalledTimes(1);
  });
});
