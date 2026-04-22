import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useExistingLibraryChooser } from './useExistingLibraryChooser';

describe('useExistingLibraryChooser', () => {
  it('runs the chooser and resets pending state', async () => {
    const onChooseExistingLibrary = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const { result } = renderHook(() => useExistingLibraryChooser({ onChooseExistingLibrary }));

    await act(async () => {
      await result.current.chooseExistingLibrary();
    });

    expect(onChooseExistingLibrary).toHaveBeenCalledTimes(1);
    expect(result.current.isChoosingExistingLibrary).toBe(false);
  });

  it('exposes pending state while the chooser is active', async () => {
    let resolveChooser: () => void = () => undefined;
    const chooserPromise = new Promise<void>((resolve) => {
      resolveChooser = resolve;
    });
    const onChooseExistingLibrary = vi.fn<() => Promise<void>>().mockReturnValue(chooserPromise);
    const { result } = renderHook(() => useExistingLibraryChooser({ onChooseExistingLibrary }));
    let action: Promise<void> = Promise.resolve();

    act(() => {
      action = result.current.chooseExistingLibrary();
    });

    expect(result.current.isChoosingExistingLibrary).toBe(true);

    await act(async () => {
      resolveChooser();
      await action;
    });

    expect(result.current.isChoosingExistingLibrary).toBe(false);
  });

  it('ignores duplicate chooser requests while pending', async () => {
    let resolveChooser: () => void = () => undefined;
    const chooserPromise = new Promise<void>((resolve) => {
      resolveChooser = resolve;
    });
    const onChooseExistingLibrary = vi.fn<() => Promise<void>>().mockReturnValue(chooserPromise);
    const { result } = renderHook(() => useExistingLibraryChooser({ onChooseExistingLibrary }));
    let firstAction: Promise<void> = Promise.resolve();

    act(() => {
      firstAction = result.current.chooseExistingLibrary();
    });

    await act(async () => {
      await result.current.chooseExistingLibrary();
    });

    expect(onChooseExistingLibrary).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveChooser();
      await firstAction;
    });
  });

  it('no-ops when no chooser is provided', async () => {
    const { result } = renderHook(() => useExistingLibraryChooser({}));

    await act(async () => {
      await result.current.chooseExistingLibrary();
    });

    expect(result.current.isChoosingExistingLibrary).toBe(false);
  });
});
