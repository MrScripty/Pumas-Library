import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ConversionProgress, ConversionSetupStatusResponse } from '../types/api-conversion';

const mocks = vi.hoisted(() => ({ readiness: vi.fn(), list: vi.fn(), setup: vi.fn(), setupStatus: vi.fn(), start: vi.fn(), cancel: vi.fn() }));
vi.mock('../api/adapter', () => ({ api: {
  check_conversion_environment: mocks.readiness,
  list_model_conversions: mocks.list,
  start_conversion_setup: mocks.setup,
  get_conversion_setup: mocks.setupStatus,
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
function setupSnapshot(status: NonNullable<ConversionSetupStatusResponse['setup']>['status'], operationId = '00112233-4455-4677-8899-aabbccddeeff'): ConversionSetupStatusResponse {
  return { success: true, setup: { operationId, status, error: status === 'failed' ? 'Conversion environment setup did not complete successfully.' : null } };
}

describe('model conversion workflow ownership', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    mocks.readiness.mockResolvedValue({ success: true, ready: true });
    mocks.list.mockResolvedValue({ success: true, conversions: [] });
    mocks.setup.mockResolvedValue(setupSnapshot('completed'));
    mocks.setupStatus.mockResolvedValue({ success: true, setup: null });
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
    mocks.readiness.mockResolvedValue({ success: true, ready: false });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    mocks.setup.mockRejectedValue(new Error('/private/root token=secret'));
    await act(async () => { await result.current.setup(); });
    expect(result.current.error).toBe('Tool setup could not be confirmed and may still be running. Refresh status to check backend setup status.');
    expect(result.current.setupUncertain).toBe(true);
    await act(async () => { await result.current.setup(); });
    expect(mocks.setup).toHaveBeenCalledTimes(1);
    mocks.readiness.mockResolvedValueOnce({ success: true, ready: false });
    act(() => result.current.refresh());
    await flush();
    expect(result.current.setupUncertain).toBe(true);
    expect(result.current.error).toContain('may still be running');
    mocks.readiness.mockResolvedValue({ success: true, ready: true });
    mocks.setupStatus.mockResolvedValue(setupSnapshot('completed'));
    act(() => result.current.refresh());
    await flush();
    expect(result.current.setupUncertain).toBe(false);
    expect(result.current.error).toBeNull();
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('converting')] });
    act(() => result.current.refresh());
    await flush();
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

  it('keeps writing and importing active until backend completion refreshes the model list once', async () => {
    const onCompleted = vi.fn();
    mocks.list.mockResolvedValue({ success: true, conversions: [{ ...conversion('writing'), progress: 0.95 }] });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors', onCompleted }));
    await flush();
    expect(result.current.conversions[0]?.status).toBe('writing');
    expect(onCompleted).not.toHaveBeenCalled();
    await act(async () => { await result.current.start(); });
    expect(mocks.start).not.toHaveBeenCalled();

    mocks.list.mockResolvedValue({ success: true, conversions: [{ ...conversion('importing'), progress: 1 }] });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(mocks.list).toHaveBeenCalledTimes(2);
    expect(result.current.conversions[0]?.status).toBe('importing');
    expect(onCompleted).not.toHaveBeenCalled();
    await act(async () => { await result.current.cancel('job'); });
    expect(mocks.cancel).toHaveBeenCalledExactlyOnceWith('job');
    expect(result.current.conversions[0]?.status).toBe('importing');
    expect(onCompleted).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(mocks.list).toHaveBeenCalledTimes(4);

    mocks.list.mockResolvedValue({ success: true, conversions: [{ ...conversion('completed'), progress: 1, outputModelId: 'converted-model' }] });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.conversions[0]?.outputModelId).toBe('converted-model');
    expect(onCompleted).toHaveBeenCalledTimes(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.list).toHaveBeenCalledTimes(5);
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

  it('does not admit setup while a conversion is active even if readiness becomes false', async () => {
    mocks.readiness.mockResolvedValue({ success: true, ready: false });
    mocks.list.mockResolvedValue({ success: true, conversions: [conversion('converting')] });
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    await act(async () => { await result.current.setup(); });
    expect(mocks.setup).not.toHaveBeenCalled();
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

  it('attaches on reopen without mutation and polls active setup until actual readiness is checked', async () => {
    mocks.setupStatus.mockResolvedValue(setupSnapshot('in_progress'));
    const { result, unmount } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    expect(result.current.setupOperation?.status).toBe('in_progress');
    expect(result.current.busy).toBe(false);
    await act(async () => { await result.current.start(); await result.current.setup(); });
    expect(mocks.setup).not.toHaveBeenCalled();
    expect(mocks.start).not.toHaveBeenCalled();
    const held = deferred<ConversionSetupStatusResponse>();
    mocks.setupStatus.mockReturnValueOnce(held.promise);
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    await act(async () => { await vi.advanceTimersByTimeAsync(4000); });
    expect(mocks.setupStatus).toHaveBeenCalledTimes(2);
    mocks.readiness.mockResolvedValue({ success: true, ready: false });
    await act(async () => { held.resolve(setupSnapshot('completed')); });
    expect(result.current.ready).toBe(false);
    expect(result.current.setupOperation?.status).toBe('completed');
    expect(result.current.error).toBeNull();
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.setupStatus).toHaveBeenCalledTimes(2);
    unmount();
  });

  it('ends busy after admission, passes an observed terminal ID only on explicit retry, and stops on read error', async () => {
    mocks.readiness.mockResolvedValue({ success: true, ready: false });
    mocks.setupStatus.mockResolvedValue(setupSnapshot('failed'));
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    const admission = deferred<ConversionSetupStatusResponse>();
    mocks.setup.mockReturnValue(admission.promise);
    let work!: Promise<void>;
    act(() => { work = result.current.setup(); void result.current.setup(); });
    await flush();
    expect(result.current.busy).toBe(true);
    expect(mocks.setup).toHaveBeenCalledExactlyOnceWith('00112233-4455-4677-8899-aabbccddeeff');
    const heldRead = deferred<ConversionSetupStatusResponse>();
    mocks.setupStatus.mockReturnValueOnce(heldRead.promise);
    await act(async () => { admission.resolve(setupSnapshot('in_progress', '11112233-4455-4677-8899-aabbccddeeff')); });
    expect(result.current.busy).toBe(false);
    expect(result.current.loading).toBe(true);
    await act(async () => { heldRead.reject(new Error('private diagnostic')); await work; });
    expect(result.current.error).toBe('Could not read conversion status. Refresh status to retry.');
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); await result.current.setup(); });
    expect(mocks.setupStatus).toHaveBeenCalledTimes(2);
    expect(mocks.setup).toHaveBeenCalledTimes(1);
    mocks.setupStatus.mockResolvedValue(setupSnapshot('cancelled', '11112233-4455-4677-8899-aabbccddeeff'));
    act(() => result.current.refresh());
    await flush();
    expect(result.current.error).toBeNull();
  });

  it('does not reconcile uncertain retry with null or the same terminal identity even if ready', async () => {
    mocks.readiness.mockResolvedValue({ success: true, ready: false });
    mocks.setupStatus.mockResolvedValue(setupSnapshot('failed'));
    const { result } = renderHook(() => useModelConversionWorkflow({ modelId: 'model', direction: 'gguf_to_safetensors' }));
    await flush();
    mocks.setup.mockRejectedValue(new Error('admission timeout /secret'));
    await act(async () => { await result.current.setup(); });
    expect(result.current.setupUncertain).toBe(true);
    mocks.readiness.mockResolvedValue({ success: true, ready: true });
    for (const snapshot of [{ success: true, setup: null }, setupSnapshot('failed')]) {
      mocks.setupStatus.mockResolvedValue(snapshot);
      act(() => result.current.refresh());
      await flush();
      expect(result.current.setupUncertain).toBe(true);
      await act(async () => { await result.current.setup(); await result.current.start(); });
    }
    expect(mocks.setup).toHaveBeenCalledTimes(1);
    expect(mocks.start).not.toHaveBeenCalled();
    mocks.setupStatus.mockResolvedValue(setupSnapshot('completed', '11112233-4455-4677-8899-aabbccddeeff'));
    act(() => result.current.refresh());
    await flush();
    expect(result.current.setupUncertain).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('observes superseded setup reads without publishing stale status or restarting polling after unmount', async () => {
    const held = deferred<ConversionSetupStatusResponse>();
    mocks.setupStatus.mockReturnValueOnce(held.promise);
    const { result, rerender, unmount } = renderHook(({ modelId }) => useModelConversionWorkflow({ modelId, direction: 'gguf_to_safetensors' }), { initialProps: { modelId: 'model' } });
    await flush();
    rerender({ modelId: 'different' });
    await flush();
    expect(mocks.setupStatus).toHaveBeenCalledTimes(1);
    await act(async () => { held.resolve(setupSnapshot('in_progress')); });
    expect(mocks.setupStatus).toHaveBeenCalledTimes(2);
    expect(result.current.setupOperation).toBeNull();
    mocks.setupStatus.mockResolvedValue(setupSnapshot('in_progress'));
    act(() => result.current.refresh());
    await flush();
    unmount();
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(mocks.setupStatus).toHaveBeenCalledTimes(3);
    expect(mocks.setup).not.toHaveBeenCalled();
  });
});
