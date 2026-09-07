import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useModelImportPicker } from './useModelImportPicker';
import type { ModelImportSelection } from '../../../electron/src/model-import-picker';

const {
  isApiAvailableMock,
  openModelImportDialogMock,
} = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  openModelImportDialogMock: vi.fn<() => Promise<ModelImportSelection>>(),
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
    openModelImportDialogMock.mockResolvedValue({ status: 'cancelled' });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('opens the import dialog when the picker returns paths', async () => {
    openModelImportDialogMock.mockResolvedValue({
      status: 'selected',
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
      status: 'selected',
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

  it('reports unavailable rejection and supports a fresh retry without leaking diagnostics', async () => {
    openModelImportDialogMock.mockRejectedValueOnce(new Error('/private/path failed'));
    const { result } = renderHook(() => useModelImportPicker({}));
    await act(async () => { expect(await result.current.openImportPicker()).toBe('unavailable'); });
    expect(result.current.pickerError).toBe('Model file picker unavailable. Try again.');
    expect(result.current.isPicking).toBe(false);
    await act(async () => { expect(await result.current.openImportPicker()).toBe('cancelled'); });
    expect(result.current.pickerError).toBeNull();
    expect(result.current.showImportDialog).toBe(false);
  });

  it.each(['invalid', 'unavailable'] as const)('preserves existing selected paths after %s or cancellation', async status => {
    const paths = ['/models/ e\u0301.gguf', '/models/ e\u0301.gguf', '/models/é.gguf'];
    openModelImportDialogMock.mockResolvedValueOnce({ status: 'selected', paths });
    const { result } = renderHook(() => useModelImportPicker({}));
    await act(async () => { await result.current.openImportPicker(); });
    openModelImportDialogMock.mockResolvedValueOnce({ status });
    await act(async () => { expect(await result.current.openImportPicker()).toBe(status); });
    expect(result.current.pickerError).not.toBeNull();
    expect(result.current.importPaths).toEqual(paths);
    expect(result.current.showImportDialog).toBe(true);
    await act(async () => { await result.current.openImportPicker(); });
    expect(result.current.importPaths).toEqual(paths);
    expect(result.current.showImportDialog).toBe(true);
  });

  it('suppresses duplicate requests and observes a closed pending selection as superseded', async () => {
    let resolve!: (value: ModelImportSelection) => void;
    openModelImportDialogMock.mockReturnValueOnce(new Promise(done => { resolve = done; }));
    const { result } = renderHook(() => useModelImportPicker({}));
    let pending!: ReturnType<typeof result.current.openImportPicker>;
    act(() => { pending = result.current.openImportPicker(); });
    expect(result.current.isPicking).toBe(true);
    await act(async () => { expect(await result.current.openImportPicker()).toBe('busy'); });
    act(() => result.current.closeImportDialog());
    await act(async () => {
      resolve({ status: 'selected', paths: ['/late.gguf'] });
      expect(await pending).toBe('superseded');
    });
    expect(openModelImportDialogMock).toHaveBeenCalledTimes(1);
    expect(result.current.showImportDialog).toBe(false);
    expect(result.current.importPaths).toEqual([]);
    expect(result.current.isPicking).toBe(false);
  });

  it('observes rejection after unmount without publishing into a replacement owner', async () => {
    let reject!: (error: Error) => void;
    openModelImportDialogMock.mockReturnValueOnce(new Promise((_, fail) => { reject = fail; }));
    const old = renderHook(() => useModelImportPicker({}));
    let pending!: ReturnType<typeof old.result.current.openImportPicker>;
    act(() => { pending = old.result.current.openImportPicker(); });
    old.unmount();
    const replacement = renderHook(() => useModelImportPicker({}));
    await act(async () => { reject(new Error('late')); expect(await pending).toBe('superseded'); });
    expect(replacement.result.current.pickerError).toBeNull();
    expect(replacement.result.current.showImportDialog).toBe(false);
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
