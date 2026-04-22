import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useModelImportPicker } from './useModelImportPicker';

const {
  isApiAvailableMock,
  openModelImportDialogMock,
} = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  openModelImportDialogMock: vi.fn<() => Promise<{ success: boolean; paths: string[] }>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    open_model_import_dialog: openModelImportDialogMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('useModelImportPicker', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    openModelImportDialogMock.mockResolvedValue({ success: true, paths: [] });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('opens the import dialog when the picker returns paths', async () => {
    openModelImportDialogMock.mockResolvedValue({
      success: true,
      paths: ['/models/a.gguf', '/models/b.safetensors'],
    });
    const { result } = renderHook(() => useModelImportPicker({}));

    await act(async () => {
      await result.current.openImportPicker();
    });

    expect(result.current.importPaths).toEqual(['/models/a.gguf', '/models/b.safetensors']);
    expect(result.current.showImportDialog).toBe(true);
  });

  it('closes the import dialog and clears selected paths', async () => {
    openModelImportDialogMock.mockResolvedValue({
      success: true,
      paths: ['/models/a.gguf'],
    });
    const { result } = renderHook(() => useModelImportPicker({}));

    await act(async () => {
      await result.current.openImportPicker();
      result.current.closeImportDialog();
    });

    expect(result.current.importPaths).toEqual([]);
    expect(result.current.showImportDialog).toBe(false);
  });

  it('notifies the caller when import completes', () => {
    const onModelsImported = vi.fn();
    const { result } = renderHook(() => useModelImportPicker({ onModelsImported }));

    act(() => {
      result.current.completeImport();
    });

    expect(onModelsImported).toHaveBeenCalledTimes(1);
  });

  it('does not open the picker when the bridge is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const { result } = renderHook(() => useModelImportPicker({}));

    await act(async () => {
      await result.current.openImportPicker();
    });

    expect(openModelImportDialogMock).not.toHaveBeenCalled();
    expect(result.current.showImportDialog).toBe(false);
  });
});
