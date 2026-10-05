import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { S3ImportOutcome, S3ImportParams, S3ImportCancelOutcome } from '../generated/desktop-contract';
import { useS3ModelImport, type S3ImportDraft } from './useS3ModelImport';

const { start, get, cancel } = vi.hoisted(() => ({
  start: vi.fn<(request: S3ImportParams) => Promise<S3ImportOutcome>>(),
  get: vi.fn<(id?: string) => Promise<S3ImportOutcome>>(),
  cancel: vi.fn<(id: string) => Promise<S3ImportCancelOutcome>>(),
}));
vi.mock('../api/import', () => ({ importAPI: {
  startS3ModelImport: start, getS3ModelImport: get, cancelS3ModelImport: cancel,
} }));
const id = 'c3f7d104-1234-4321-abcd-aaaaaaaaaaaa';
const draft: S3ImportDraft = {
  endpoint: 'https://source.invalid', region: 'fixture-region', bucket: 'fixture-bucket',
  addressing: 'path', key: 'models/exact object.gguf', version_id: 'exact+version/id',
  filename: 'weights.gguf', sha256: 'a'.repeat(64), family: 'fixture', official_name: 'Fixture GGUF',
};
const running: S3ImportOutcome = { status: 'running', operation_id: id,
  progress: { phase: 'acquiring', downloaded_for_current_file: '18446744073709551615' } };
const completed: S3ImportOutcome = { status: 'finished', operation_id: id,
  result: { status: 'completed', model_id: 'fixture/model' } };
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
async function settle() { await act(async () => { await Promise.resolve(); }); }

describe('explicit S3 import observation', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.spyOn(crypto, 'randomUUID').mockReturnValue(id);
    get.mockResolvedValue({ status: 'idle' });
  });
  afterEach(() => { vi.clearAllMocks(); vi.restoreAllMocks(); vi.useRealTimers(); });

  it('validates pins before admission and forwards exact source facts once while a command is pending', async () => {
    const pending = deferred<S3ImportOutcome>();
    start.mockReturnValueOnce(pending.promise);
    get.mockResolvedValueOnce({ status: 'idle' }).mockResolvedValue(running);
    const { result } = renderHook(() => useS3ModelImport());
    await settle();
    for (const invalid of [{ sha256: 'bad' }, { version_id: '' }, { endpoint: 'http://source.invalid' },
      { endpoint: 'https://user:secret@source.invalid' }, { session_token: 'synthetic-secret' }]) {
      await act(async () => { await result.current.start({ ...draft, ...invalid }); });
    }
    expect(start).not.toHaveBeenCalled();
    let command!: Promise<void>;
    act(() => { command = result.current.start(draft); });
    await act(async () => { await result.current.start(draft); });
    expect(start).toHaveBeenCalledTimes(1);
    expect(start).toHaveBeenCalledWith({ ...draft, operation_id: id });
    await act(async () => { pending.resolve(running); await command; });
    expect(result.current.snapshot).toEqual(running);
    expect(result.current.commandBusy).toBe(false);
  });

  it('fences an older status response when cancellation takes ownership; acknowledgement is not completion', async () => {
    const old = deferred<S3ImportOutcome>();
    get.mockResolvedValueOnce(running).mockReturnValueOnce(old.promise).mockResolvedValue(running);
    cancel.mockResolvedValue({ accepted: true, outcome: {
      ...running, progress: { phase: 'cancelling', downloaded_for_current_file: '1' },
    } });
    const onImported = vi.fn();
    const { result } = renderHook(() => useS3ModelImport(onImported));
    await settle();
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    await act(async () => { await result.current.cancel(); });
    await act(async () => { old.resolve(completed); });
    expect(result.current.snapshot?.status).toBe('running');
    expect(result.current.error).toContain('Cancellation requested');
    expect(onImported).not.toHaveBeenCalled();
    expect(cancel).toHaveBeenCalledWith(id);
  });

  it('observes the same identity after an ambiguous admission and never resubmits', async () => {
    start.mockRejectedValue(new Error('private transport detail'));
    get.mockResolvedValueOnce({ status: 'idle' }).mockResolvedValue(completed);
    const onImported = vi.fn();
    const { result } = renderHook(() => useS3ModelImport(onImported));
    await settle();
    await act(async () => { await result.current.start(draft); });
    expect(start).toHaveBeenCalledTimes(1);
    expect(get).toHaveBeenLastCalledWith(id);
    expect(result.current.snapshot).toEqual(completed);
    expect(result.current.error).toBeNull();
    expect(onImported).toHaveBeenCalledTimes(1);
    act(() => result.current.observeAgain());
    await settle();
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('stops polling on close and reopens the retained backend result without admission', async () => {
    const pending = deferred<S3ImportOutcome>();
    get.mockResolvedValueOnce(running).mockReturnValueOnce(pending.promise).mockResolvedValue(completed);
    const onImported = vi.fn();
    const old = renderHook(() => useS3ModelImport(onImported));
    await settle();
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    old.unmount();
    await act(async () => { pending.resolve(completed); await vi.advanceTimersByTimeAsync(5000); });
    expect(get).toHaveBeenCalledTimes(2);
    expect(onImported).not.toHaveBeenCalled();
    const reopened = renderHook(() => useS3ModelImport(onImported));
    await settle();
    expect(reopened.result.current.snapshot).toEqual(completed);
    expect(start).not.toHaveBeenCalled();
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('does not accept an observation for another identity or leak rejection diagnostics', async () => {
    get.mockResolvedValueOnce(running)
      .mockResolvedValueOnce({ ...completed, operation_id: 'different' })
      .mockRejectedValueOnce(new Error('private diagnostic'));
    const { result } = renderHook(() => useS3ModelImport());
    await settle();
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.snapshot).toBeNull();
    expect(result.current.error).toContain('did not match');
    act(() => result.current.observeAgain());
    await settle();
    expect(result.current.error).toContain('observation is unavailable');
    expect(result.current.error).not.toContain('private diagnostic');
    expect(get).toHaveBeenLastCalledWith(id);
  });
});
