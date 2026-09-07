import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConversionProgress } from '../types/api-conversion';

const mocks = vi.hoisted(() => ({ readiness: vi.fn(), list: vi.fn(), setup: vi.fn(), start: vi.fn(), cancel: vi.fn() }));
vi.mock('../api/adapter', () => ({ api: {
  check_conversion_environment: mocks.readiness,
  list_model_conversions: mocks.list,
  setup_conversion_environment: mocks.setup,
  start_model_conversion: mocks.start,
  cancel_model_conversion: mocks.cancel,
} }));
import { useModelConversionWorkflow } from './useModelConversionWorkflow';

function conversion(status: ConversionProgress['status'], conversionId = 'job'): ConversionProgress {
  return { conversionId, sourceModelId: 'model', direction: 'gguf_to_safetensors', status,
    progress: null, currentTensor: null, tensorsCompleted: null, tensorsTotal: null,
    bytesWritten: null, estimatedOutputSize: null, targetQuant: null, error: null,
    outputModelId: null, pipelineStep: null, pipelineStepsTotal: null, pipelineStepLabel: null };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
async function flush() { await act(async () => { await Promise.resolve(); }); }

describe('model conversion workflow ownership', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    mocks.readiness.mockResolvedValue({ success: true, ready: true });
    mocks.list.mockResolvedValue({ success: true, conversions: [] });
    mocks.setup.mockResolvedValue({ success: true });
    mocks.start.mockResolvedValue({ success: true, conversion_id: 'job' });
    mocks.cancel.mockResolvedValue({ success: true, cancelled: true });
  });
  afterEach(() => { vi.useRealTimers(); });

  it('requires explicit setup and suppresses duplicate starts synchronously', async () => {
    mocks.readiness.mockResolvedValueOnce({ success: true, ready: false });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    expect(mocks.setup).not.toHaveBeenCalled();
    await act(async () => { await result.current.start(); });
    expect(mocks.start).not.toHaveBeenCalled();
    await act(async () => { await result.current.setup(); });
    expect(result.current.ready).toBe(true);
    const held = deferred<{ success: true; conversion_id: string }>();
    mocks.start.mockReturnValue(held.promise);
    let operation!: Promise<void>;
    act(() => { operation = result.current.start(); void result.current.start(); });
    await flush();
    expect(result.current.busy).toBe(true);
    expect(mocks.start).toHaveBeenCalledExactlyOnceWith('model', 'gguf_to_safetensors', 'F16');
    await act(async () => { held.resolve({ success: true, conversion_id: 'job' }); await operation; });
    await act(async () => { await result.current.start(); });
    expect(mocks.start).toHaveBeenCalledTimes(1);
  });

  it('retains uncertain start refusal after refresh without automatic retry', async () => {
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'safetensors_to_gguf' }));
    await flush();
    mocks.start.mockRejectedValue(new Error('reply lost'));
    await act(async () => { await result.current.start(); });
    expect(result.current.startUncertain).toBe(true);
    expect(result.current.error).toContain('close and reopen');
    act(() => result.current.refresh());
    await flush();
    await act(async () => { await result.current.start(); });
    expect(result.current.startUncertain).toBe(true);
    expect(mocks.start).toHaveBeenCalledTimes(1);
  });

  it('shows accepted starts missing from the list until authoritative progress appears', async () => {
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    await act(async () => { await result.current.start(); });
    expect(result.current.awaitingProgress).toBe(true);
    expect(result.current.busy).toBe(false);
    await act(async () => { await result.current.start(); });
    expect(mocks.start).toHaveBeenCalledTimes(1);
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('converting')] });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.awaitingProgress).toBe(false);
    expect(result.current.conversions[0]?.conversionId).toBe('job');
  });

  it('allows dismissal during slow reads and observes rejection after unmount', async () => {
    const held = deferred<{ success: true; conversions: ConversionProgress[] }>();
    mocks.list.mockReturnValue(held.promise);
    const { result, unmount } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    expect(result.current.loading).toBe(true);
    expect(result.current.busy).toBe(false);
    unmount();
    await act(async () => { held.reject(new Error('/private/root token=secret')); });
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.list).toHaveBeenCalledTimes(1);
  });

  it('projects bounded setup and cancellation errors without exposing bridge diagnostics', async () => {
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('converting')] });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    mocks.setup.mockRejectedValue(new Error('/private/root token=secret'));
    await act(async () => { await result.current.setup(); });
    expect(result.current.error).toBe('Tool setup could not be confirmed and may still be running. Refresh status to check readiness.');
    expect(result.current.setupUncertain).toBe(true);
    await act(async () => { await result.current.setup(); });
    expect(mocks.setup).toHaveBeenCalledTimes(1);
    mocks.readiness.mockResolvedValueOnce({ success: true, ready: false });
    act(() => result.current.refresh());
    await flush();
    expect(result.current.setupUncertain).toBe(true);
    expect(result.current.error).toContain('may still be running');
    act(() => result.current.refresh());
    await flush();
    expect(result.current.setupUncertain).toBe(false);
    expect(result.current.error).toBeNull();
    mocks.cancel.mockRejectedValue(new Error('/private/root token=secret'));
    await act(async () => { await result.current.cancel('job'); });
    expect(result.current.error).toBe('Could not request cancellation. Refresh status to check the backend.');
    expect(result.current.error).not.toContain('secret');
  });

  it('polls sequentially, stops on read error, and notifies only new completions once', async () => {
    const onCompleted = vi.fn();
    mocks.list.mockResolvedValueOnce({ success: true, conversions: [conversion('completed', 'historical'), conversion('converting')] });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors', onCompleted }));
    await flush();
    expect(onCompleted).not.toHaveBeenCalled();
    const held = deferred<{ success: true; conversions: ConversionProgress[] }>();
    mocks.list.mockReturnValueOnce(held.promise);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.list).toHaveBeenCalledTimes(2);
    await act(async () => { held.reject(new Error('read failed')); });
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.list).toHaveBeenCalledTimes(2);
    expect(result.current.error).toBe('Could not read conversion status. Refresh status to retry.');
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('completed', 'historical'), conversion('completed')] });
    act(() => result.current.refresh());
    await flush();
    act(() => result.current.refresh());
    await flush();
    expect(onCompleted).toHaveBeenCalledTimes(1);
  });

  it('reports false cancellation and waits for backend terminal status after accepted cancellation', async () => {
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('converting')] });
    mocks.cancel.mockResolvedValueOnce({ success: true, cancelled: false });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    await act(async () => { await result.current.cancel('job'); });
    expect(result.current.error).toContain('not active');
    await act(async () => { await result.current.cancel('job'); });
    expect(result.current.conversions[0]?.status).toBe('converting');
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(mocks.list).toHaveBeenCalledTimes(4);
  });

  it('observes superseded calls without overlapping reads or applying stale results', async () => {
    const held = deferred<{ success: true; conversions: ConversionProgress[] }>();
    mocks.list.mockReturnValueOnce(held.promise);
    const onCompleted = vi.fn();
    const { result, rerender, unmount } = renderHook(({ modelId }) => useModelConversionWorkflow({ modelId, direction: 'gguf_to_safetensors', onCompleted }), { initialProps: { modelId: 'model' } });
    await flush();
    rerender({ modelId: 'different-model' });
    await flush();
    expect(mocks.list).toHaveBeenCalledTimes(1);
    await act(async () => { held.resolve({ success: true, conversions: [conversion('completed')] }); });
    expect(result.current.conversions).toEqual([]);
    expect(mocks.list).toHaveBeenCalledTimes(2);
    expect(onCompleted).not.toHaveBeenCalled();
    const later = deferred<{ success: true; conversions: ConversionProgress[] }>();
    mocks.list.mockReturnValueOnce(later.promise);
    act(() => result.current.refresh());
    await flush();
    unmount();
    await act(async () => { later.reject(new Error('late rejection')); });
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.list).toHaveBeenCalledTimes(3);
  });
});
